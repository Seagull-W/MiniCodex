//! S6 将 Chat Completions 的 SSE 字节流转换为 S1 的 ModelTurn。

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;

use crate::tools::ToolSpec;
use crate::{Message, ModelTurn};

#[derive(Debug, PartialEq, Eq)]
pub enum LlmError {
    Config(String),
    Transport(String),
    Http { status: u16, body: String },
    Protocol(String),
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "模型配置错误：{msg}"),
            Self::Transport(msg) => write!(f, "请求传输失败：{msg}"),
            Self::Http { status, body } => write!(f, "HTTP {status}：{body}"),
            Self::Protocol(msg) => write!(f, "流式协议错误：{msg}"),
        }
    }
}

impl std::error::Error for LlmError {}

#[derive(Debug, PartialEq, Eq)]
pub enum SseEvent {
    Data(String),
    Done,
}

#[derive(Default)]
pub struct SseParser {
    pending: Vec<u8>,
    done: bool,
}

impl SseParser {
    pub fn buffered_bytes(&self) -> usize {
        self.pending.len()
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    /// 保留未完成帧的原始字节，空行提交一个事件。每条数据只产出一次。
    pub fn feed(&mut self, _chunk: &[u8]) -> Result<Vec<SseEvent>, LlmError> {
        todo!("跨块识别 LF/CRLF 和空行，在完整帧中合并 data 行、略过注释并识别 [DONE]")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamDelta {
    Text(String),
    ToolCall {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments: String,
    },
}

#[derive(Default)]
#[allow(dead_code, reason = "S6 的 push 和 finish 将读写工具片段")]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Default)]
#[allow(dead_code, reason = "S6 的 push 和 finish 将读写累积状态")]
pub struct ToolCallAssembler {
    text: Option<String>,
    calls: BTreeMap<usize, PartialToolCall>,
}

impl ToolCallAssembler {
    /// 文本按到达顺序累积。多个工具的片段可能交错，以 index 关联。
    pub fn push(&mut self, _delta: StreamDelta) {
        todo!("累积文本和每个 index 的调用 ID、名称及原始参数片段")
    }

    /// 消耗一次采样的全部状态，按 index 顺序产出工具，参数 JSON 在此时校验。
    pub fn finish(self) -> Result<ModelTurn, LlmError> {
        todo!("补全每个 ToolCall 并解析 arguments，缺失必需字段或非法 JSON 返回协议错误")
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// 回调随完整增量到达而调用。采样结束后一次性交付完整回复和用量。
pub trait AsyncLlmClient: Send {
    fn sample(
        &mut self,
        history: &[Message],
        tools: &[ToolSpec],
        on_delta: &mut (dyn FnMut(StreamDelta) + Send),
    ) -> impl Future<Output = Result<(ModelTurn, Usage), LlmError>> + Send;
}

pub struct OpenAiConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl OpenAiConfig {
    /// 密钥必填。服务地址和模型的缺省值见 S6 指南。
    pub fn from_env() -> Result<Self, LlmError> {
        todo!("读取 OPENAI_API_KEY、OPENAI_BASE_URL 和 MINICODEX_MODEL，并校验必填配置")
    }
}

#[allow(dead_code, reason = "S6 的 sample 将使用配置和可复用的 HTTP 客户端")]
pub struct OpenAiClient {
    config: OpenAiConfig,
    http: reqwest::Client,
}

impl OpenAiClient {
    pub fn new(config: OpenAiConfig, http: reqwest::Client) -> Self {
        Self { config, http }
    }
}

impl AsyncLlmClient for OpenAiClient {
    async fn sample(
        &mut self,
        _history: &[Message],
        _tools: &[ToolSpec],
        _on_delta: &mut (dyn FnMut(StreamDelta) + Send),
    ) -> Result<(ModelTurn, Usage), LlmError> {
        todo!("发送流式请求，按状态码有限次指数退避；逐块解码并回调，最后返回 ModelTurn 和用量")
    }
}
