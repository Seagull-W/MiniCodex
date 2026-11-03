# AGENTS.md — MiniCodex 长期约束

> 本文件由 Codex 在每次会话自动加载（从项目根到 cwd 逐级收集 `AGENTS.md` 并拼接，项目根以 `.git` 为标记）。
> 它只做一件事：防止长期协作把「学着重写」变成「让模型生成一堆看不懂的代码」。
> 具体怎么测试、怎么验收，见 `docs/COLLAB.md`；本文件不重复。

## 0. 项目定义

MiniCodex 是用 Rust **逐模块手写**复刻 `openai/codex` 核心语义的学习仓库。

- 目标：亲手写出并真正读懂一个 coding agent 的最小内核——协议类型、配置分层、指令注入、
  命令执行与策略、补丁应用、会话持久化、单轮主循环、非交互前端。
- 非目标（明确不做）：与上游功能对齐、性能对齐、TUI、云任务、插件/市场、语音、实时对话、
  Windows 沙箱、遥测、MCP、认证与计费。
- 完成判据（三者缺一不算完成）：
  1. 本仓库测试通过；
  2. 语义与上游一致，且能指出上游的 `path:line`；
  3. **人能不看代码复述这个模块在做什么、为什么这么做**——这条最硬，它是本项目存在的理由。

## 1. 七条规矩

| ID | 规矩 | 为什么 |
|----|------|--------|
| R1 | 一个任务只推进一个里程碑里的一个模块；跨模块先拆任务 | 拆开才读得完；混在一起时人只看结论，不看过程 |
| R2 | 顺序固定：`notes/<模块>.md` → 测试 → 实现 | 先写测试才知道要实现什么；反过来就是照着上游抄 |
| R3 | `codex-src/` 只读，禁止作为依赖、禁止 vendor 进本仓库 | 依赖上游 = 没写；本项目要的是理解，不是能跑 |
| R4 | 新增第三方依赖要先写进 `docs/COLLAB.md` 裁决表 | 一个依赖就是一片要读的代码，学习预算有限 |
| R5 | 上游的 README / 注释 / 测试名都不是规格；规格是源码行为 + 可运行测试 | 文档会过期，注释会骗人，编译器和断言不会 |
| R6 | 说「上游行为是 X」必须给出 `codex-src/…:行号`；给不出就标 UNVERIFIED | 这是我们检查"是否真的读懂了"的唯一办法 |
| R7 | 不为了让测试变绿而改测试或放宽断言 | 改测试 = 把尺子锯短，学习项目里这是唯一真正的失败 |

## 2. 固定骨架

```
MiniCodex/
├─ AGENTS.md                 本文件（长期约束）
├─ docs/COLLAB.md            协作协议 + 怎么测试 + 台账（追加式）
├─ codex-src/                只读上游参考（git clone，不提交）
├─ Cargo.toml                workspace
├─ x.sh                      一条命令跑完 fmt + clippy + test
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
└─ notes/                    每个模块一份笔记，结论带出处
```

不新增顶层目录或 crate——骨架稳定了，注意力才在模块内部。

## 3. 里程碑

顺序固定，可跳过某个模块，但不可跳级。路径相对仓库根的 `codex-src/codex-rs/`。

| 里程碑 | 模块 | 上游锚点（只读这些） | 目标行数 | 完成标准 |
|--------|------|----------------------|----------|----------|
| M0 | 骨架 | — | 0 | workspace 能编译；`x.sh` 跑通；无上游依赖 |
| M1 | mc-protocol | `protocol/src/protocol.rs`, `protocol/src/items.rs` | 600–900 | serde 往返不丢字段；枚举命名与上游一致（子集） |
| M2 | mc-config | `config/src/loader/mod.rs`, `config/src/types.rs`, `config/src/config_toml.rs` | 800–1200 | 默认→用户→项目→profile→`-c` 五层深合并，优先序有测试锁死 |
| M3 | mc-agents-md | `core/src/agents_md.rs` | 250–400 | 项目根到 cwd 拼接；`.override` 生效；大小上限有测试 |
| M4 | mc-exec + mc-policy | `shell-command/src/`, `execpolicy/src/parser.rs`, `execpolicy/src/policy.rs` | 900–1400 | 命令构造与上游同参；exit code/超时/截断有测试；策略三态可判 |
| M5 | mc-patch | `apply-patch/src/lib.rs`, `apply-patch/src/parser.rs` | 500–800 | Add/Delete/Update 三种；上下文不匹配时整体不写入 |
| M6 | mc-rollout | `rollout/src/recorder.rs`（只取 recorder 子集） | 500–800 | 追加写 JSONL；按会话恢复；坏行跳过并告警 |
| M7 | mc-agent | `core/src/session/turn.rs`, `core/src/client.rs`, `core/src/tools/` | 1000–1500 | 单轮：请求→流式事件→工具派发→观测→终止；错误路径有测试 |
| M8 | mc-cli | `exec/src/lib.rs`, `exec/src/cli.rs` | 300–500 | `--json` 每行合法 JSON；`-o` 写最终消息；退出码区分成败 |

行数是**上限不是指标**：写超了通常说明切分错了，先回去拆，不要硬写。

开工前先回答：**这个模块最容易被误解的 3 个点是什么？** 写进 `notes/<模块>.md` 第一节。

## 4. 一次交付长什么样

1. `notes/<模块>.md`：上游语义摘要（每条带 `path:line`）+ 我的取舍 + 「原来以为 X，实际是 Y」。
2. `src/`：实现。模块级 `//!` 说明它在整体里担任什么角色（写给人看，不是写给编译器看）。
3. `tests/`：行为测试，测试名描述行为，不描述实现。
4. 回话：改了什么、跑了什么命令、真实输出尾部 10 行、哪些还没验证。

## 5. 测试怎么写（摘要，完整版见 `docs/COLLAB.md` §5）

- 先写测试再写实现。测试是**搞清楚上游语义的工具**，不是交差用的。
- 每个模块至少三类：正常路径、错误路径、边界（空/重复/缺失）。
- 纯函数用表驱动（一组输入→输出对）；协议与持久化用往返测试（encode→decode→相等）；
  前端用集成测试（`assert_cmd` 跑 CLI，断言 stdout 与退出码）。
- 断言具体值（`assert_eq!`），不要 `assert!(x.is_ok())` 就算完。
- 拿不准上游行为时**做对照实验**：跑一次真 `codex`，把输出存成 fixture，让我们的实现对齐它。
- 不值得写测试的地方：一行 getter、纯数据构造——别把时间花在不可能错的地方。

## 6. 依赖基线

`serde`, `serde_json`, `toml`, `clap`, `thiserror`, `anyhow`, `tracing`, `regex`, `shlex`,
`tempfile`, `assert_cmd`, `insta`，以及 M7 起按需的 `tokio`。其余先问。

## 7. 什么时候停下交人

1. 需要新增依赖或新增 crate；
2. 上游同一行为有多处互相矛盾的实现，判断不出哪个是当前语义；
3. 需要联网、需要凭据、需要改 `codex-src`；
4. 任务实际规模超过一个模块，或预计改动超过 800 行。

停下的动作：把情况和你的判断写进 `docs/COLLAB.md` 台账，然后等人。

## 8. 提交

- 分支 `m<N>/<module>`；每个提交自身可编译可测。
- 信息：`m<N>: <module> <做了什么>`，例如 `m2: config five-layer merge with -c overrides`。
- 正文写：读过的上游 `path:line`、取舍、还没覆盖的边界。

## 9. 交接前自查

- [ ] `git status --short` 显示只动了本任务的路径
- [ ] `./x.sh` 全绿（贴真实末尾输出，不转述）
- [ ] 新行为都有测试；测试名说的是行为
- [ ] 每个「上游如此」都有 `path:line`
- [ ] 没有新增依赖、没有 `unsafe`、库代码没有 `unwrap`
- [ ] 未验证项列清楚了，没有把"没做"写成"已完成"
- [ ] 台账追加了本次条目
