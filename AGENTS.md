# AGENTS.md — MiniCodex 学习协作约定

MiniCodex 是一个用于**学习 Rust** 和**理解 Coding Agent（Codex）核心机制**的项目，并同步向 `/home/seagull/seagull-llm` 沉淀「MiniCodex 机制笔记」与「Rust 笔记」两套长期知识库。

## 六条核心约定

1. **学机制，不抄历史包袱**
   - 聚焦 Codex 真正核心的架构机制（主循环、工具注册与错误自纠、安全策略、原子补丁、上下文注入、流式响应、事件驱动与事件溯源持久化）。
   - 不复刻上游的历史兼容细节（如 `task_started` 别名、多层冗余协议类型、特殊字段的奇特序列化规则）。仅在接真模型 API 的适配层处理外部协议。
   - 如需查阅上游参考实现，`/home/seagull/codex-src/` 只读，禁止作为依赖引入。

2. **每阶段可运行，按 S1–S7 垂直推进**
   - 单 crate 结构（`src/` 按模块拆分），每个阶段交付一个完整、可运行的增量能力。
   - S1–S5 保持纯同步 Rust，S6 再引入 `tokio` 异步与真实 LLM 流式接入。

3. **清晰的分工：骨架与测试由助手准备，核心逻辑由学习者手写**
   - 助手负责提供：各阶段设计文档（`docs/stages/S<N>-*.md`）、干净的类型/Trait 接口定义、核心逻辑留作 `todo!()` 的可编译骨架、以及行为验收测试（`tests/s<N>_*.rs`）。
   - **严禁代写核心业务逻辑，也严禁在学习者完成该阶段前提前代写 `/home/seagull/seagull-llm` 中的笔记正文**。
   - 学习者逐阶段手写实现并跑通对应阶段验证（如 `./x.sh s1`），再共同做 Code Review，之后才基于真实代码与思考沉淀该阶段的双笔记；任何时候**不得自动 `git push`**。

4. **代码与测试纪律**
   - 库代码不使用 `unsafe`，不在生产路径里用 `unwrap()` 吞掉错误。
   - 不为了测试变绿而放宽测试断言；每阶段结束保证 `./x.sh`（fmt + clippy + test）通过。

5. **按需引入依赖**
   - 保持依赖精简，每阶段只引入当前真正需要的 crate（S1 仅需 `serde`、`serde_json`）。

6. **双笔记同步沉淀（`/home/seagull/seagull-llm`）**
   - **纵向 `docs/mini-codex/`（新建栏目）**：按 S1–S7 一阶段一章，聚焦 Coding Agent / Codex 的核心机制、不变量、失败路径、架构流程图、真实运行轨迹与上游 `codex-src` 对照。首个章节完成后连同栏目首页一并建栏接入（不留空栏目）。
   - **横向 `docs/rust-notes/`（已有栏目）**：按 Rust 语言主题聚合为 5 篇（S1 / S2 / S3–S4 / S5 / S6–S7），用 MiniCodex 中的真实代码与编译期权衡讲清 Rust 特性，不与 `mini-codex` 重复叙事。
   - 必须严格遵守 `/home/seagull/seagull-llm` 的 `AGENTS.md`、`HTML_FORMATS.md`、`writing/README.md` 与 `writing/publishing.md`（`.html` 正文 + `.md` 元数据双文件、`data-design="linux-notebook"`、更新 `index.html`/`sidebar.ts`/`content-catalog.ts`/`sections.json`/`config.mts`），并跑通站点 4 项验证命令；在审阅通过前不推送远端。
