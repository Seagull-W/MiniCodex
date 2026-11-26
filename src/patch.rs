//! S4 的 Search/Replace 补丁、工作目录边界与工具接口。

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::tools::{Tool, ToolSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    pub files: Vec<FilePatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilePatch {
    AddFile { path: PathBuf, content: String },
    UpdateFile { path: PathBuf, hunks: Vec<Hunk> },
    DeleteFile { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub search: String,
    pub replace: String,
}

/// 整批提交成功的文件数，每个文件只计一次。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PatchSummary {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
}

#[derive(Debug)]
pub enum PatchError {
    Syntax { line: usize, detail: String },
    MissingFile(PathBuf),
    AlreadyExists(PathBuf),
    DuplicatePath(PathBuf),
    ContextNotFound(PathBuf),
    AmbiguousContext(PathBuf),
    PathOutsideRoot(PathBuf),
    Io { path: PathBuf, source: io::Error },
}

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { line, detail } => write!(f, "补丁第 {line} 行语法错误：{detail}"),
            Self::MissingFile(path) => write!(f, "文件不存在：{}", path.display()),
            Self::AlreadyExists(path) => write!(f, "新增路径已存在：{}", path.display()),
            Self::DuplicatePath(path) => write!(f, "重复的文件路径：{}", path.display()),
            Self::ContextNotFound(path) => write!(f, "找不到待替换文本：{}", path.display()),
            Self::AmbiguousContext(path) => write!(f, "待替换文本有多处匹配：{}", path.display()),
            Self::PathOutsideRoot(path) => write!(f, "路径超出工作目录边界：{}", path.display()),
            Self::Io { path, source } => write!(f, "{}：{source}", path.display()),
        }
    }
}

impl std::error::Error for PatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// 将操作行和 JSON 字符串正文解析成有序补丁，语法错误报告从 1 开始的行号。
pub fn parse_patch(_input: &str) -> Result<Patch, PatchError> {
    todo!("识别 ADD、UPDATE、DELETE，解码正文与有序的 SEARCH/REPLACE 对，并定位语法错误")
}

/// 先在内存中验证所有相对路径和编辑并生成新内容，再提交整批变更。
pub fn apply_patch_atomic(_root: &Path, _patch: &Patch) -> Result<PatchSummary, PatchError> {
    todo!("检查路径边界与唯一匹配，所有文件预检成功后再提交，返回变更计数")
}

pub struct ApplyPatchTool {
    root: PathBuf,
}

impl ApplyPatchTool {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl Tool for ApplyPatchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "apply_patch".into(),
            description: "在工作目录中应用一批 Search/Replace 文件编辑".into(),
            parameters: json!({
                "type": "object",
                "properties": { "patch": { "type": "string" } },
                "required": ["patch"]
            }),
        }
    }

    fn call(&self, _args: Value) -> Result<String, String> {
        todo!("读取字符串参数 patch，调用解析与原子提交，把摘要或失败原因返回给模型")
    }
}
