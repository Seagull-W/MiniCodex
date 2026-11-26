//! S5 的同步配置加载、项目指令发现与系统提示词构造。

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    OnRequest,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub model: String,
    pub base_url: String,
    pub max_steps: usize,
    pub shell_timeout_secs: u64,
    pub approval_mode: ApprovalMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4o-mini".into(),
            base_url: "https://api.openai.com/v1".into(),
            max_steps: 8,
            shell_timeout_secs: 30,
            approval_mode: ApprovalMode::OnRequest,
        }
    }
}

/// 实际入选的项目指令，保留来源路径供提示词标注。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsDoc {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug)]
pub enum ContextError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Config {
        path: Option<PathBuf>,
        detail: String,
    },
}

impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}：{source}", path.display()),
            Self::Config {
                path: Some(path),
                detail,
            } => write!(f, "配置 {}：{detail}", path.display()),
            Self::Config { path: None, detail } => write!(f, "合并后的配置无效：{detail}"),
        }
    }
}

impl std::error::Error for ContextError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Config { .. } => None,
        }
    }
}

/// 递归合并两张表。同名键的任一侧为非表时，采用上层的整个值。
pub fn merge_toml(_base: toml::Value, _overlay: toml::Value) -> toml::Value {
    todo!("递归合并表并保留同级键，按上层值替换标量、数组及类型冲突项")
}

/// 默认配置、可选的用户/项目 TOML 文件和 CLI 指定的键依次覆盖；最后转成强类型。
pub fn load_layered_config(
    _user_path: Option<&Path>,
    _project_path: Option<&Path>,
    _cli: toml::Table,
) -> Result<AppConfig, ContextError> {
    todo!("按默认、用户、项目、CLI 的顺序合并；跳过缺失文件，报告其他读取或解析错误")
}

/// 从最近的 Git 根向下收集，每层选一份指令，所有正文共享一个 UTF-8 字节预算。
pub fn discover_agents_md(
    _start_dir: &Path,
    _max_bytes: usize,
) -> Result<Vec<AgentsDoc>, ContextError> {
    todo!("确定根目录并逐层选择 override 或普通文件，在共享预算内返回有序来源与正文")
}

/// 将基础提示词、工作目录、按顺序标注来源的项目指令拼为一条系统消息的正文。
pub fn build_system_prompt(_base: &str, _docs: &[AgentsDoc], _cwd: &Path) -> String {
    todo!("保留基础提示词，写入工作目录，再按给定顺序追加各份指令的来源路径和正文")
}
