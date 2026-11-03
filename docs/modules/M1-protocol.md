# M1 mc-protocol

> 依据：上游 `f9e7b19` 的 `codex-rs/protocol`。本文所有结论都在源码里核过，行号可直接跳。
> 解读草稿来自 Codex 的三份报告（MC-0005/0006/0007），**只保留了我在源码复核通过的部分**；
> 它报错的、含糊的、以及"通常来说"的部分全部删掉了。

## 0. 一句话 + 位置

协议层 = 会话里流通的**三种语言**：发给模型的线格式（`ResponseItem`）、给人看的条目（`TurnItem`）、
进程内外传的消息（`Op` 进 / `EventMsg` 出）。它不依赖任何模块，M2–M8 全部依赖它。

```
Op ──→ [会话循环] ──→ EventMsg ──→ 前端 / rollout
              │
              ├──→ ResponseItem ──→ 模型 API（发出去 / 收回来）
              └──→ TurnItem ─────→ 界面显示
```

## 1. 上游在哪（只读这些）

| 文件 | 行数 | 读它为了什么 |
|------|------|--------------|
| `protocol/src/session_id.rs`、`thread_id.rs`、`response_item_id.rs` | 各 <120 | 三种 ID：两种 UUID、一种宽松字符串。看它们怎么手写 serde |
| `protocol/src/models.rs` 883–913、1069–1341、2368–2379 | — | `ContentItem`、`ResponseItem`、工具输出 payload |
| `protocol/src/items.rs` 47–75、85–293 | — | `TurnItem` 与四个条目结构 |
| `protocol/src/protocol.rs` 694–941、1528–1700、2329–2345、2407–2465、2781–2793、3870–3945 | — | `Op`、`EventMsg` 与六个事件结构 |
| `protocol/src/turn_input.rs` | — | `TurnInputRequest`（`Op::TurnInput` 的载荷） |

`protocol/src/protocol.rs` 有 6803 行：**按行号区间读**，别通读。

## 2. 数据结构

### 2.1 三种 ID（`ids.rs`）

| 类型 | 内部表示 | 序列化成什么 | 反序列化行为 | 出处 |
|------|----------|--------------|--------------|------|
| `SessionId` | `Uuid` | 字符串（手写 `Serialize`） | 先要字符串，再 `Uuid::parse_str`，失败 → serde 自定义错误 | `session_id.rs:79–96` |
| `ThreadId` | `Uuid` | 同上 | 同上 | `thread_id.rs:11–18` |
| `RolloutId` | **是 `ThreadId` 的别名**，不是新类型 | — | — | `thread_id.rs:23` |
| `ResponseItemId` | `String`（`#[serde(transparent)]`） | 裸字符串 | **不校验**：空串、`"x"`、数字以外的任何字符串都收 | `response_item_id.rs:11–17` |

要点：`ThreadId` 与 `SessionId` 是同一个 UUID 的两种角色，可以互转且不重新生成（`session_id.rs:55–64`）。
`ResponseItemId` 是"宽松接收、严格生产"——`new("msg")` 生成 `msg_<uuidv7>`，但读旧数据时不管格式。

### 2.2 `ContentItem`（`models.rs:882–913`）

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentItem {
    InputText  { text: String },                 // {"type":"input_text","text":...}
    OutputText { text: String,                   // {"type":"output_text",...}
                 annotations: Option<Vec<Value>>, logprobs: Option<Vec<Value>> },
    // 上游还有 InputImage / InputAudio —— 本课程不做
}
```

### 2.3 `ResponseItem`（`models.rs:1069–1341`）——发给模型的线格式

`#[serde(tag = "type", rename_all = "snake_case")]`，末尾还有 `#[serde(other)] Other`（1339–1340）。

| 变体 | 我们保留的字段 | 说明 |
|------|----------------|------|
| `Message` | `id?`, `role: String`, `content: Vec<ContentItem>`, `phase?` | 上游还有 `encrypted_content`/`status`/metadata，全部 Option，本课程不移植 |
| `Reasoning` | `id?`, `summary: Vec<_>`, `content?: Option<Vec<_>>` | `content` 上有 `skip_serializing_if = "should_serialize_reasoning_content"`（1121），即**默认不发** |
| `FunctionCall` | `id?`, `name: String`, `arguments: String`, `call_id: String`, `namespace?` | `arguments` 是**字符串化的 JSON**，不是对象（1155 注释里写明） |
| `FunctionCallOutput` | `id?`, `call_id?: Option<String>`, `output: FunctionCallOutputPayload` | `call_id` 在上游居然是可选的（1195） |
| `Other` | — | 未知 tag 落到这里而不是报错（1339） |

### 2.4 工具输出 payload（`models.rs:2364–2379`）

```rust
/// `body` 直接作为 wire 上 function_call_output.output 的值；
/// `success` 只是内部元数据。
pub struct FunctionCallOutputPayload { pub body: FunctionCallOutputBody, pub success: Option<bool> }

#[serde(untagged)]
pub enum FunctionCallOutputBody { Text(String), ContentItems(Vec<...>) }
```

关键：**这个结构体没有 derive Serialize/Deserialize**（2368 的 derive 列表里只有 Debug/Default/Clone/PartialEq/JsonSchema/TS）。
线上只有 `body`；`success` 不进 JSON。这是"内存结构 ≠ 线格式"的唯一样例，值得单独写测试。

### 2.5 `TurnItem`（`items.rs:47–75`）——给人看的条目

```rust
#[serde(tag = "type")]          // 注意：没有 rename_all
pub enum TurnItem { UserMessage(..), AgentMessage(..), Reasoning(..), CommandExecution(..), ... }
```

**tag 就是 Rust 变体名**：`{"type":"UserMessage"}`，不是 `user_message`。这条与 `ResponseItem` 的 snake_case 完全相反，
是这一课最容易被"顺手统一"掉的差异。

| 结构 | 我们保留的字段 | 出处 |
|------|----------------|------|
| `UserMessageItem` | `id: String`, `client_id?`, `content: Vec<UserInput>` | `items.rs:85–91` |
| `AgentMessageItem` | `id`, `content: Vec<AgentMessageContent>`, `phase?` | `items.rs:160–179` |
| `ReasoningItem` | `id`, `summary_text: Vec<String>`, `raw_content: Vec<String>`（`#[serde(default)]`） | `items.rs:200–206` |
| `CommandExecutionItem` | `id`, `command: Vec<String>`, `cwd: PathUri`, `status: CommandExecutionStatus`, `aggregated_output?`, `exit_code?`, `duration?` | `items.rs:253–289` |

`CommandExecutionStatus` = `in_progress | completed | failed | declined`（`items.rs:208–214`，snake_case）。

### 2.6 `Op` / `EventMsg`（`protocol.rs:694–941`、`1528–1700`）

- **`Op` 没有 serde derive**（702：只有 `derive_more::Debug` + `#[non_exhaustive]`），因为它带
  `reply: oneshot::Sender<CodexResult<TurnInputSubmission>>`（768）——进程内通道，不可能落盘。
  真正的输入变体叫 **`Op::TurnInput`**（765），不是 `UserTurn`。
- **`EventMsg` 有 serde**（1531–1533：`tag = "type"`, `rename_all = "snake_case"`），可以落盘、可以 `--json` 输出。

| Rust 变体 | 线格式 `type` | 反序列化额外接受 | 出处 |
|-----------|---------------|------------------|------|
| `Error` | `error` | — | 1537 |
| `TurnStarted` | **`task_started`** | `turn_started` | 1585 |
| `TurnComplete` | **`task_complete`** | `turn_complete` | 1594 |
| `AgentMessage` | `agent_message` | — | 1602 |
| `ExecCommandBegin` | `exec_command_begin` | — | 1650 |
| `ExecCommandEnd` | `exec_command_end` | — | 1658 |

`task_*` 是 v1 线格式，`turn_*` 只是兼容 alias —— 单看枚举名会写错，这是必须靠读属性才知道的事。

事件字段（我们保留的子集）：

| 事件 | 字段 | 备注 |
|------|------|------|
| `TurnStartedEvent` | `turn_id: String`, `started_at?: i64`, `model_context_window?: i64` | 上游另有 attribution/trace/`collaboration_mode_kind`（2437–2459） |
| `TurnCompleteEvent` | `turn_id`, `last_agent_message?`, `error?: ErrorEvent`, `started_at?`, `completed_at?`, `duration_ms?` | 结束独有字段全部 Option |
| `AgentMessageEvent` | `message: String`, `phase?: MessagePhase` | `MessagePhase = commentary \| partial_answer \| final_answer`（`models.rs:985–995`） |
| `ExecCommandBeginEvent` | `call_id`, `turn_id`, `started_at_ms: i64`, `command: Vec<String>`, `cwd: PathUri`, `source: ExecCommandSource` | `started_at_ms` 有 `#[serde(default)]` |
| `ExecCommandEndEvent` | `call_id`, `turn_id`, `completed_at_ms: i64`, `command`, `cwd`, `aggregated_output: String`, `exit_code: i32`, `duration: Duration`, `status: ExecCommandStatus` | `duration` 用 `#[ts(type="string")]`，线格式是字符串 |
| `ErrorEvent` | `message: String`, `codex_error_info?` | `misalignment` 带 `#[serde(skip)]`（2334）：**双向丢弃**，往返后必为 None |

`ExecCommandSource` = `agent | user_shell | unified_exec_startup | unified_exec_interaction`，默认 `agent`（3847–3859）。

## 3. 语义规则

**不变量**
1. `ResponseItem` 的 tag 是 snake_case；`TurnItem` 的 tag 是 PascalCase。两者不可统一。
2. 三种 ID 在 JSON 里都是字符串；`SessionId`/`ThreadId` 解析失败即错误，`ResponseItemId` 永不因格式报错。
3. 未知 `ResponseItem` tag → `Other`，不报错；`EventMsg` 未确认有同类兜底（我们**不**加，让它报错，因为事件流坏了应该立刻知道）。

**顺序**
- 一轮：`Op::TurnInput` 进入 → `task_started` → ……（`agent_message` / `exec_command_begin` → `exec_command_end`）→ `task_complete`。
- `exec_command_begin` / `exec_command_end` 靠 `call_id` 配对，且两端都带 `turn_id`。
- 顺序在上游是运行时行为（发射点在我们不读的 core 里），**这条来自 M7 的需要，不是从类型推出来的**——写代码时按这个顺序，等 M7 再验证。

**错误路径**
- `ErrorEvent.message` 必填（无 default）；`codex_error_info` 可缺。
- `TurnCompleteEvent.error` 是"整轮失败"的收口；`EventMsg::Error` 是过程错误。两者同一类型，但**不能假定同一次错误会同时出现在两处**（上游发射逻辑未读）。
- `misalignment` 双向 skip：`Some` 编码后消失，解码后是 `None`。

## 4. 必须写的测试（12 条）

| 测试名 | 断言什么 |
|--------|----------|
| `response_item_message_tag_is_snake_case` | `serde_json::to_value(Message{..})["type"] == "message"` |
| `turn_item_tag_keeps_pascal_case` | `TurnItem::UserMessage(..)` 的 `type` 是 `"UserMessage"` |
| `event_turn_started_serializes_as_task_started` | `type == "task_started"` |
| `event_accepts_turn_started_alias` | 输入 `{"type":"turn_started",...}` 能解成 `TurnStarted`，再编码变回 `task_started` |
| `response_item_unknown_tag_becomes_other` | `{"type":"brand_new"}` → `ResponseItem::Other`，不 panic |
| `content_item_round_trips_input_text` | 往返后 `text` 相等，`type` 仍是 `input_text` |
| `function_call_arguments_stay_string` | `arguments` 是 `"{\"path\":\"a\"}"` 字符串，不是对象 |
| `function_call_output_serializes_body_only` | 编码结果里没有 `success` 键（即使 `Some(true)`） |
| `function_call_output_decode_drops_success` | 解码后 `success == None`；且 `body` 两种形状（字符串 / 数组）都能解 |
| `session_id_rejects_non_uuid` | `"not-a-uuid"` 解码报错，错误信息里含该输入 |
| `response_item_id_accepts_any_string` | `""`、`"x"`、`"msg_1"` 都能解，且编码回原样 |
| `turn_complete_optional_fields_default_to_none` | 只给 `turn_id` 时，其余 Option 全 None，且编码后这些键不出现 |

错误路径 4 条已含在上面：非 UUID 解码、未知 `ResponseItem` tag（兜底不算错）、缺必填字段（`ErrorEvent` 少 `message` 报 "missing field"）、
以及 `FunctionCallOutputBody` 收到对象而不是字符串/数组。

## 5. 手写步骤

| Step | 做什么 | 大概行数 | 绿了之后 |
|------|--------|----------|----------|
| 1 | `ids.rs`：三个 newtype + `Display` + 手写 `Serialize`/`Deserialize`（先不写 `FromStr`） | ~110 | 跑 3 条 ID 测试 |
| 2 | 加 `uuid` 依赖（需裁决 R-008 先通过），补 `SessionId::new()` | ~20 | — |
| 3 | `items.rs`：`ContentItem` + `ResponseItem` 四个变体 + `Other` | ~140 | 跑 tag / round-trip 测试 |
| 4 | `FunctionCallOutputPayload`：手写 `Serialize`（只写 body）+ `Deserialize`（success=None） | ~60 | 跑 body-only 两条测试 |
| 5 | `turn.rs`：`TurnItem` + 四个条目结构（注意 PascalCase tag） | ~120 | 跑 PascalCase 测试 |
| 6 | `event.rs`：`EventMsg` + 六个事件 + `task_*` rename/alias | ~170 | 跑事件 tag 测试 |
| 7 | `op.rs`：`Op`（无 serde）+ `TurnInputRequest` 精简版 | ~60 | `./x.sh` 全绿 |

合计 ~680 行，落在 M1 预算（600–900）内。

Step 7 的一个主动简化：上游的 `Op::TurnInput` 带 `reply: oneshot::Sender<..>`（768），
我们的 M1 不引 tokio，所以写成 `Op::TurnInput { input: TurnInput }`，**并在 `notes/m1.md` 里记下这处偏离**。
等到 M7 需要应答时再补 reply——那时你才知道它为什么存在。

## 6. 三个常见误解

1. **「所有 tag 都转 snake_case」** → `TurnItem` 没 `rename_all`，tag 是 `UserMessage`。`items.rs:47–51`
2. **「事件名就是 type 值」** → `TurnStarted` 输出 `task_started`，`turn_started` 只是输入 alias。`protocol.rs:1585、1594`
3. **「结构体字段都会进 JSON」** → `FunctionCallOutputPayload` 根本没派生 serde，只有 `body` 上线，`success` 是内部字段；`ErrorEvent.misalignment` 被双向 skip。`models.rs:2368–2372`、`protocol.rs:2334–2337`

## 7. 验收问题（答不出不算过）

1. 为什么 `Op` 不能序列化？如果硬要序列化，哪个字段先炸？
2. `ResponseItem` 和 `TurnItem` 描述的是同一件事吗？各自给谁看？
3. `FunctionCallOutputPayload` 为什么不能直接 `derive(Serialize)`？手写时你要丢掉什么？
4. `task_started` 这个字符串从哪来？如果只读枚举名会写成什么？
5. `SessionId` 与 `ThreadId` 能不能合并成一个类型？合并会丢什么信息？
