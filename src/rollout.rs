//! S7 按行保存会话历史。每条完整记录对应一条 S1 消息。

use std::{fmt, io, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::Message;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutRecord {
    pub message: Message,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RolloutLoadResult {
    pub records: Vec<RolloutRecord>,
    /// 被跳过的物理行数，包括空白行、坏 JSON 和不符合记录结构的行。
    pub skipped_bad_lines: usize,
}

impl RolloutLoadResult {
    /// 按磁盘顺序恢复历史，包括已有的工具观测；重放本身没有外部副作用。
    pub fn replay(&self) -> Vec<Message> {
        todo!("按记录顺序重建 Message 历史，原样保留调用 ID、输出和错误标记")
    }
}

#[derive(Debug)]
pub enum RolloutError {
    Io(io::Error),
    Encode(serde_json::Error),
    EmptyFile,
}

impl fmt::Display for RolloutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "读写会话日志失败：{error}"),
            Self::Encode(error) => write!(f, "编码会话记录失败：{error}"),
            Self::EmptyFile => f.write_str("会话日志是空文件"),
        }
    }
}

impl std::error::Error for RolloutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::EmptyFile => None,
        }
    }
}

/// 一个会话独占的 JSONL 文件。构造时只保存路径。
pub struct RolloutStore {
    pub path: PathBuf,
}

impl RolloutStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn append(&self, _record: &RolloutRecord) -> Result<(), RolloutError> {
        todo!("首次追加时创建文件，每条记录编码为一行 JSON 并追加换行，写入失败向上传播")
    }

    pub fn load(&self) -> Result<RolloutLoadResult, RolloutError> {
        todo!("逐行读取、保序收集有效记录并累计坏行；零字节文件和 I/O 故障返回对应错误")
    }
}
