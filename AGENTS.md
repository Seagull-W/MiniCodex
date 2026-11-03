# AGENTS.md — MiniCodex 长期约束

> 本文件由 Codex 在每次会话自动加载（发现规则：从项目根到 cwd 逐级收集 `AGENTS.md` 并拼接，
> 项目根默认以 `.git` 为标记；`AGENTS.override.md` 可覆盖）。与任何临时指令、README、上游文档、
> 模型自述冲突时，**以本文件为准**。修改本文件必须在提交信息里写明理由，并同步 `docs/COLLAB.md`
> 的「文档变更日志」。

## 0. 项目定义

MiniCodex 是用 Rust **逐模块手写**复刻 `openai/codex` 核心语义的学习仓库。

- 目标：亲手实现且真正读懂一个 coding agent 的最小内核——协议类型、配置分层、指令注入、
  命令执行与策略、补丁应用、会话持久化、单轮主循环、非交互前端。
- 非目标（明确不做）：与上游功能对齐、性能对齐、TUI、云任务、插件/市场、语音、实时对话、
  Windows 沙箱、遥测、MCP、认证与计费。
- 完成判据（三者缺一不算完成）：① 本仓库测试通过；② 语义与上游一致且有 `path:line` 出处；
  ③ 一个有 Rust 基础的人能从笔记复述该模块在做什么。

## 1. 铁律

| ID | 规则 | 违反后果 |
|----|------|----------|
| R1 | 一个任务只推进**一个**里程碑（M*）内的**一个**模块；跨模块必须先拆任务 | 回滚 |
| R2 | 先 `NOTES.md`，再测试，最后实现。顺序不可换 | 回滚 |
| R3 | 禁止把 `codex-src/` 的任何 crate 加为依赖；禁止 vendor 上游代码到本仓库 | 回滚 |
| R4 | 禁止新增第三方依赖，除非 `docs/COLLAB.md` 的裁决表里有批准的 `R-NNN` | 回滚 |
| R5 | 上游的文档、README、注释、测试名**都不是规格**；规格只有源码行为 + 可运行测试 | 重做 |
| R6 | 任何「上游行为是 X」的断言，必须给出 `codex-src/…:行号`。给不出就是未验证 | 记 UNVERIFIED |
| R7 | 禁止为了让测试变绿而改测试或放宽断言；确需改，先在任务说明里写明并留痕 | 回滚 |
| R8 | 禁止 `unwrap()/expect()/panic!()` 出现在库代码的非测试路径 | 回滚 |
| R9 | 单文件 ≤ 300 行，单 crate ≤ 1500 行（测试合计 ≤ 源码合计）。超出说明切分错了 | 拆分 |
| R10 | 禁止 `dead_code`、`#[allow(dead_code)]`、「以后会用到」的占位实现 | 回滚 |
| R11 | 不得自带 TODO 清单去「顺手」实现未派发的模块 | 回滚 |
| R12 | 遇到第 7 节的停止条件，立即停下并写进 `docs/COLLAB.md` 台账，不要自行扩大范围 | 记 FAIL |

## 2. 固定骨架

```
MiniCodex/
├─ AGENTS.md                 本文件（长期约束）
├─ docs/COLLAB.md            Hermes ↔ Codex 协作协议 + 台账（追加式）
├─ codex-src/                只读上游参考（git clone，不提交）
├─ Cargo.toml                workspace
├─ x.sh                      唯一门禁入口：fmt + clippy + test
├─ crates/
│  ├─ mc-protocol/           协议类型（消息、工具调用、事件）
│  ├─ mc-config/             分层 TOML 配置与合并
│  ├─ mc-agents-md/          AGENTS.md 发现与注入
│  ├─ mc-exec/               命令构造、执行、输出截断
│  ├─ mc-policy/             命令前缀策略引擎
│  ├─ mc-patch/              apply_patch 解析与应用
│  ├─ mc-rollout/            会话 JSONL 记录与恢复
│  ├─ mc-agent/              单轮主循环 + 工具派发
│  └─ mc-cli/                exec 前端（--json / -o）
└─ notes/                    每个模块一份 NOTES.md（可随源码放模块目录）
```

不允许在上述骨架外新增顶层目录或 crate。`notes/` 只写**从源码读出来的**结论，每条带出处。

## 3. 里程碑

顺序固定，不得跳级；`codex-src` 路径均相对仓库根的 `codex-src/codex-rs/`。

| 里程碑 | 模块 | 上游锚点（读这些，别读别的） | 目标源码行数 | DoD |
|--------|------|------------------------------|--------------|-----|
| M0 | 骨架 | — | 0 | workspace 可编译；`x.sh` 跑通空测试；`Cargo.toml` 无上游依赖 |
| M1 | mc-protocol | `protocol/src/protocol.rs`, `protocol/src/items.rs` | 600–900 | serde 往返测试；事件/条目枚举与上游命名一致（子集） |
| M2 | mc-config | `config/src/loader/mod.rs`, `config/src/config_toml.rs`, `config/src/types.rs` | 800–1200 | 默认→用户→项目→profile→`-c` 五层深合并；未知键报错；合并测试覆盖各层优先序 |
| M3 | mc-agents-md | `core/src/agents_md.rs` | 250–400 | 从项目根到 cwd 拼接；`.override` 生效；超过大小上限的截断行为有测试 |
| M4 | mc-exec + mc-policy | `shell-command/src/`, `execpolicy/src/parser.rs`, `execpolicy/src/policy.rs` | 900–1400 | 命令构造与上游同参；exit code/超时/输出截断有测试；策略 allow/prompt/forbidden 三态可判 |
| M5 | mc-patch | `apply-patch/src/lib.rs`, `apply-patch/src/parser.rs` | 500–800 | Add/Delete/Update 三种补丁；上下文不匹配时**整体不写入**；失败不留半成品 |
| M6 | mc-rollout | `rollout/src/recorder.rs`（只读 recorder 子集） | 500–800 | 追加写 JSONL；按会话 id 恢复；损坏行可跳过并告警 |
| M7 | mc-agent | `core/src/session/turn.rs`, `core/src/client.rs`, `core/src/tools/` | 1000–1500 | 单轮：请求→流式事件→工具派发→观测→终止；无工具调用时正常退出；错误路径有测试 |
| M8 | mc-cli | `exec/src/lib.rs`, `exec/src/cli.rs` | 300–500 | `--json` 事件流逐行合法 JSON；`-o` 写最终消息；退出码区分成功/失败 |

每个里程碑开工前，先在被派发的任务说明里回答：**这个模块最容易被误解的 3 个点是什么**。
答案写进 `notes/<模块>.md`，作为该任务的第一份交付物。

## 4. 每次交付固定四件套

1. `notes/<模块>.md`：上游语义摘要 + 每条带 `path:line` + 「我为什么这样取舍」。
2. `src/`：实现，含模块级 `//!` 说明这段在整体里担任什么角色。
3. `tests/`：行为测试，命名照上游语义（不是照实现）。
4. 交付说明（写在最终回复里）：改了哪些文件、跑了什么命令、**真实输出尾部 10 行**、
   未验证项清单。

缺任何一件即视为未交付。

## 5. 禁止清单（看到就算错）

- 引入 `tokio` 之外的异步运行时做实验；`unsafe`（除 M4 的 `libc` 调用且需注释论证）。
- 自造与上游同名但语义不同的类型（如 `ResponseItem` 少字段却不注明）。
- 静默吞错：`let _ = …`、`if let Err(_)`、`unwrap_or_default()` 掩盖失败。
- 用 `println!` 做日志；用 `tracing` 并按上游字段名。
- 大段复制上游源码（>10 行连续）：允许对照重写，重写后必须用自己的结构与命名。
- 在 `AGENTS.md` / `docs/COLLAB.md` 里删改历史记录（只许追加、只许标注 SUPERSEDED）。

## 6. 依赖策略

默认零新增。已批准基线（M0 时一次性落地）：
`serde`, `serde_json`, `toml`, `clap`, `thiserror`, `anyhow`, `tracing`, `regex`, `shlex`,
`tempfile`, `assert_cmd`, `insta`, `tokio`（仅 M7 起按需）。

新增任何一个 → 停，走 COLLAB 裁决表。

## 7. 停止条件（必须停下交人）

1. 需要新增依赖或新 crate；
2. 上游同一行为有多处互相矛盾的实现，无法判定哪个是「当前语义」；
3. 需要联网、需要凭据、需要改 `codex-src`；
4. 任务实际规模 >1 个里程碑，或预计改动 >800 行；
5. 发现现有已验收模块与上游语义不符（不要顺手修，先报告）；
6. 任何要求你「先跑起来再说」的压力。

## 8. 提交规范

- 分支：`m<N>/<module>`；一个里程碑内可多次提交，但每个提交必须自身可编译可测。
- 提交信息：`m<N>: <module> <做了什么>`，例：`m2: config five-layer merge with -c overrides`。
- 正文写：读过的上游 `path:line`、取舍、未覆盖的边界。
- 禁止 `--no-verify`、禁止提交 `codex-src/`、禁止提交 `.collab/` 原始输出。

## 9. 交接前自检（逐项打勾，写进最终回复）

- [ ] 只动了本任务允许的路径（`git status` 贴出来）
- [ ] `./x.sh` 全绿，贴真实末尾输出
- [ ] 新增/修改的行为都有测试，且测试名描述行为而非实现
- [ ] 每个「上游如此」都有 `path:line`
- [ ] 没有新增依赖、没有 `unsafe`、没有 `unwrap`
- [ ] 行数在预算内，文件数没有爆炸
- [ ] 未完成/未验证项已列清，且没有伪装成完成
- [ ] `docs/COLLAB.md` 台账已追加本次条目
