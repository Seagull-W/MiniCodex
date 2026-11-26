//! 模型客户端与工具执行器抽象，以及用于 S1 测试和演示的轻量实现。

use std::collections::VecDeque;

use crate::model::{AgentError, Message, ModelTurn, ToolCall};

/// 模型客户端抽象：给定完整历史切片，生成模型的下一步输出。
///
/// S1 使用 `ScriptedClient` 驱动确定性测试；S6 会实现真正的 OpenAI-compatible 流式客户端。
pub trait LlmClient {
    fn generate(&mut self, history: &[Message]) -> Result<ModelTurn, AgentError>;
}

/// 工具执行抽象：接收一次工具调用，返回输出文本或错误文本。
///
/// S2 会将其演进为基于 `Tool` trait 和 JSON Schema 的动态工具注册表。
pub trait ToolExecutor {
    fn execute(&mut self, call: &ToolCall) -> Result<String, String>;
}

/// 剧本化的 Mock 模型客户端：按预定顺序返回响应，并记录每次收到的历史快照。
///
/// 通过 `recorded_requests()` 可以在测试中断言：每一步发给模型的上下文是否完全符合预期。
#[derive(Debug, Default)]
pub struct ScriptedClient {
    responses: VecDeque<Result<ModelTurn, AgentError>>,
    recorded_requests: Vec<Vec<Message>>,
}

impl ScriptedClient {
    pub fn new(responses: impl IntoIterator<Item = Result<ModelTurn, AgentError>>) -> Self {
        Self {
            responses: responses.into_iter().collect(),
            recorded_requests: Vec::new(),
        }
    }

    /// 返回迄今为止每次 `generate` 调用收到的历史快照。
    pub fn recorded_requests(&self) -> &[Vec<Message>] {
        &self.recorded_requests
    }
}

impl LlmClient for ScriptedClient {
    fn generate(&mut self, history: &[Message]) -> Result<ModelTurn, AgentError> {
        self.recorded_requests.push(history.to_vec());
        self.responses.pop_front().unwrap_or_else(|| {
            Err(AgentError::Model(
                "scripted client has no more responses".into(),
            ))
        })
    }
}

/// 用闭包快速构造 `ToolExecutor` 的适配器，方便测试与演示。
pub struct FnToolExecutor<F> {
    handler: F,
}

impl<F> FnToolExecutor<F>
where
    F: FnMut(&ToolCall) -> Result<String, String>,
{
    pub fn new(handler: F) -> Self {
        Self { handler }
    }
}

impl<F> ToolExecutor for FnToolExecutor<F>
where
    F: FnMut(&ToolCall) -> Result<String, String>,
{
    fn execute(&mut self, call: &ToolCall) -> Result<String, String> {
        (self.handler)(call)
    }
}
