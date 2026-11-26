# S1：最小 Agent 主循环（ReAct / Turn Loop）

这是整个 MiniCodex 的心脏。我们先跳过所有外围细节（配置、流式 HTTP、持久化），用不到 100 行核心代码跑通 **Coding Agent 最本质的循环**。

---

## 1. 为什么 Coding Agent 的本质是一个循环？

普通聊天机器人是「**一问一答**」：你发一句话，模型回一段字，结束。

Coding Agent（包括 Codex、Claude Code）则是「**思考 → 调工具 → 观察结果 → 再思考**」的闭环（ReAct Loop）：

```text
用户输入 ("检查 Cargo.toml 里的项目名")
   │
   ▼
写入 history: [System, User]
   │
   ▼
┌──────────▶ 调用模型 client.generate(&history)
│                  │
│          ┌───────┴───────┐
│      没有 tool_calls    有 tool_calls
│          │               │
│          ▼               ▼
│    写入 Assistant    写入 Assistant(含 tool_calls)
│    返回 final_text       │
│                          ▼
│                    依次执行工具 tools.execute(&call)
│                    成功/失败都写入 Message::ToolResult
│                          │
└──────────────────────────┘  (继续循环，直到模型不再要工具或触发 max_steps)
```

### 这个循环里最关键的 3 个设计不变量

1. **历史（`Vec<Message>`）是唯一的真相源**
   发给模型的不是拼出来的字符串，而是完整的消息列表。工具的执行结果之所以能被模型看到，**不是靠什么特殊开关，仅仅是因为它作为 `Message::ToolResult` 被追加进了 `history`**。下一轮调用 `client.generate(&self.history)` 时，模型自然就看到了上一步工具的产出。
2. **工具报错不是程序崩溃，而是喂回给模型的「观测」**
   当 `tools.execute(&call)` 返回 `Err(msg)`（比如模型传错了文件路径、命令返回非零退出码）时，**绝对不要直接用 `?` 中断整个 `run_turn`**。正确做法是构造 `Message::ToolResult { is_error: true, output: msg, .. }` 写进历史，让模型在下一步看到报错并**自我修正**。
   相反，如果是 `client.generate(...)` 返回 `Err(e)`（模型服务本身挂了），这才属于不可恢复的故障，应当立即用 `?` 向外传播。
3. **循环必须有硬上限（`max_steps`）和空响应保护**
   现实里模型可能陷入死循环（反复调同一个出错的工具）或返回空响应。`max_steps` 兜住死循环，`EmptyResponse` 兜住既无文本又无工具调用的异常输出。

> **可选·上游 Codex 对照阅读**：
> 在 `/home/seagull/codex-src/codex-rs/core/src/session/turn.rs`（约 590–660 行）中，Codex 的主循环遵循完全相同的结构：取历史快照 → 发请求 → 遇到工具调用则执行并把 `FunctionCallOutput` 写回历史 → 以 `needs_follow_up` 决定是否继续下一 step。只是上游叠加了压缩、插话（steer）和流式事件，我们在后续阶段再逐层加上。

---

## 2. 本阶段你要练的 Rust 核心能力

- **代数数据类型（`enum`）与模式匹配**：用 `Message` 的四个变体表达对话全貌，用 `AgentError` 精确区分三类失败原因。
- **泛型与 Trait 约束**：`Agent<C, T>` 通过 `C: LlmClient` 和 `T: ToolExecutor` 解耦外部依赖——S1 用 `ScriptedClient` 做确定性测试，S2 可以把注册了 `ShellTool` 的 `ToolRegistry` 传给同一个同步 Agent。S6 在新模块中引入异步 HTTP/SSE 客户端，由 S7 的会话驱动。
- **所有权与借用**：
  - `run_turn(&mut self, user_input: impl Into<String>)` 接收所有权并移入 `history`；
  - `client.generate(&self.history)` 只借用不可变切片 `&[Message]`，不转移所有权；
  - 在 `run_turn` 中既要读取 `tool_calls` 又要把它存进 `Message::Assistant`，注意如何安排 `clone` 或先后顺序来满足借用检查器。
- **两类错误的分层处理**：`Result<ModelTurn, AgentError>` 用 `?` 传播；`Result<String, String>`（工具结果）用 `match` 转成数据写回历史。

---

## 3. 代码结构与本阶段任务

已经为你准备好的部分：
- [src/model.rs](../../src/model.rs)：`Message`、`ToolCall`、`ModelTurn`、`TurnOutcome`、`AgentError`（干净极简，无历史兼容包袱）。
- [src/client.rs](../../src/client.rs)：`LlmClient` / `ToolExecutor` trait，以及用于测试和演示的 `ScriptedClient`、`FnToolExecutor`。
- [src/main.rs](../../src/main.rs)：一个完整的演示场景（模拟模型先猜错文件路径、看到报错后自纠、再读到 `Cargo.toml` 给出答案）。
- [tests/s1_agent_loop.rs](../../tests/s1_agent_loop.rs)：9 条覆盖正常路径、多工具保序、错误自纠、模型错误传播、死循环保护、空响应与多轮会话的行为测试。

**你需要手写的部分在 [src/agent.rs](../../src/agent.rs) 的三处 `todo!()`**：

1. `Agent::new`
   - 创建 `Vec::new()` 作为初始历史；若 `system_prompt` 为 `Some(prompt)`，将其作为第一条 `Message::System(prompt)` 放入 `history`。
2. `Agent::execute_tool_calls`
   - 遍历 `calls`，逐个调用 `self.tools.execute(call)`；
   - `Ok(output)` → `Message::ToolResult { call_id, tool_name, output, is_error: false }`；
   - `Err(err)` → `Message::ToolResult { call_id, tool_name, output: err, is_error: true }`；
   - 全部按顺序 `push` 进 `self.history`，返回本次执行的工具次数。
3. `Agent::run_turn`
   - 先把 `Message::User(user_input.into())` 追加进 `self.history`；
   - 在 `1..=self.max_steps` 范围内循环（注意 `max_steps == 0` 应直接返回 `MaxStepsExceeded`）：
     1. 调用 `self.client.generate(&self.history)?`；
     2. 若 `tool_calls` 为空：
        - 若 `text` 为 `None`，返回 `Err(AgentError::EmptyResponse)`；
        - 若 `text` 为 `Some(final_text)`，将 `Message::Assistant { text: Some(final_text.clone()), tool_calls: Vec::new() }` 写入 `self.history`，返回 `Ok(TurnOutcome { final_text, steps, tool_calls_executed })`；
     3. 若 `tool_calls` 非空：
        - 先把 `Message::Assistant { text, tool_calls: ... }` 写入 `self.history`（**必须在 ToolResult 之前**，否则模型看到的因果顺序会颠倒）；
        - 调用 `self.execute_tool_calls(...)` 累加 `tool_calls_executed`；
        - 进入下一步循环；
   - 如果用完 `self.max_steps` 仍未拿到最终文本，返回 `Err(AgentError::MaxStepsExceeded { max_steps: self.max_steps })`。

---

## 4. 验收方式

写完 [src/agent.rs](../../src/agent.rs) 后执行：

```bash
# 1. 检查全仓格式和 Clippy，运行库测试与 S1 的 9 条验收测试
./x.sh s1

# 2. 亲眼看一次带“工具报错自纠”的完整 Agent 循环
cargo run
```

全绿之后告诉我，我们一起 review 你的实现，顺便聊两个问题再进入 **S2（工具注册表与真实 Shell 执行）**：
1. 为什么 `Message::Assistant { tool_calls }` 必须写在 `Message::ToolResult` 之前？
2. 如果让你把工具执行改成「遇到未注册工具就报错给模型，但遇到用户按 Ctrl+C 就中止循环」，`ToolExecutor` 的返回类型应该怎么设计？
