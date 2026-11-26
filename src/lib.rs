//! MiniCodex 核心库：从最小 ReAct 主循环逐步演进为完整的 Coding Agent。

#![forbid(unsafe_code)]

pub mod agent;
pub mod client;
pub mod context;
pub mod llm;
pub mod model;
pub mod patch;
pub mod policy;
pub mod rollout;
pub mod session;
pub mod tools;

pub use agent::Agent;
pub use client::{FnToolExecutor, LlmClient, ScriptedClient, ToolExecutor};
pub use model::{AgentError, Message, ModelTurn, ToolCall, TurnOutcome};
