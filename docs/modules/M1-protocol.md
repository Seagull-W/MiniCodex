# 第一步：给会话一套语言

这一块叫 `mc-protocol`，五个文件、大约七百行。它是整条链路里最小的模块之一，但要最早写——
因为后面每一个部件都在说它的方言：循环要 match 事件、请求要装 item、落盘要序列化。
先把语言定下来，后面的代码才不会一边写一边改字段。

写完之后你应该能做到：拿一段手写的 JSON，解析成事件、再编码回去，得到和上游一样的字符串。
这件事听起来琐碎，但它检验的是你对"三种语言分工"的理解——而不是记忆力。

## 一、这块在链路的哪里

codex 内部同时流通三种东西，长得像但用途完全不同。

发给模型的是 `ResponseItem`：一条消息、一次推理、一个工具调用、一次工具输出。它是请求体里的数组元素。
给界面看的是 `TurnItem`：同一件事的展示形态，带稳定的 id，方便前端增量更新。
在进程内传递的是 `Op` 和 `EventMsg`：前者从外面进来，后者往外走。

用户打一句话，三种东西会在一秒内全部出现：

```
你输入 "修一下 README"
   → Op::TurnInput            （进队列）
   → ResponseItem::Message    （作为 user 消息进历史，随请求发给模型）
   → EventMsg::TurnStarted    （告诉界面：开始干活了）
   → TurnItem::UserMessage    （界面上出现你那条消息的卡片）
```

看懂这条链，就知道为什么后面写循环时，"记历史"和"发事件"总是成对出现——
它们不是同一件事，只是一起发生。

## 二、上游在哪，怎么读

上游 `codex-rs/protocol` 有三万四千行、七十多个模块，绝大部分与我们无关。要读的只有六段，总共约六百行：

先看三个 ID 的小文件（`session_id.rs`、`thread_id.rs`、`response_item_id.rs`，每个不到一百二十行）。
它们示范了如何给一个 newtype 手写 serde，是本模块唯一需要动脑的地方。

再看 `models.rs` 的三段：`ContentItem`（882–913）、`ResponseItem`（1069–1341）、工具输出 payload（2364–2379）。

然后 `items.rs` 的 47–293 行，`TurnItem` 与四个条目结构都在里面。

最后 `protocol.rs`，它有 6803 行，只看四段：`Op`（694–941）、`EventMsg`（1528–1700）、
以及 `ErrorEvent`（2329–2345）、`TurnCompleteEvent` / `TurnStartedEvent`（2407–2465）、
`AgentMessageEvent`（2781–2793）、两个 exec 事件（3870–3945）。**按区间读，别通读。**

## 三、三个 ID：手写 serde 的三个理由

`SessionId` 里面是一个 `Uuid`，但它在 JSON 里是一个裸字符串。所以上游没有 derive，
而是手写 `Serialize`（`collect_str`）和 `Deserialize`（先取字符串，再 `Uuid::parse_str`，失败时用
`serde::de::Error::custom` 报错，`session_id.rs:79-96`）。手写的好处是错误信息能带上原始输入。

`ThreadId` 一模一样，但语义不同：它标识"一条会话"，`SessionId` 标识"一次运行"。
两者可以互转，转换时不重新生成 UUID（`session_id.rs:55-64`）——这条容易写错成"转一次生成一个新 id"。

`RolloutId` 不是第四个类型，只是 `type RolloutId = ThreadId`（`thread_id.rs:23`）。
这是一个可以学的取舍：新类型能防串用，别名省掉一次转换；上游在这里选了后者。

`ResponseItemId` 反过来，是"宽松接收、严格生产"：内部是裸 `String`，带 `#[serde(transparent)]`
（`response_item_id.rs:11-17`）。`new("msg")` 会生成 `msg_<uuidv7>`，但反序列化时不管格式，
空串、`"x"` 都收。原因是它要读历史数据，而历史里什么都可能有。

## 四、发给模型的 item

`ResponseItem` 顶部是 `#[serde(tag = "type", rename_all = "snake_case")]`，所以线上是
`{"type":"message"}` 这种形状（`models.rs:1069-1070`）。我们只做五种变体。

`Message` 装一个 `role` 和一个 `ContentItem` 数组。`ContentItem` 自己也带同样的 tag 和
`rename_all`（882–883），所以是 `input_text` / `output_text`。上游还有图片和音频，我们不做。

`Reasoning` 有个细节：`content` 字段的序列化条件是 `should_serialize_reasoning_content`（1121），
意思是"推理正文默认不发出去，只发摘要"。这个函数体我们没读，但行为可以先用测试锁死：
`content` 是 `None` 时序列化结果里没有这个键。

`FunctionCall` 里最容易写错的是 `arguments`——它是**字符串化的 JSON**，不是对象（1155 附近的注释写明）。
模型返回的就是一段文本，我们不做解析。

`FunctionCallOutput` 的 `output` 字段是 `FunctionCallOutputPayload`（2364–2379），这里藏着本模块唯一的
"内存结构 ≠ 线格式"：那个结构体**没有 derive Serialize/Deserialize**，注释说得很直接——
`body` 直接作为线上的值，`success` 只是内部元数据。也就是说，编码时只写 `body`，
解码时 `success` 永远是 `None`。这解释了为什么它需要手写，也解释了为什么我们单独给它写两条测试。

最后是 `Other`：`ResponseItem` 末尾有 `#[serde(other)] Other`（1339–1340）。
未知的 `type` 不会报错，而是落进这个变体。这是"读旧数据"的策略，不是疏忽。

## 五、给界面看的条目，为什么是另一套

`TurnItem` 的第一行就有陷阱：它只有 `#[serde(tag = "type")]`，**没有 `rename_all`**
（`items.rs:47-51`）。所以 tag 是 Rust 变体名本身，`{"type":"UserMessage"}`，不是 `user_message`。

同一份数据，给模型的那套转成 snake_case，给界面的这套保持 PascalCase。这不是历史遗留的懒，
而是因为 `TurnItem` 是界面契约：前端按这些字符串做增量渲染，改名等于破坏兼容。

四个条目结构里，`UserMessageItem` 和 `AgentMessageItem` 都带 `id`（给界面做 key），
`ReasoningItem` 多一个 `raw_content`（`#[serde(default)]`，容忍缺失），
`CommandExecutionItem` 里有 `status`、`exit_code`、`duration`。
`CommandExecutionStatus` 是 snake_case 的四个值：`in_progress`、`completed`、`failed`、`declined`
（`items.rs:208-214`）。

## 六、事件能落盘，Op 不能

`EventMsg` 带完整的 serde（`protocol.rs:1531-1533`：`tag="type"`, `rename_all="snake_case"`），
因为事件要写进 rollout、要通过 `--json` 输出。我们只做六个变体。

其中两个的名字需要背下来：`TurnStarted` 线上的 `type` 是 **`task_started`**，
`TurnComplete` 是 **`task_complete`**（`protocol.rs:1585`、`:1594`）。
注释写得很清楚——v1 线格式用 `task_*`，`turn_*` 只是给新格式留的 alias。
只读枚举名的人会写错，而且写错之后测试是绿的（因为枚举内部一致），只有和真数据对照才会发现。

事件字段还有一个规律：开始事件和结束事件各带一半信息。`TurnStartedEvent` 有 `started_at`，
`TurnCompleteEvent` 有 `last_agent_message`、`error`、`completed_at`、`duration_ms`，且全是 Option
（`protocol.rs:2407-2433`）。`ErrorEvent.misalignment` 则带 `#[serde(skip)]`（2334-2337），
双向丢弃：编码时消失，解码后是 `None`。

`Op` 完全相反：它只有 `derive_more::Debug` 和 `#[non_exhaustive]`（702-705），一个 serde 都没有。
原因在字段里——`Op::TurnInput` 带 `reply: oneshot::Sender<CodexResult<..>>`（768），
这是进程内的应答通道，序列化它没有意义。

我们的 M1 里不引 tokio，所以 `Op::TurnInput` 先只保留输入本身，去掉 `reply`。
这处偏离要写进 `notes/m1.md`：等 M7 需要"提交后得到应答"时再补，那时你才会知道它为什么存在。

## 七、先写这十二个测试

测试是这一章唯一能证明"你看懂了"的东西。名字按行为起，断言具体值。

| 测试 | 断言 |
|------|------|
| `response_item_message_tag_is_snake_case` | `type` 等于 `"message"` |
| `turn_item_tag_keeps_pascal_case` | `TurnItem::UserMessage` 的 `type` 是 `"UserMessage"` |
| `event_turn_started_serializes_as_task_started` | `type` 等于 `"task_started"` |
| `event_accepts_turn_started_alias` | 输入 `turn_started` 能解，再编码变回 `task_started` |
| `response_item_unknown_tag_becomes_other` | `{"type":"brand_new"}` 解成 `Other`，不 panic |
| `content_item_round_trips_input_text` | 往返后 `text` 不变，`type` 仍是 `input_text` |
| `function_call_arguments_stay_string` | `arguments` 是字符串，不是对象 |
| `function_call_output_serializes_body_only` | 即使 `success = Some(true)`，JSON 里也没有这个键 |
| `function_call_output_decode_drops_success` | 解码后 `success` 是 `None`；body 的两种形状都能解 |
| `session_id_rejects_non_uuid` | `"not-a-uuid"` 解码报错 |
| `response_item_id_accepts_any_string` | `""`、`"x"`、`"msg_1"` 都能解，编码回原样 |
| `turn_complete_optional_fields_default_to_none` | 只给 `turn_id` 时其余为 `None`，编码后这些键缺席 |

错误路径包含在里面了：非 UUID 解码、缺必填字段（`ErrorEvent` 少 `message` 报 missing field）、
body 收到对象而不是字符串/数组。写完这十二条，你的协议层基本就对了。

## 八、七步，每步都能编译

| 步 | 做什么 | 行数 |
|----|--------|------|
| 1 | `ids.rs`：三个 newtype、`Display`、手写 `Serialize` / `Deserialize` | ~110 |
| 2 | 接上 `uuid` 依赖（已按 R-008 批准），补 `SessionId::new()` | ~20 |
| 3 | `items.rs`：`ContentItem` 与 `ResponseItem` 的四个变体 + `Other` | ~140 |
| 4 | `FunctionCallOutputPayload`：手写序列化，只写 body | ~60 |
| 5 | `turn.rs`：`TurnItem` 与四个条目结构，注意 PascalCase | ~120 |
| 6 | `event.rs`：`EventMsg` 六个变体 + `task_*` 的 rename 与 alias | ~170 |
| 7 | `op.rs`：`Op`（不加 serde）与精简的 `TurnInput` | ~60 |

合计约六百八十行，在预算内。第四步值得慢一点：手写 serde 是这一章唯一真正需要查文档的地方，
写完你会明白"哪些字段在线格式里根本不存在"。

## 九、三个坑

第一个坑是那个看不见的 `rename_all`。`TurnItem` 没有它，tag 就是变体名。写完先跑那条 PascalCase 测试，
不要凭直觉统一成 snake_case。

第二个坑是 `task_started`。它是上游为了兼容 v1 线格式留下的名字，`turn_started` 只是输入侧的别名。
测试要同时覆盖"编码出来是 task_started"和"能读进 turn_started"两个方向。

第三个坑是"结构体字段都会进 JSON"。`FunctionCallOutputPayload` 没有派生 serde，`success` 永远不上线；
`ErrorEvent.misalignment` 被双向跳过。这两处是上游刻意的不对称，不是遗漏。

## 十、做完给我看什么

跑一次 `./x.sh`，把末尾输出贴出来；把 `notes/m1.md` 里的三行笔记（这一段在链路里的位置、
它的输入输出、最容易误解的一点）发我；然后回答下面五个问题。

我会挑两条测试读断言，再随机抽一条行为回上游源码核对。都过了，就把 R1 标记为完成，开始写 R2 的路书。

## 十一、验收问题

1. 为什么 `Op` 不能序列化？如果硬要序列化，哪个字段先炸？
2. `ResponseItem` 和 `TurnItem` 描述的是同一件事吗？各自给谁看？
3. `FunctionCallOutputPayload` 为什么不能直接 `derive(Serialize)`？手写时你丢掉了什么？
4. `task_started` 这个字符串从哪来？只读枚举名会写成什么？
5. `SessionId` 和 `ThreadId` 能不能合并成一个类型？合并会丢什么信息？
