# Codex CLI 全局架构

> 对象：锁定的上游 `f9e7b19`（`/home/seagull/MiniCodex/codex-src`）。
> 所有 `path:line` 都在该提交上核过。标注〔未验证〕的地方是我没读到实现、只从类型或调用点推断的。
> 用法：**先读 §1 一张图，再读 §5 一次 turn，最后按 §10 的顺序读代码。** 不要从头顺读。

---

## 1. 一页图

```
┌─ 层 1 前端 ────────────────────────────────────────────────────────────┐
│ cli/src/main.rs:145  Subcommand::Exec ..                                │
│   → exec/src/lib.rs:271 run_main → :861 run_exec_session → :1433 start  │
│   → 自己的事件类型 exec/src/exec_events.rs:11 ThreadEvent               │
│      （thread.started / turn.started / item.* / error，逐行 JSON）       │
└───────────────┬────────────────────────────────────────────────────────┘
                │ ① app-server JSON-RPC（ClientRequest / ServerNotification）
┌───────────────▼─ 层 2 线程与协议 ──────────────────────────────────────┐
│ app-server（7.0 万行）：把"开线程 / 发一轮 / 中断 / 审批" 变成 RPC       │
│ core 侧：ThreadManager → CodexThread 持有 Session                      │
└───────────────┬────────────────────────────────────────────────────────┘
                │ ② core 内部提交协议：Op 进 / EventMsg 出
┌───────────────▼─ 层 3 会话与 turn ─────────────────────────────────────┐
│ Session::submit(Op)        session/mod.rs:1110                          │
│   → submission_loop        session/mod.rs:1072                          │
│   → handlers.rs:594 Op::TurnInput → 选 Task                             │
│   → RegularTask::run       tasks/regular.rs:48                          │
│   → run_turns（有排队输入就再跑一轮）tasks/regular.rs:129                │
│   → run_turn               session/turn.rs:178                          │
│        ├─ 记输入 / 预压缩 / 组 step（Prompt + 工具清单）                 │
│        └─ loop { run_sampling_request → 工具派发 → needs_follow_up? }    │
│   → ModelClient::stream    client.rs:2510                               │
│   → tool_router.dispatch   tools/router.rs:367                          │
└───────────────┬────────────────────────────────────────────────────────┘
                │ ③ 模型线格式：ResponseItem 进请求、SSE ResponseEvent 回来
┌───────────────▼─ 层 4 基础设施 ────────────────────────────────────────┐
│ config（分层 TOML）· history/rollout（JSONL 事件溯源）· sandbox · auth   │
└────────────────────────────────────────────────────────────────────────┘
```

一句话：**同一个 turn 的信息被翻译了五次**（前端参数 → JSON-RPC → `Op` → `Prompt`/`ResponseItem` → SSE 事件 → `EventMsg` → `ThreadEvent`）。
架构的复杂度不在算法，全在这些翻译的边界上。

---

## 2. 四层，五个"语言"

| 层 | 职责 | 代码规模 | 我们是否复刻 |
|----|------|----------|--------------|
| 1 前端 | 参数、模式、把结果打印成可消费的流 | cli 3.0 万 + exec 0.66 万 + tui 39.9 万 | 只做 exec 形态 |
| 2 线程/协议 | 多线程生命周期、对外 RPC（IDE/TUI/exec 共用） | app-server 7.0 万 + app-server-protocol 3.8 万 | **不做**，用直接函数调用替掉 |
| 3 会话与 turn | 提交队列、任务选择、turn 循环、提示词、工具编排 | core 26.8 万 | 做最小闭环 |
| 4 基础设施 | 配置、历史与落盘、沙箱、认证 | config 3.4 万 / rollout 1.9 万 / history 0.4 万 | 做配置、历史、落盘；沙箱只做策略判定 |

五种"语言"及其定义处：

| 语言 | 定义 | 谁读 | 可序列化 |
|------|------|------|----------|
| app-server RPC | `app-server-protocol` 的 `ClientRequest` / `ServerNotification` | 前端 ↔ core | 是（JSON-RPC） |
| 提交协议 | `Op`（`protocol.rs:702`） | 前端 → 会话循环 | **否**（含 `oneshot::Sender`） |
| 事件协议 | `EventMsg`（`protocol.rs:1531`） | 会话循环 → 前端 / 落盘 | 是（`tag="type"`） |
| 模型线格式 | `ResponseItem`（`models.rs:1069`） | core ↔ 模型 API | 是（`tag="type"`, snake_case） |
| 展示条目 | `TurnItem`（`items.rs:47`） | 界面渲染 | 是（tag 是 PascalCase） |

落盘另有 `RolloutItem`（`history/src/lib.rs:220`：`SessionMeta` / `ResponseItem` / `DynamicToolsUpdated` …）——它是**事件的持久化投影**，不是新语义。

---

## 3. 层 1：exec 前端做了什么（比想象的少）

`run_main`（`exec/src/lib.rs:271`）只做四件事：读配置 → 建 in-process app-server 客户端 → 起线程 → 循环消费事件。
它自己**不碰 core 的类型**，只认 `ClientRequest` 和 `ThreadEvent`：

- `InProcessAppServerClient`（`exec/src/lib.rs:22`）——同一个进程内的 JSON-RPC 客户端；
- `ThreadEvent`（`exec/src/exec_events.rs:11`）——`--json` 每行输出的那个结构，`serde(rename = "thread.started")` 这类重命名就写在这里；
- 最终消息另存（`-o`），由 `event_processor_with_jsonl_output.rs:84` 的 state 收集。

要点：**前端是协议的消费者，不是逻辑的拥有者。** 所以我们复刻时可以把层 2 整个砍掉，
让 `mc-cli` 直接调 `mc-agent`，只保留"逐行输出 `ThreadEvent`"这个形态。

## 4. 层 2：为什么 core 外面还套一层 RPC

因为 TUI、exec、IDE 三种前端要共享同一套会话能力，而 TUI 需要**在 turn 进行中插话/审批**。
`app-server` 把"开线程、提交一轮、中断、答复审批"做成请求-响应 + 通知流，core 只暴露 `Session::submit` 与 `next_event`（`session/mod.rs:1110`、`:1223`）。

我们不做这一层，但**必须保留它的两个语义**，否则 R2 之后会返工：
1. 提交是**排队**的，不是直接调用（`Op` 进队列，由会话循环串行消费）；
2. 事件是**流**，前端边收边渲染（`next_event` 一直可等）。

## 5. 层 3：一次 turn 的完整生命周期（本文主体）

### 5.1 外层：一个任务可以跑多轮 turn

`run_turns`（`tasks/regular.rs:129`）是一个循环：跑完一轮，如果 `input_queue` 里还有待处理输入（用户在 turn 中途又发了话），
就把 `next_input` 清空再跑一轮，直到没有排队输入（`:150–160`）。这就是"steering"（中途插话）的实现位置。

### 5.2 内层：run_turn 的十步

| # | 步骤 | 位置 | 数据变化 |
|---|------|------|----------|
| 1 | 记输入 | `turn.rs:447 record_conversation_items` | `TurnInput` → `ResponseItem` 进历史 |
| 2 | 预压缩 | `turn.rs:1457 run_pre_sampling_compact` | 历史可能被摘要替换（我们不做） |
| 3 | 解析 MCP/工具需求 | `turn.rs:~300` | 决定本轮可见工具集（我们固定） |
| 4 | 组 step context | `turn.rs:491` 之前 | 得到 `StepContext`：模型信息 + `tool_router` + 提示词快照〔未验证字段全貌〕 |
| 5 | 取历史快照 | `turn.rs:590 clone_history` | `Vec<ResponseItem>` |
| 6 | 组装请求 | `turn.rs:1751 build_prompt` → `Prompt { input, tools, parallel_tool_calls: true, base_instructions, output_schema }` | 历史 + 工具 JSON Schema + 系统指令 |
| 7 | 采样 | `turn.rs:1792 run_sampling_request` → `:2751 try_run_sampling_request` → `client.rs:2510 stream` | 请求 → SSE `ResponseEvent` 流 |
| 8 | 流事件落成条目 | `turn.rs:774 record_response_item_and_emit_turn_item` | 每个 `ResponseItem` → ①进历史 ②转 `TurnItem` 发事件 |
| 9 | 工具派发 | `router.rs:271 build_tool_call` → `:367 dispatch_tool_call_with_state`；并行由 `:260 tool_supports_parallel` 决定，`turn.rs:3159 in_flight.push_back` | `FunctionCall` → 执行 → `FunctionCallOutput` 进历史 |
| 10 | 判定是否再来一轮 | `turn.rs:637`：`needs_follow_up = model_needs_follow_up \|\| has_pending_input` | true → 回到 6；false → 收尾发 `task_complete` |

### 5.3 终止条件（一个公式 + 两个意外）

`turn.rs:637` 把"继续"写成了一个或运算：

```
needs_follow_up = 模型还要工具（model_needs_follow_up） || 队列里还有输入（has_pending_input）
```

所以停下来的条件只有一个：**模型不再要工具，且队列空了**。由此派生三种真实情形：

1. 正常收尾：模型给出终稿，队列空 → `task_complete`；
2. 中途插话：模型不再要工具，但用户又发了话 → 同一 turn 里继续（steering，`run_turns` 也会再跑一轮，`regular.rs:150`）；
3. 取消：`CancellationToken` 沿每层传播，中断后走 `TurnAborted` 路径而不是 `task_complete`〔未验证该路径的具体事件与顺序〕。

### 5.4 请求里到底有什么

`build_prompt`（`turn.rs:1751`）给出的 `Prompt` 字段就是"模型看到的一切"：

| 字段 | 来源 | 说明 |
|------|------|------|
| `input` | 会话历史快照 | `Vec<ResponseItem>`，含历史工具输出 |
| `tools` | `tool_router.model_visible_specs()` | 工具的 JSON Schema 清单，**由注册表生成**，不是写死的 |
| `parallel_tool_calls` | 常量 `true` | 是否允许模型一次给多个调用 |
| `base_instructions` | 配置 + 仓库指令（AGENTS.md） | 系统提示词主体 |
| `output_schema` | turn 配置 | 结构化输出时才有 |

**没有"消息字符串"这一层**：发给模型的不是一段文本，而是 `input` 数组 + `tools` 数组。这是最容易带着"聊天应用"直觉写错的地方。

## 6. 层 4：三块基础设施

| 设施 | 职责 | 关键位置 | 我们复刻到什么程度 |
|------|------|----------|--------------------|
| config | 分层合并：默认 → 用户 → 项目 → profile → CLI | `config/src/loader/mod.rs`、`types.rs` | 五层深合并 + 未知键报错（R5） |
| history / rollout | 历史是内存里的 `Vec<ResponseItem>`；落盘是 JSONL 追加 `RolloutItem` | `history/src/lib.rs:220`、`rollout/src/recorder.rs` | 追加写 + 读回重放（R6） |
| sandbox / 审批 | 决定命令能不能跑、要不要问人 | `execpolicy/`、`sandboxing/` | 只做前缀规则三态（R3 的旁支） |

认证（`login/`）与云端（`cloud-*`）不碰：本地 CLI 用已有凭据即可。

---

## 7. 为什么这么分层（六条设计约束，都有代码证据）

1. **提交串行、事件流式**：每个会话一个 `submission_loop`（`session/mod.rs:1072`），保证历史只有一个写者；
   前端只能通过 `next_event`（`:1223`）观察。→ 复刻时别用"回调里直接改历史"，会出现两份真相。
2. **`Op` 不可序列化是刻意的**：它带 `oneshot::Sender`（`protocol.rs:768`），是进程内命令，不是消息。
   能落盘的只有 `EventMsg` 与 `ResponseItem`。→ 我们的 `Op` 保持"无 serde"，别为了测试方便给它 derive。
3. **工具是注册表，不是 if-else**：请求里的 `tools` 由路由表生成（`turn.rs:1763`），
   并行与否也由注册表声明（`router.rs:260`）。→ 加工具 = 注册一条，不动主循环。
4. **工具结果就是一条 `ResponseItem`**：`FunctionCallOutput` 进历史（`models.rs:1190`），
   下一轮采样自然带上。→ 所以"工具输出要不要发给模型"不是策略问题，是数据结构问题。
5. **落盘是事件溯源**：`RolloutItem` 记录的是"发生过什么"（`history/src/lib.rs:220`），不是"当前状态"。
   恢复 = 重放，不是读快照。→ 这也是为什么坏行可以跳过。
6. **两套事件表示（`ResponseItem` / `TurnItem`）是分工，不是冗余**：前者给模型和磁盘，后者给界面。
   tag 规则不同（snake_case vs PascalCase），由 serde 属性决定，与变体名无关。

## 8. 并发与取消（三条规则）

1. **一个会话同时只有一个活动 turn**：新输入进 `input_queue`，由当前 turn 消费（`run_turns` 的循环条件）。
2. **工具可以并行，但必须是注册表声明过的**：`tool_supports_parallel`（`router.rs:260`）为假就串行；
   并行结果用 `FuturesOrdered` 保序回写（`turn.rs:3159`）。
3. **取消沿 token 传播**：`run_turn` 每一层都收 `CancellationToken`，中断不会杀死后台进程，只结束这一轮。

## 9. 我们要复刻的最小闭环

`R0–R8` 的顺序与验收见 `docs/PLAN.md` §2。这里只强调它与架构的对应：

```
R1 语言(§2 表格第 3、5 行) → R2 层3 的 5.2 十步骨架 → R3 第 9 步 → R4 第 9 步的第二种工具
→ R5 第 4/6 步的输入来源 → R6 第 8 步的落盘 → R7 第 7 步换真实现 → R8 层 1 的输出形态
```

也就是：**先造 §5.2 那条流水线，再逐段把假件换成真件。** 层 2 整个不要，层 4 只保留配置与落盘。

## 10. 读代码路线（按这个顺序，别乱翻）

| # | 读什么 | 行数 | 读的目的 |
|---|--------|------|----------|
| 1 | `exec/src/lib.rs:271-300, 861-930, 1433-1470` | ~120 | 前端只做协议消费，理解 `--json` 的由来 |
| 2 | `core/src/session/mod.rs:1072-1135, 1223-1235` | ~70 | 提交与事件两条通道 |
| 3 | `core/src/session/handlers.rs:500-600` | ~100 | `Op` 如何被分派成任务 |
| 4 | `core/src/tasks/regular.rs:48-165` | ~117 | starts/turns 两层循环与 steering |
| 5 | `core/src/session/turn.rs:178-260`（入口）、`430-520`（组 step）、`590-660`（采样结果）| ~200 | 主流程骨架 |
| 6 | `core/src/session/turn.rs:1751-1800`、`2751-2830` | ~130 | 请求怎么组装、流怎么进来 |
| 7 | `core/src/tools/router.rs:250-300, 340-400` | ~100 | 工具注册表与派发 |
| 8 | `history/src/lib.rs:220-260`、`rollout/src/recorder.rs`（挑 100 行） | ~150 | 落盘记录的种类与写入时机 |

合计约 1000 行，是理解整个 codex 的最小阅读量（对比 core 的 26.8 万行）。
其余（compaction、guardian、hooks、MCP、多代理）都是在这条主线上挂的旁路。

## 11. 术语表

| 词 | 含义 | 别混 |
|----|------|------|
| thread | 一条持久会话（有 `ThreadId`），可多次 turn | ≠ `turn` |
| session | 运行中的线程实例（`Session`，持有队列与历史） | 1 thread 在内存里对应 1 session |
| submission | 一次 `Op` 入队 | 一次 `Op::TurnInput` 会开启一个 turn |
| task | 驱动一个 turn 的执行体（Regular/Review/Compact…） | task 完成 ≠ turn 完成（turn 可续跑） |
| turn | 用户一次提问引发的处理单元，含多次采样 | 不是"一次请求" |
| step | turn 内的一次"组请求 + 采样" | 工具调用后进入下一 step |
| rollout | 落盘的事件流（JSONL） | 不是聊天记录，是事件溯源日志 |
| compaction | 历史超阈值时的摘要替换 | 会改历史，不改事件流 |

## 12. 未验证项（下一轮读代码要补）

1. `StepContext` 的完整字段与它捕获的时机（`core/src/session/step_context.rs`）；
2. `Op::TurnInput` 到 `RegularTask` 的选择逻辑（`tasks/mod.rs`，什么输入走 review/compact）；
3. `input_queue` 的具体语义：中途插话是"合并进当前轮"还是"开新轮"；
4. app-server 的 `ServerNotification` 到 exec `ThreadEvent` 的完整映射表；
5. 恢复会话时如何避免重复执行已落盘的工具调用（幂等边界在哪）；
6. 中断路径发出的事件名与顺序。

这六条读清之后再更新本文件，并把对应的行号补进 §5.2 的表。
