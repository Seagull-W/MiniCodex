# Codex CLI 是怎么跑起来的

> **延伸参考地图**：本文记录的是上游 `openai/codex`（158 万行源码）的真实调用链与架构拆解，
> 供你在推进 MiniCodex S1–S7 时随时对照「真实产品里这部分长什么样」，**不是要求逐字段照抄的规格书**。

这份文档讲 codex CLI 的整体架构。如果你刚打开这个仓库，想先弄清楚"它到底怎么运作"，
从这里开始比从 `core/src/lib.rs` 一页页翻要快得多。

codex 做的事情可以一句话说完：把用户在终端里的一句话，变成对本地文件和进程的一串操作，
再把过程实时回放给用户。仓库有 158 万行 Rust，但真正常驻运行的只有一条流水线；
剩下的都是挂在这条线上的分支——压缩、审批、MCP、多代理、TUI。

下面先给一张图，然后完整走一遍"一句话进来、一轮对话结束"，再说明每个部件的边界和阅读顺序。

---

## 一、鸟瞰

用户输入先进前端，前端把输入交给一个会话，会话把它变成一个 turn，turn 反复向模型要下一段输出，
遇到工具调用就执行，把结果塞回上下文再要一次，直到模型不再要工具。整个过程中，每一个产出
（一段助手文本、一次命令的起止、一条错误）都同时被推到前端显示、写进磁盘。

```
终端 ──▶ exec 前端 ──▶ 会话（Session）──▶ turn ──▶ 模型
                ▲            │              │
                │            │              └─▶ 工具（shell / apply_patch）
                └── 事件流 ◀──┘              │
                                            └─▶ rollout（JSONL，磁盘）
```

代码上分四层，但只有第三层是"逻辑"，其余是外壳与底座：

- **前端**（`cli` + `exec`，约 3.7 万行）：解析参数，把结果打成一行行 JSON。`tui` 是另一个前端，40 万行，与本项目无关。
- **线程与协议层**（`app-server`，约 11 万行）：让 TUI、exec、IDE 三种前端共用同一套会话能力，对外是 JSON-RPC。
- **会话与 turn**（`core`，26.8 万行）：提交队列、任务选择、turn 循环、提示词、工具编排。本文的主体。
- **基础设施**：配置（3.4 万）、历史与落盘（`history` 0.4 万 + `rollout` 1.9 万）、沙箱、认证。

状态只存在三个地方：内存里的会话历史（`Vec<ResponseItem>`）、队列里的待处理输入、磁盘上的 rollout 文件。
没有全局数据库参与主流程——SQLite 只用于线程列表这类索引，缺了也能跑。
（`rollout/src/recorder.rs:557`：没有数据库句柄时，线程列表退回文件系统扫描。）

---

## 二、一轮对话的经过

### 1. 输入进前端

`exec::run_main`（`exec/src/lib.rs:271`）做四件事：读配置、建一个进程内的 app-server 客户端、
开一条线程、循环消费事件。它不认识 core 的任何类型，只认两种东西：发出去的 `ClientRequest`
和收回来的 `ServerNotification`。这就是为什么 `exec` 只有 6.6 千行却能驱动 26 万行的 core。

### 2. 提交进队列，而不是直接调用

前端把"请处理这段输入"包成一次提交，交给会话。会话不立即执行，而是塞进队列，
由 `submission_loop`（`core/src/session/mod.rs:1072`）串行取出处理。
这条设计约束很关键：**一个会话的历史只有一个写者**，前端只能通过 `next_event`（`:1223`）观察。

提交的类型叫 `Op`，它带了一个 `oneshot::Sender`（`protocol.rs:768`），用来把"收到/接受/拒绝"回给调用方。
所以 `Op` 根本没有 serde——它是进程内命令，不是消息。能落盘的只有事件。

### 3. 决定这次输入算什么

`Op::TurnInput` 有一个模式字段（`core/src/session/turn_input.rs:264`），决定这次输入怎么处理：

- `StartOrSteer`：有活动 turn 就插进去，没有就开新 turn。这是交互式前端最常用的模式。
- `StartIfIdle`：只在空闲时开新 turn，忙就返回"不空闲"（`:554`），不会自动排队。
- `Steer`：只往指定的 turn 里投递输入（`:971`）。
- `ContinueIfIdle`：空闲时继续一次已有的对话。

要开新 turn 时，代码显式构造 `RegularTask::new()`（`turn_input.rs:426`）。
隔壁还有 `ReviewTask`、`CompactTask`、`UserShellCommandTask`，但它们由各自的 `Op` 分支选择，
不是根据输入内容猜的。插话只能插进 Regular 类型的 turn，插进 review/compact 会被拒绝
（`:917`，`ActiveTurnNotSteerable`）。

### 4. 任务开始，一轮 turn 里有什么

`run_turns`（`tasks/regular.rs:129`）是个循环：跑完一轮，如果队列里还有待处理输入，
就带着新输入再跑一轮。这是"你在它干活时补一句话"能被接住的机制。

进入 `run_turn`（`session/turn.rs:178`）后，顺序大致是：

1. 把输入记进历史（`turn.rs:447`）。用户的一句话在这里变成一条 `ResponseItem`。
2. 可能跑一次压缩（`turn.rs:1457`）。历史太长时用摘要替换，我们不做。
3. 组装这一 step 的上下文：`StepContext`，15 个字段，其中与请求直接相关的是
   `settings`（模型与参数）、`tool_router`（工具注册表）、`turn`（turn 级上下文）、
   `loaded_agents_md`（这个仓库的 `AGENTS.md` 内容）（`session/step_context.rs:26-50`）。
4. 取历史快照，`clone_history`（`turn.rs:590`），得到 `Vec<ResponseItem>`。
5. 拼请求，`build_prompt`（`turn.rs:1751`）。
6. 发请求并消费流，`run_sampling_request`（`:1792`）→ `client.rs:2510`。
7. 流里每出一条 item，就记进历史并翻译成给界面看的事件（`turn.rs:774`）。
8. 遇到工具调用就执行，结果作为 `FunctionCallOutput` 记进历史（见下一节）。
9. 判断要不要再来一轮。

### 5. 请求里到底有什么

`build_prompt` 造出来的 `Prompt` 有六个字段（`turn.rs:1758`）：

- `input`：历史快照，一个 `ResponseItem` 数组；
- `tools`：工具清单，由工具注册表生成（`:1763`），不是写死的；
- `parallel_tool_calls`：常量 `true`；
- `base_instructions`：系统指令，包含配置里的内容和这个仓库的 `AGENTS.md`；
- `output_schema` / `output_schema_strict`：需要结构化输出时才填。

值得注意的是这里**没有"消息字符串"这一层**。发给模型的不是一段对话文本，而是一个 item 数组加一份工具
JSON Schema。带着"聊天应用"的直觉读这段代码，很容易找不到"拼 prompt 的地方"——因为不存在那个地方。

还有一种增量模式：`incremental_tools` 为真时，`tools` 和 `base_instructions` 都留空
（`turn.rs:1755-1766`），工具声明和指令已经作为 item 存在于历史里，不必每次重发。
它由 `use_responses_lite && features(IncrementalTools)` 决定（`step_context.rs:55`）。
换句话说：省 token 的办法不是压缩提示词，而是把它挪进历史。

### 6. 工具调用：一条和其他 item 平等的记录

流里出现 `FunctionCall` 时，`build_tool_call`（`tools/router.rs:271`）把它转成带名字的调用，
再交给 `dispatch_tool_call_with_state`（`:367`）执行。能不能并行由注册表上的声明决定
（`:260`，`supports_parallel_tool_calls`）；并行的结果用 `FuturesOrdered` 保序回写
（`turn.rs:3159`），免得输出顺序乱掉。

执行完的结果不是"塞进对话"，而是变成一条 `FunctionCallOutput` 的 `ResponseItem` 进历史
（`models.rs:1190`）。下一轮采样自然带上它——所以"工具结果要不要给模型看"不是策略问题，
是数据结构问题：进没进历史。

### 7. 什么时候停

`turn.rs:637` 把"继续"写成了一个或运算：

```
needs_follow_up = 模型还要工具 || 队列里还有输入
```

也就是说，一轮真正结束的条件是：模型不再要工具，而且没有人再说话。

- 正常结束：发 `task_complete`，带上 `last_agent_message`；
- 有插话：继续在同一 turn 里跑（外层 `run_turns` 也会再起一轮，`regular.rs:150`）；
- 被中断：走另一条路径，发 `TurnAborted`，而不是 `task_complete`（`tasks/mod.rs:1140`），
  且这条路径**不会**再补一个完成事件。

写自己的循环时，这个公式值得照抄——它把"模型想继续"和"用户想继续"处理成了同一种情况，
少一个状态变量。

---

## 三、五种协议

一轮对话里，同一条信息会被翻译好几次。搞清楚每种"语言"归谁所有，比记住字段名有用。

| 协议 | 定义处 | 谁发给谁 | 能落盘吗 |
|------|--------|----------|----------|
| JSON-RPC（`ClientRequest` / `ServerNotification`） | `app-server-protocol` | 前端 ↔ core | 能，但没人落 |
| `Op` | `protocol/src/protocol.rs:702` | 前端 → 会话队列 | 不能，含 `oneshot::Sender` |
| `EventMsg` | `protocol.rs:1531` | 会话 → 前端 / 磁盘 | 能，`tag="type"` |
| `ResponseItem` | `protocol/src/models.rs:1069` | core ↔ 模型 API | 能，`tag="type"`，snake_case |
| `TurnItem` | `protocol/src/items.rs:47` | core → 界面 | 能，tag 是 **PascalCase** |

前两种是"命令"，后三种是"事实"。命令可以带回调、可以被拒绝；事实只描述发生了什么，所以可以落盘、可以重放。

一个容易踩的坑：`ResponseItem` 和 `TurnItem` 描述的是同一件事的两种视角——前者发给模型、写进磁盘，
后者给界面渲染。它们的 tag 规则不同（`message` 对 `UserMessage`），因为前者加了 `rename_all = "snake_case"`，
后者没有。这个差异不是随手写的，见 `docs/modules/M1-protocol.md`。

---

## 四、七条不变量

如果只记七句话，记这些。它们都能在代码里找到出处，也都在我们自己的实现里被继承。

> **提交串行，事件流式。** 每个会话只有一个 `submission_loop` 在消费队列（`session/mod.rs:1072`），
> 历史因此只有一个写者；前端只能通过 `next_event` 观察（`:1223`）。

> **`Op` 不可序列化。** 它带 `oneshot::Sender`（`protocol.rs:768`），是进程内命令。
> 想给它加 `Serialize` 之前，先问这个 `Sender` 怎么办。

> **工具是注册表，不是 if-else。** 请求里的工具清单由注册表生成（`turn.rs:1763`），
> 并行与否也由注册表声明（`router.rs:260`）。加一个工具不需要改主循环。

> **工具结果进历史。** `FunctionCallOutput` 是一条普通 `ResponseItem`（`models.rs:1190`），
> 所以"让模型看到结果"= 把它记进历史，没有别的开关。

> **落盘是事件溯源。** rollout 记的是"发生过什么"，不是"当前状态"
> （`history/src/lib.rs:220`，13 种 `RolloutItem`）。恢复等于重放，所以坏行可以跳过。

> **一个会话只有一个活动 turn。** 新输入走队列，由当前 turn 消费；review / compact 各有自己的任务类型，
> 不能靠"猜输入内容"切换（`turn_input.rs:426`、`:917`）。

> **取消是协作式的。** `CancellationToken` 从提交一路传到工具；中断先取消、等任务退出、再发事件
> （`tasks/mod.rs:1031-1140`）。它不会顺手杀掉你自己起的后台进程。

---

## 五、事件怎么落盘，又怎么读回来

写盘很朴素：一个会话一个 JSONL 文件，路径是 `~/.codex/sessions/<年>/<月>/<日>/<文件名>`
（`rollout/recorder.rs:1718`），文件名里的 rollout id 默认就是 thread id。追加模式打开，
开新文件之前先补一个换行，避免上一行被截断后粘在一起（`:2047`）。

写入是异步的：调用方把一批条目丢进队列（`AddItems`），后台 writer 按顺序逐条序列化，
每条附时间戳和可选的序数，然后 `write_all` + `flush`（`:1073`、`:1752`、`:1916`）。
写失败会关掉当前 writer、重开、重试一次；再失败就报错，成功写过的前缀从缓存里去掉，剩下的留着（`:1091-1112`）。
turn 结束时会显式 flush 一次，因为"终止事件"可能是缓冲区里最后一条（`tasks/mod.rs:1149`）。

读取时不排序，按文件行序 `push` 进结果（`recorder.rs:1138`、`:1172`）；thread id 取第一条成功解析的
`SessionMeta`。空白行跳过；坏 JSON 或解不出的行打 warning、计数、继续（`:1144`、`:1163`）；
但空文件会直接报 `empty session file`（`:1182`）。

有一个概念值得记住：恢复拿到的是"历史条目"，不是"待办指令"。工具结果早就以 `FunctionCallOutput`
的形式躺在历史里，所以重放不需要再执行一次工具——至少数据结构上没有这个必要
（线程内部如何消费这段历史，见 §十）。

---

## 六、终端里的每一行 JSON 从哪来

`exec --json` 的每一行都是 `ThreadEvent`（`exec/src/exec_events.rs:11`），它是前端的"对外语言"，
和 core 的 `EventMsg` 不是同一个类型。转换发生在
`event_processor_with_jsonl_output.rs`，规则零散但很具体：

- 先按线程 id、turn id 过滤（`exec/src/lib.rs:1701` 起）：别的线程或别的 turn 的通知直接丢掉。
- `TurnStarted` → `turn.started`（`J:597`）；`TurnCompleted` 先补发还没闭合的 item 完成事件，
  再按状态发 `turn.completed` 或 `turn.failed`（`J:516-560`）。
- `ItemStarted` / `ItemCompleted` → `item.started` / `item.completed`；助手文本和推理不输出 started，
  只有 completed（`J:344-351`）。
- 只用 token 或 diff 的通知不产出任何行（`J:512`、`:570`）。
- 一条通知可能产出零行、一行或多行；反过来，`thread.started` 这一行不来自通知，
  是初始化时按 `SessionConfiguredEvent.thread_id` 打印的（`J:609`）。

item 的 id 也要翻译：core 的原始 id 到前端是 `raw → item_<n>` 的映射，
started 时登记、completed 时取出（`J:328-341`）。这样即使某个 item 没有 started 事件，
completed 也能拿到一个稳定的 id。

`-o` 的文件在进程退出前写：事件循环结束后 `client.shutdown()` 返回，再调 `print_final_output`
（`exec/src/lib.rs:1028`、`:1422`）。内容优先取最后一条 `AgentMessage`，没有就取最后一个 `Plan`，
两者都没有就保留上一次缓存；turn 失败或中断会清空它（`J:486-543`、`:562`）。

---

## 七、并发、插话与中断

真正并发的东西只有两处：并行的工具调用，和后台的 rollout writer。其余都是串行队列。

插话有三条不同路径，别混为一谈：`StartOrSteer` 把输入塞进当前 turn；`StartIfIdle` 忙时直接失败；
`Steer` 只认指定的 turn。session 级还有一套"邮箱"，用于把外部消息留到下一轮再投递
（`input_queue.rs:237`、`:403`），和用户插话不是一回事。

中断是一条清理流水线：取消 token → 等任务退出或超时 → abort 并 join → 处理尚未准入的输入 →
执行中断 hooks → 发 `TurnAborted` → 再 flush（`tasks/mod.rs:1031-1149`）。
顺序有意义：事件发在清理之后，前端收到"已中止"时，磁盘上的历史已经是完整的。

turn 正常结束时，队列里剩下的输入不会丢：它们和"尚未准入的输入"合并，交给
`run_hooks_and_record_inputs` 记录（`tasks/mod.rs:730-762`），然后才发终止事件。

---

## 八、哪些我们不做

判断标准只有一条：主循环能不能不依赖它跑完一轮。

- `tui`（40 万行）、`app-server`（11 万行）——是外壳。我们直接把 `Session` 当库调用。
- MCP、多代理、计划模式、hooks——是工具层和循环的扩展。
- compaction、guardian、approval reviewer——是围绕主循环的旁路，`turn.rs` 里靠 `if` 分支挂上去。
- 云端（`cloud-*`）、认证（`login`）、遥测——本地学习项目用不到。

---

## 九、读代码的顺序

大约一千行，读完能掌握主干。按这个顺序读，别从 `lib.rs` 顺读。

| 顺序 | 读什么 | 为什么 |
|------|--------|--------|
| 1 | `exec/src/lib.rs:271-300`、`:861-930`、`:1428-1470` | 看前端到底有多薄 |
| 2 | `core/src/session/mod.rs:1072-1135`、`:1223-1235` | 提交与事件两条通道 |
| 3 | `core/src/session/handlers.rs:500-600` | `Op` 怎么被分派 |
| 4 | `core/src/session/turn_input.rs:264-340`、`:900-980` | 四种输入模式与 steer 限制 |
| 5 | `core/src/tasks/regular.rs:48-165` | 两层循环 |
| 6 | `core/src/session/turn.rs:178-260`、`:430-520`、`:590-660` | turn 主流程与续轮判定 |
| 7 | `core/src/session/turn.rs:1751-1800`、`:2751-2830` | 请求怎么拼、流怎么进 |
| 8 | `core/src/tools/router.rs:250-300`、`:340-400` | 工具注册表与派发 |
| 9 | `core/src/session/input_queue.rs:120-260`、`:500-530` | 队列与 `has_pending_input` |
| 10 | `history/src/lib.rs:220-260`；`rollout/src/recorder.rs` 挑 150 行 | 落盘条目与写入时机 |
| 11 | `exec/src/exec_events.rs` 全文；`exec/src/event_processor_with_jsonl_output.rs:320-610` | 一行 JSON 的由来 |

---

## 十、还没搞清的地方

这些是我读到时只能看到一半、暂时不敢写死的点。下一步补读的目标：

1. `StepContext` 的构造点。只看到结构定义（15 个字段，`step_context.rs:26-50`）和它在 `run_turn` 里的使用，
   没找到构造表达式；"每 step 重建一次"目前是推断。
2. 环境信息（cwd、git 状态、日期）注入到 `input` 的具体编码。`Prompt` 里没有这些字段，
   只知道它们在 `TurnContext` 里（`turn_context.rs:1135`、`:1303`、`:1420`），以 item 形式进历史的细节没读到。
3. 恢复会话时线程内部如何消费历史、如何保证不重复执行工具。数据上不需要重跑，但代码路径没读完。
4. `TurnInputMode` 四个模式在前端各被谁使用——`exec` 用哪个、`tui` 用哪个，没读前端调用点。
5. 中断路径里 hooks 和 rollout flush 的失败处理。顺序已确认，错误分支没看。

写完这几条，这份文档就可以当作"读代码前的地图"稳定下来；再往下就该写每个模块的路书了。
