# Codex CLI 核心逻辑拆解（我们这个项目的地图）

> 目的：先看清**一次 turn 从按键到收尾**经过哪些零件、每个零件的边界在哪，再决定学什么、按什么顺序学。
> 所有 `path:line` 都对着锁定的 `f9e7b19`。这份文档是"地图"，`docs/modules/M*.md` 是"每一段的路书"。

---

## 1. 三层，七个零件

Codex CLI 不是"一个 agent 类"，是**一条流水线**：前端把输入塞进队列 → 会话循环把输入变成一次 turn →
turn 每轮向模型要事件、必要时调工具 → 事件同时被推给前端和落盘。

| # | 零件 | 职责（一句话） | 上游主文件 | 我们的取舍 |
|---|------|----------------|------------|-----------|
| 1 | **前端** | 解析参数、选模式、把用户输入变成一次提交、打印事件 | `cli/src/main.rs` → `exec/src/lib.rs:271 run_main`、`861 run_exec_session` | 只做 exec 模式（`--json`/`-o`），不做 TUI/云/app-server |
| 2 | **提交通道** | `Op` 进、`EventMsg` 出；一个线程一个队列 | `core/src/session/mod.rs:1072 submission_loop`、`1223 next_event`、`handlers.rs:594` | 做同步队列（无 tokio），只保留 `TurnInput`/`Interrupt` |
| 3 | **任务与 turn 循环** | 决定"这一轮怎么跑"：记输入 → 可能要压缩 → 组 step → 采样 → 调工具 → 再来一轮 | `tasks/regular.rs:48 run` → `turn.rs:178 run_turn`（主循环 `:491`） | 做单轮 + 工具回环，砍 compaction/guardian/hooks |
| 4 | **上下文与提示词** | 决定"发给模型什么"：历史 + 指令 + 工具清单 + 环境 | `core/src/context_manager/{mod,history,normalize}.rs`、`core/src/agents_md.rs` | 先做 history 拼接与 AGENTS.md 注入，砍归一化与 token 预算 |
| 5 | **模型客户端** | 发请求、解析流、把每个 SSE 事件变成 `ResponseEvent` | `core/src/client.rs:2510 stream` | 先假模型（脚本化事件流），最后才接真 SSE |
| 6 | **工具层** | 把模型给的 `FunctionCall` 变成受控执行，结果写成 `FunctionCallOutput` | `core/src/tools/router.rs:344 dispatch_tool_call_with_code_mode_result`、`:367`、`spec_plan.rs` | 先做 `shell` 一个工具，再加 `apply_patch`；砍 MCP/多代理/计划工具 |
| 7 | **持久化** | 事件流追加成 JSONL，重启能接着聊 | `rollout/src/recorder.rs`（`~/.codex/sessions/YYYY/MM/DD`） | 做追加写 + 恢复；砍 SQLite 索引/压缩/搜索 |

配置（`config`）不是零件，是**贯穿所有零件的参数板**：它决定用哪个模型、哪个沙箱、哪些工具可见。
所以它不能太早学（没东西可配），也不能太晚（否则你以为行为是硬编码的）。

### 砍掉的东西（以及为什么能砍）

| 砍掉 | 上游位置 | 为什么可以砍 |
|------|----------|--------------|
| TUI / app-server / 云任务 | `tui`（39.9 万行）、`app-server` | 它们只是前端；核心逻辑一行都不在里面 |
| MCP / 多代理 / 计划模式 | `codex-mcp`、`agent/` | 工具层的**扩展**，不是工具层本身 |
| compaction / guardian / hooks | `turn.rs` 里 1457、1592、`guardian/` | 都是"围绕主循环"的旁路；主循环不依赖它们 |
| 权限策略引擎全量 | `execpolicy`（3.1 k） | 我们只做前缀规则三态（M4） |
| SQLite 线程库 | `state`、`thread-store` | JSONL 足够复现"恢复会话" |

判断标准：**主循环能不能不依赖它跑完一轮？** 能 → 砍。

---

## 2. 一轮 turn 的真实时序

```
① 前端        exec::run_main → run_exec_session → start_thread           exec/src/lib.rs:271,861,1433
② 提交        Session::submit(Op::TurnInput{..})                          session/mod.rs:1120 附近
③ 分派        submission_loop 读队列 → handlers.rs:594 匹配 TurnInput     session/handlers.rs:594
④ 起任务      RegularTask::run → run_turns（可多轮）                      tasks/regular.rs:48,129
⑤ 记输入      run_hooks_and_record_inputs / record_conversation_items     turn.rs:447
⑥ 预压缩      run_pre_sampling_compact（我们不做，留空）                  turn.rs:1457
⑦ 组 step     捕获 step context：模型信息 + 工具路由 + 提示词             turn.rs:491 之前
⑧ 采样        run_sampling_request → try_run_sampling_request             turn.rs:1792,2751
  ⑧a 发请求    ModelClient::stream                                        client.rs:2510
  ⑧b 收事件    ResponseEvent → ResponseItem → 事件/条目                    turn.rs:774
⑨ 调工具      tool_router.dispatch_tool_call_with_state                   tools/router.rs:367
  ⑨a 执行      shell / apply_patch 具体处理器
  ⑨b 回写      FunctionCallOutput 记入历史，回到 ⑧ 再采样
⑩ 收尾        无工具调用且模型给出终稿 → TurnComplete(turn_id,..)          protocol.rs:1594
⑪ 落盘        每个事件/条目追加进 rollout JSONL                            rollout/src/recorder.rs
⑫ 前端打印    --json 逐行输出；-o 写最终消息                               exec/src/lib.rs
```

三个必须记住的事实：

1. **`Op` 与 `EventMsg` 不是同一种消息**。`Op` 是进程内提交（带 `oneshot::Sender`，没有 serde，`protocol.rs:702/768`），
   `EventMsg` 是可序列化事件（`tag="type"`，`protocol.rs:1531`）。前端只消费后者。
2. **"一轮"其实是一个循环**（`turn.rs:491`）：采样 → 有工具调用就执行并回写 → 再采样，直到模型不再调工具。
   所以 M7 不是"写一个请求函数"，是**写一个带终止条件的循环**。
3. **事件有两条去向**：给前端看（`TurnItem`，PascalCase tag）和发给模型/落盘（`ResponseItem`，snake_case tag）。
   同一件事有两种表示，是这行代码里最容易写错的地方（见 `docs/modules/M1-protocol.md` §6）。

---

## 3. 六个"接缝"（学习优先级最高的地方）

接缝 = 两个零件交换数据的那一行。**读懂接缝比读完整模块有用**，因为 bug 和理解偏差几乎都长在接缝上。

| 接缝 | 交界处 | 要回答的问题 | 上游 |
|------|--------|--------------|------|
| S1 前端 → 会话 | `Op::TurnInput` | 用户输入怎么变成一次提交？中途插入怎么办？ | `handlers.rs:594` |
| S2 会话 → 任务 | `RegularTask` 选择 | 什么输入走"普通轮"，什么输入走 review/compact？ | `tasks/mod.rs` |
| S3 上下文 → 请求 | step context 组装 | 历史、指令、工具清单、环境各自从哪来、按什么顺序拼？ | `turn.rs:1763` 附近 |
| S4 流 → 事件 | `ResponseEvent` 翻译 | 一个 delta 怎么变成 `agent_message`？终稿怎么判定？ | `turn.rs:774`、`protocol.rs:985` |
| S5 工具调用 → 执行 | `router.dispatch` | 谁决定能不能并行？失败怎么变成 `FunctionCallOutput`？ | `router.rs:260,367` |
| S6 事件 → 落盘 | recorder | 哪些事件落盘、哪些只给界面？恢复时怎么重放？ | `rollout/src/recorder.rs` |

---

## 4. 新的学习顺序（tracer bullet：先跑通一条细线，再逐段加厚）

旧顺序（M1 类型 → M2 配置 → M3 指令 → …）的问题是：**前十小时看不到任何会动的东西**，
而且类型是先验的——你背下了 `ResponseItem`，但不知道它在第几步被谁构造。

新顺序按"每完成一段就能看到一个可观察现象"排：

| 段 | 做什么（能跑通的细线） | 依赖 | 你应当看到的现象 | 对应旧编号 |
|----|------------------------|------|------------------|-----------|
| **R0** | 骨架 + `x.sh`（已完成） | — | `./x.sh` 绿 | M0 |
| **R1** | 最小类型子集：`TurnInput`、`EventMsg` 六个事件、`TurnItem` 四个条目、ID | — | 能把一段写死的 JSON 解成事件再编码回来 | M1（缩到 40%） |
| **R2** | **假模型 + 单轮循环**：脚本给 3 个事件（started / 一条 agent_message / complete），跑完打印 | R1 | `mc-cli` 跑出一条完整 turn 的事件流 | M7 的骨架部分 |
| **R3** | **工具回环**：加 `shell` 工具；模型说"要调工具"→ 执行 → 回写 → 再要一次 → 结束 | R2 | 看到 `exec_command_begin/end` 包住一次真实命令 | M4 的前半 |
| **R4** | **写文件**：`apply_patch`，原子失败 | R3 | 上下文不匹配时文件不变、事件是 error | M5 |
| **R5** | **上下文与指令**：配置分层 + AGENTS.md 注入 + prompt 组装顺序 | R2 | 改配置文件/加 AGENTS.md 后，请求内容跟着变 | M2+M3 |
| **R6** | **持久化与恢复**：JSONL 追加、`resume` 重放 | R2 | 杀掉进程后能接着上一次的会话 | M6 |
| **R7** | **真模型**：SSE 流解析、重试、usage | R2 | 同一个循环接真 API，事件流形状不变 | M7 剩余 |
| **R8** | **前端收口**：`--json`/`-o`/退出码 | R7 | 可被脚本消费 | M8 |

顺序理由（三条，都来自代码结构）：

1. **R2 必须先于一切"实现细节"**：`run_turn` 的骨架（`turn.rs:178`）决定了后面每个模块被调用的位置。
   先把骨架跑起来，类型和工具才有落脚点。
2. **R3/R4 在 R5 之前**：工具执行的输入输出形状（`FunctionCallOutput`）会影响提示词与历史，
   先做工具再学上下文，能亲眼看到"为什么历史里要存工具输出"。
3. **R7 放最后**：真模型的 SSE 是最脏的一段（重试、断流、usage、加密内容），
   放在假模型验证过循环之后，出错时你能分清是"循环错了"还是"解析错了"。

---

## 5. 每段的可观察验收（不是"测试通过"，是"看到现象"）

| 段 | 验收要看到 |
|----|-----------|
| R1 | 一段手写 JSON → `EventMsg` → 编码回 JSON，`type` 是 `task_started`（不是 `turn_started`） |
| R2 | 终端依次出现：`task_started` → `agent_message` → `task_complete`，且 `turn_id` 一致 |
| R3 | 中间多出 `exec_command_begin{cwd,command}` / `exec_command_end{exit_code,aggregated_output}`，`call_id` 配对 |
| R4 | 故意给错上下文 → 文件内容不变、有 error 事件；给对 → 文件变了、`exit_code=0` |
| R5 | 往项目根加一行 `AGENTS.md`，请求体里出现对应的指令块；换 profile，模型名跟着变 |
| R6 | `Ctrl-C` 后再跑 `resume`，历史条数与上次相同、能继续提问 |
| R7 | 同一个二进制，把假模型换成真客户端，事件序列形状不变（只多了 usage/delta） |
| R8 | `mc-cli --json > out.jsonl` 每行都是合法 JSON；错命令退出码非 0 |

---

## 6. 每个零件"学到什么算懂"（对应旧文档）

| 零件 | 懂的标志 |
|------|----------|
| 前端 | 能说出 `--json` 的每一行是谁产生的、为什么不是一次性输出 |
| 提交通道 | 能解释为什么 `Op` 不能序列化，而 `EventMsg` 可以 |
| turn 循环 | 能画出"采样 → 工具 → 再采样"的循环图，并说出三种终止条件 |
| 上下文 | 能列出请求体的组成部分与拼接顺序，指出哪部分来自配置、哪部分来自仓库 |
| 模型客户端 | 能说出一个 SSE delta 到 `agent_message` 的完整变换链 |
| 工具层 | 能解释并行工具的安全边界，以及失败如何回写成结果 |
| 持久化 | 能解释"哪些事件进 JSONL"以及恢复时如何避免重复执行工具 |

---

## 7. 这份拆解纠正的三个误解

1. **"先学类型，再学逻辑"** → 类型是被循环塑造的。先有骨架（R2），类型的字段才有意义。
2. **"一个 turn = 一次请求"** → `turn.rs:491` 是循环：请求→工具→再请求，直到模型不再要工具。
3. **"事件是给人看的"** → 事件同时供前端显示（`TurnItem`）和发给模型/落盘（`ResponseItem`），
   两种表示 tag 规则不同（PascalCase vs snake_case），这是最容易静默写错的一处。
