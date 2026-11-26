//! S7 在一个写者中维护会话历史，通过 Op 收输入，通过 Event 输出进度。

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{Message, llm::AsyncLlmClient, rollout::RolloutStore, tools::ToolRegistry};

/// 进程内控制指令。序列化边界在 Event 与 RolloutRecord。
#[derive(Debug)]
pub enum Op {
    UserTurn(String),
    Interrupt,
}

/// 前端可以把每个事件编码为一行 JSON，即使文本自身含有换行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    TurnStarted,
    TextDelta {
        text: String,
    },
    ToolCallBegin {
        call_id: String,
        name: String,
    },
    ToolCallEnd {
        call_id: String,
        output: String,
        is_error: bool,
    },
    TurnCompleted {
        final_text: String,
    },
    TurnAborted,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    /// 从空历史开始，使用新的日志路径。
    New,
    /// 先读日志，再受理本次会话的输入。
    Resume,
}

pub struct SessionHandle {
    pub ops: mpsc::Sender<Op>,
    pub events: mpsc::UnboundedReceiver<Event>,
}

/// 启动所需的依赖。调用 spawn 后，模型、工具和日志都归会话任务所有。
pub struct Session<C> {
    pub client: C,
    pub tools: ToolRegistry,
    pub store: RolloutStore,
    pub max_steps: usize,
}

impl<C: AsyncLlmClient + 'static> Session<C> {
    /// 启动单写者，返回前端通道。恢复失败时先发 Error，再关闭事件流。
    pub fn spawn(self, _mode: SessionMode) -> SessionHandle {
        todo!("建立 Op/Event 通道，按模式初始化历史；活动轮与输入并行轮询，中断时取消该轮 token")
    }

    /// 当前轮独占 history。它负责流式回调、工具观测、持久化和唯一的终态事件。
    #[allow(dead_code, reason = "学习者在 spawn 的单写者循环中调用")]
    async fn run_turn(
        &mut self,
        _history: &mut Vec<Message>,
        _input: String,
        _cancel: CancellationToken,
        _events: &mpsc::UnboundedSender<Event>,
    ) {
        todo!("在步数上限内驱动异步采样和工具调用，按序记录消息；取消、完成或错误时只发一个终态")
    }
}
