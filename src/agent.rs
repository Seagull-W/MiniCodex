//! S1 核心模块：Coding Agent 的 ReAct 主循环。
//!
//! 详细设计与练习指引见 `docs/stages/S1-agent-loop.md`。

use crate::client::{LlmClient, ToolExecutor};
use crate::model::{AgentError, Message, ToolCall, TurnOutcome};

/// 维护单一会话历史并驱动「模型 → 工具 → 观测 → 模型」循环的最小 Agent。
pub struct Agent<C, T> {
    client: C,
    tools: T,
    history: Vec<Message>,
    max_steps: usize,
}

impl<C, T> Agent<C, T> {
    /// 创建一个新的 Agent。
    ///
    /// - 若提供了 `system_prompt`（`Some(prompt)`），应将其作为首条 `Message::System` 放入 `history`。
    /// - `max_steps` 限制单次 `run_turn` 最多向模型发起多少次采样请求。
    pub fn new(_client: C, _tools: T, _system_prompt: Option<String>, _max_steps: usize) -> Self {
        todo!("初始化 Agent；如果提供了 system_prompt，将其作为首条 Message::System 放进 history")
    }

    /// 只读访问当前会话的完整历史记录。
    pub fn history(&self) -> &[Message] {
        &self.history
    }

    /// 只读访问底层模型客户端（方便在测试中检查发送过的请求快照）。
    pub fn client(&self) -> &C {
        &self.client
    }

    /// 只读访问工具执行器。
    pub fn tools(&self) -> &T {
        &self.tools
    }

    /// 返回当前配置的单轮最大循环步数。
    pub fn max_steps(&self) -> usize {
        self.max_steps
    }
}

impl<C: LlmClient, T: ToolExecutor> Agent<C, T> {
    /// 运行一轮完整的用户对话，直到模型给出最终文本或触发错误/步数上限。
    pub fn run_turn(&mut self, _user_input: impl Into<String>) -> Result<TurnOutcome, AgentError> {
        todo!(
            "实现 ReAct 主循环：\n\
             1. 将 user_input 作为 Message::User 追加进 self.history\n\
             2. 在 1..=self.max_steps 范围内循环调用 self.client.generate(&self.history)?\n\
             3. 若 tool_calls 为空：text 为 None 时返回 EmptyResponse；否则写入 Message::Assistant 并返回 TurnOutcome\n\
             4. 若 tool_calls 非空：先写入 Message::Assistant，再调用 self.execute_tool_calls 写回结果并继续循环\n\
             5. 步数耗尽仍未结束时返回 MaxStepsExceeded"
        )
    }

    /// 按顺序执行一批工具调用，将每次结果（成功或失败）包装为 `Message::ToolResult` 追加进历史。
    #[allow(dead_code)] // 在 run_turn 中调用后即可删除这行
    fn execute_tool_calls(&mut self, _calls: &[ToolCall]) -> usize {
        todo!(
            "遍历 calls，调用 self.tools.execute(call)，将 Ok/Err 转为 Message::ToolResult \
             (is_error 分别为 false/true) 追加进 self.history，返回执行次数"
        )
    }
}
