//! S3 命令分词、前缀规则和执行前审批。

use std::fmt;

use serde_json::{Value, json};

use crate::tools::{ShellTool, Tool, ToolSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Ask,
    Deny { reason: String },
}

/// 从 argv 的第一个参数开始，逐项完整匹配非空前缀。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixRule {
    pub prefix: Vec<String>,
    pub decision: Decision,
}

#[derive(Debug, Clone, Default)]
pub struct ExecPolicy {
    pub rules: Vec<PrefixRule>,
}

impl ExecPolicy {
    pub fn new(rules: Vec<PrefixRule>) -> Self {
        Self { rules }
    }

    pub fn evaluate(&self, _argv: &[String]) -> Decision {
        todo!("匹配非空参数前缀，任一 Deny 优先于所有 Allow，余下情形返回 Ask")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    EmptyCommand,
    UnclosedQuote(char),
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCommand => write!(f, "命令缺少程序名"),
            Self::UnclosedQuote(quote) => write!(f, "命令中有未闭合的引号 {quote}"),
        }
    }
}

impl std::error::Error for PolicyError {}

/// 空白分隔参数，成对的单、双引号分组并保留空参数，其他字符按字面值传入。
pub fn split_command(_cmd: &str) -> Result<Vec<String>, PolicyError> {
    todo!("分词并移除分组引号，保留引号内空白。缺少程序名或引号未闭合时返回对应错误")
}

pub trait ApprovalHandler: Send {
    fn approve(&self, argv: &[String]) -> bool;
}

impl<F: Fn(&[String]) -> bool + Send> ApprovalHandler for F {
    fn approve(&self, argv: &[String]) -> bool {
        self(argv)
    }
}

/// 注册时使用此包装器取代裸 ShellTool，模型提交 command 字符串。
#[allow(dead_code)] // 完成 call 练习后读取三个组件。
pub struct PolicyGuard<A> {
    shell: ShellTool,
    policy: ExecPolicy,
    approval: A,
}

impl<A: ApprovalHandler> PolicyGuard<A> {
    pub fn new(shell: ShellTool, policy: ExecPolicy, approval: A) -> Self {
        Self {
            shell,
            policy,
            approval,
        }
    }
}

impl<A: ApprovalHandler> Tool for PolicyGuard<A> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "shell".into(),
            description: "按前缀规则和审批结果运行一条命令".into(),
            parameters: json!({
                "type": "object",
                "properties": {"command": {"type": "string"}},
                "required": ["command"]
            }),
        }
    }

    fn call(&self, _args: Value) -> Result<String, String> {
        todo!(
            "提取 command 并分词，先评估策略，Ask 时审批一次。获准后才把同一 argv 传给 ShellTool，拒绝或输入错误直接返回错误"
        )
    }
}
