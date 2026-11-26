# MiniCodex

用 Rust 从零手写一个最小 Coding Agent，目标有两个：

1. **真正理解 Coding Agent（以 Codex 为蓝本）的核心机制**；
2. **在真实工程问题中循序渐进掌握地道 Rust**。

核心原则：**机制对齐、协议极简、每阶段可运行**。我们学习 Codex 真正起作用的设计思想（ReAct 循环、工具注册表与错误自纠、安全策略引擎、原子 Patch、上下文注入、SSE 流式解析、事件驱动与事件溯源持久化），不把精力浪费在 OpenAI 历史线格式兼容细节上。

---

## 七阶段路线图

| 阶段 | 模块 | 核心 Agent / Codex 机制 | 对应的 Rust 核心能力 | 状态 |
|---|---|---|---|---|
| **S1** | [最小 Agent 主循环](docs/stages/S1-agent-loop.md) | ReAct 循环、`Vec<Message>` 历史、工具错误自纠、最大步数保护 | `enum` + `match`、所有权与借用、`trait` 抽象、`Result`/`?`、自定义错误 | 进行中 |
| **S2** | [工具系统 & Shell 执行](docs/stages/S2-tools-and-shell.md) | 工具注册表、JSON Schema、子进程超时、输出截断 | `Box<dyn Tool>` 动态派发、`serde_json::Value`、`Command`、UTF-8 截断 | 骨架已备 |
| **S3** | [安全策略与审批](docs/stages/S3-exec-policy.md) | 命令分词与前缀匹配、`Allow`/`Ask`/`Deny` 三态决策、审批回调 | 轻量解析、切片与迭代器、借用与生命周期、闭包回调 | 骨架已备 |
| **S4** | [文件编辑 `apply_patch`](docs/stages/S4-apply-patch.md) | Search/Replace 补丁、上下文定位、全有或全无原子写盘 | 状态机解析、结构化错误定位、内存事务、`tempfile` 隔离测试 | 骨架已备 |
| **S5** | [上下文与配置注入](docs/stages/S5-context-and-config.md) | 分层配置深合并、从 Git 根到 `cwd` 发现 `AGENTS.md`（含 `.override`） | `Path`/`PathBuf` 遍历、`toml` 递归合并、`Option` 组合子 | 骨架已备 |
| **S6** | [接通真实 LLM（SSE）](docs/stages/S6-llm-streaming.md) | OpenAI-compatible 流式 API、SSE 增量解析、工具参数分片拼接、重试 | `tokio` + `async/await`、流式解析、状态累加器、退避重试 | 骨架已备 |
| **S7** | [事件驱动、中断与恢复](docs/stages/S7-session-and-rollout.md) | `Op`/`Event` 通道解耦（单写者）、协作式取消、JSONL 消息重放与会话恢复 | `mpsc` 通道、`tokio::select!`、`CancellationToken`、Serde JSONL | 骨架已备 |

> 想看上游 158 万行 Codex CLI 的全貌作为延伸对照，可随时翻阅 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

---

## 分阶段练习

从表中的阶段指南进入，按行为测试补全对应模块的中文 `todo!()`。S1–S5 都使用同步接口。S6 引入独立的异步客户端，S7 的会话再把它与工具注册表连接起来。仓库的默认演示仍使用 S1 的剧本客户端。

```bash
# 骨架交接时检查全部测试能否编译
cargo test --no-run

# 学完哪一阶段就运行哪一行
./x.sh s1
./x.sh s2
./x.sh s3
./x.sh s4
./x.sh s5
./x.sh s6
./x.sh s7

# 全阶段验收
./x.sh

# 完成 S1 后观察默认演示中的工具报错和自纠
cargo run
```

每个阶段命令先检查全仓格式和 Clippy，再运行库测试与选中阶段的集成测试。`./x.sh` 执行同样的前置检查和全量 `cargo test`。尚未补全的练习会在行为测试中触发对应的 `todo!()`。阶段通过并完成代码审阅后，按[双笔记收口清单](docs/NOTES_BLUEPRINT.md)整理真实运行记录。
