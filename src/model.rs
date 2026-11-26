//! Agent 内部核心数据模型。
//!
//! 设计原则：保持内核模型极简、干净、自洽，不包含特定供应商的历史兼容细节。
//! 外部 API（如 OpenAI Responses / Chat Completions）的线格式差异在 S6 客户端层做适配。

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 模型发起的一次工具调用请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// 用于把本次调用与后续的 `Message::ToolResult` 一一关联起来。
    pub call_id: String,
    /// 工具名称，例如 `"read_file"`、`"shell"`。
    pub name: String,
    /// 模型传入的参数（JSON 值）。
    pub arguments: Value,
}

/// 会话历史中的一条记录。
///
/// 在 Coding Agent 中，历史 `Vec<Message>` 是唯一的真相源：
/// 系统规则、用户提问、模型思考、工具调用以及工具观测结果全部平等地存放在这条序列里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Message {
    /// 系统提示词，定义 Agent 的角色、能力边界与项目规范。
    System(String),
    /// 用户的提问或指令。
    User(String),
    /// 模型的单步产出：可以只回复文本、只请求工具，或同时包含思考文本与工具调用。
    Assistant {
        text: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    /// 工具执行后的观测结果；即便执行失败也作为一条记录进历史，让模型能够自我修正。
    ToolResult {
        call_id: String,
        tool_name: String,
        output: String,
        is_error: bool,
    },
}

/// 模型单次采样（一次 `LlmClient::generate` 调用）返回的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTurn {
    pub text: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

impl ModelTurn {
    /// 构造一条纯文本回复（不包含工具调用，意味着模型准备结束本轮）。
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            tool_calls: Vec::new(),
        }
    }

    /// 构造一条包含工具调用的模型响应（可附带一段简短的思考/说明文本）。
    pub fn with_tools(text: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self { text, tool_calls }
    }
}

/// 一轮 `Agent::run_turn` 成功结束后的摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnOutcome {
    /// 最终返回给用户的文本。
    pub final_text: String,
    /// 本轮共向模型发起了多少次请求。
    pub steps: usize,
    /// 本轮累计执行了多少次工具调用。
    pub tool_calls_executed: usize,
}

/// Agent 主循环运行期的不可恢复错误。
///
/// 注意：单次工具执行失败不属于这里——工具失败会被写回历史交由模型处理。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    /// 模型客户端调用失败（如网络异常、鉴权失败、Mock 剧本耗尽）。
    Model(String),
    /// 超过最大步数仍未结束循环（防止模型死循环）。
    MaxStepsExceeded { max_steps: usize },
    /// 模型既未返回文本，也未请求任何工具。
    EmptyResponse,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(msg) => write!(f, "model error: {msg}"),
            Self::MaxStepsExceeded { max_steps } => {
                write!(f, "agent exceeded max steps ({max_steps})")
            }
            Self::EmptyResponse => write!(f, "model returned neither text nor tool calls"),
        }
    }
}

impl std::error::Error for AgentError {}
