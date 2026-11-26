//! S2 同步工具。注册表连接 S1 的执行入口和各工具自己的参数契约。

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};

use crate::{ToolCall, ToolExecutor};

/// 模型可以看到的工具描述，parameters 是 JSON Schema。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// 工具拥有自己的配置，通过同步调用返回一次可写入历史的观测。
pub trait Tool: Send {
    fn spec(&self) -> ToolSpec;
    fn call(&self, args: Value) -> Result<String, String>;
}

/// 每个名称只绑定一个工具。同一实例既提供 Schema 也处理调用。
#[derive(Default)]
pub struct ToolRegistry {
    #[allow(dead_code)] // 完成注册与派发练习后读取此表。
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 以工具声明的名称注册，重复注册时用新实例替换旧实例。
    pub fn register(&mut self, _tool: Box<dyn Tool>) {
        todo!("读取工具 spec 中的名称，将工具的所有权移入注册表")
    }

    /// 按名称升序导出完整描述，使相同的工具集产生稳定的模型请求。
    pub fn specs(&self) -> Vec<ToolSpec> {
        todo!("收集每个已注册工具的完整 spec，按名称升序返回")
    }
}

impl ToolExecutor for ToolRegistry {
    fn execute(&mut self, _call: &ToolCall) -> Result<String, String> {
        todo!("按名称查找工具，传入此次调用的 JSON 参数，保留结果或返回含未知名称的错误")
    }
}

/// 返回字节数不超过 max_bytes 的最长完整字符前缀。
pub fn truncate_utf8_output(_s: &str, _max_bytes: usize) -> String {
    todo!("在预算内保留最长前缀，预算落在 UTF-8 字符中间时退到字符边界")
}

/// 在指定目录内运行一个程序。argv 首项为程序名，其余各项直接作为参数传入。
#[derive(Debug, Clone)]
pub struct ShellTool {
    pub cwd: PathBuf,
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

impl ShellTool {
    pub fn new(cwd: impl Into<PathBuf>, timeout: Duration, max_output_bytes: usize) -> Self {
        Self {
            cwd: cwd.into(),
            timeout,
            max_output_bytes,
        }
    }
}

impl Tool for ShellTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "shell".into(),
            description: "在工作目录执行一个程序，并捕获 stdout 和 stderr".into(),
            parameters: json!({
                "type": "object",
                "properties": {"argv": {"type": "array", "minItems": 1, "items": {"type": "string"}}},
                "required": ["argv"]
            }),
        }
    }

    fn call(&self, _args: Value) -> Result<String, String> {
        todo!(
            "校验 argv 并直接启动程序，在时限内收集两条输出流并按字节预算截断。超时终止并回收子进程，非零退出返回含退出状态和诊断的错误"
        )
    }
}
