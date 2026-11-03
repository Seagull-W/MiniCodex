# Hermes ↔ Codex 协作文档 — MiniCodex

> 性质：**追加式**。协议部分（§1–§9）改动必须在 §12 留痕；§7 台账、§8 裁决、§11 未决事项只追加，不改写。
> 约束下游：本文件管「怎么协作」，`AGENTS.md` 管「Codex 不许做什么」。冲突时以 `AGENTS.md` 为准。

---

## 0. 为什么需要这份文档

任务是把 `codex`（Rust 版）的核心模块转成个人逐步手写的模块。执行由 Hermes 调度、Codex 动手。
核心风险不是「写不出来」，而是三件事：

1. **Codex 的文字不可信**：它的汇报、README 摘要、注释、「上游是这样的」都是**待验证断言**，
   不是规格。它会把「像」当成「是」，把一次通过当成语义正确。
2. **范围漂移**：学习型重写最容易被「顺手补全」拖成产品级复刻，最后没人读得懂。
3. **验收缺位**：没有任何人能证明「模块完成」的时候，进度就是自我感觉。

本文件把这三件事变成流程：证据分级（§2）、派单（§3）、门禁（§5）、反幻觉审查（§6）、台账（§7）。

---

## 1. 角色

| 角色 | 职责 | 不做 |
|------|------|------|
| **人（学习者）** | 定方向、读 NOTES、决定是否继续投入、最终验收 | 不写样板代码 |
| **Hermes** | 写 brief、选思考强度、派单、逐门审查、裁决、记账单、拦漂移 | 不替 Codex 编造证据；不把「看起来对」当验收 |
| **Codex** | 在 brief 范围内实现 + 自测 + 交付四件套 | 不扩范围、不新增依赖、不修改上游 |

Hermes 的定位是**审查者而非传声筒**：Codex 的任何结论进入仓库前，Hermes 必须独立复核一次。

---

## 2. 证据分级（所有结论必须落级）

| 级别 | 含义 | 可否用于验收 |
|------|------|--------------|
| **E0** | 本仓库可复现命令的真实输出（build/test/clippy/程序 stdout、退出码） | ✅ 唯一够格 |
| **E1** | `codex-src/` 源码 `path:line` 原文 | ✅ 用于语义比对 |
| **E2** | 上游 README/注释/文档 | ⚠️ 仅在 E1 一致时引用 |
| **E3** | Codex 或任何模型的自述、总结、「我记得」 | ❌ 只作线索 |

规则：
- 断言必须自带级别。写不出 `path:line` 的「上游行为」→ 标 `UNVERIFIED`，进 §11。
- **E3 永远不能升级为 E1/E0**，只能被 E0/E1 替换。替换时在台账记 `E3→E1` 并给出出处。
- 冲突时优先级：E0 > E1 > E2 > E3。E0 与 E1 冲突 = 我们的实现或上游理解有错，必须查清，
  不许二选一糊过去。

---

## 3. 派单协议

### 3.1 前置

- 上游参考固定在 `/home/seagull/MiniCodex/codex-src`（git clone，不入库），
  当前锁定提交 `f9e7b19df8aa75490fc1d90f98d3d95ad716d08e`（2026-11-03）。
  **不在该提交上的行为一律不作依据**；升级需要在 §8 立一条裁决。
- 已核实环境：`codex-cli 0.166.0`；`model = "gpt-6-astra"`；`model_reasoning_effort = "low"`（默认）。
- **`rustc`/`cargo` 未安装** → M0 的阻塞项，见 §11。

### 3.2 派单命令（WSL，root）

```bash
cd /home/seagull/MiniCodex
mkdir -p .collab/raw .collab/out
codex exec \
  --sandbox danger-full-access \
  -C /home/seagull/MiniCodex \
  -m gpt-6-astra -c model_reasoning_effort=medium \
  --json \
  -o .collab/out/<TASK-ID>.md \
  "$(cat .collab/briefs/<TASK-ID>.md)" \
  > .collab/raw/<TASK-ID>.jsonl 2>&1
```

要点（均已按本机 `codex exec --help` 核对）：
- `--sandbox danger-full-access`：WSL + root 下 `workspace-write` 的 bubblewrap 常失败；
  安全边界改由「干净 git 状态 + narrow brief + 事后 diff 审查」承担。
- `-c model_reasoning_effort=…` **必须显式给**，否则吃 `config.toml` 的 `low`。
- `--json` 落原始事件流，事后可审计；`-o` 只取最终消息（即 Codex 的自述，级别 E3）。
- 长 brief 写文件再 `"$(cat …)"`，不要内联；不要用 `exec` 不支持的 flag（会 exit 2 且零产出）。
- 一次只有一个 codex 进程改同一 workdir。需要并行 → 用 `git worktree`，一树一任务。

### 3.3 brief 模板（`.collab/briefs/<TASK-ID>.md`）

```markdown
# <TASK-ID> <里程碑> <模块>

## 范围（只许改这些路径）
- crates/mc-config/src/**
- crates/mc-config/tests/**
- notes/mc-config.md

## 上游锚点（只读，不许修改）
- codex-src/codex-rs/config/src/loader/mod.rs
- codex-src/codex-rs/config/src/types.rs

## 要做什么
<3–7 条，行为级描述，不写实现方案>

## 不要做什么
- 不要新增依赖；不要动 M0 骨架；不要实现 M3 及以后
- 不要修改 codex-src

## 交付
1) notes/mc-config.md（每条结论带 codex-src path:line）
2) 实现 + 测试
3) 最终回复附：`git status --short`、`./x.sh` 真实末尾输出、未验证项

## 停止条件
见 AGENTS.md §7；触发即停，写进 docs/COLLAB.md 台账。
```

一份 brief 只覆盖一个里程碑内的一个模块，且能在一屏内读完。写不出范围清单 → 说明还没想清楚，不许派单。

---

## 4. 思考强度预算

`gpt-6-astra` 上限 `high`。**默认 medium，按任务性质升降，不许盲选 high**。

| 强度 | 用在什么任务 | 典型例子 |
|------|--------------|----------|
| `low` | 机械、可穷举、一眼可验 | 建 workspace 骨架、改命名、补测试样板、格式修复、写 x.sh |
| `medium` | **常态**：单模块实现，需读 2–6 个上游文件 | M1/M2/M3/M5 的常规实现 |
| `high` | 语义纠缠或跨模块协议：判错一次后面全错 | M4 策略三态与沙箱交互、M7 主循环事件时序、连续两次验收失败后的第三次 |
| — | 禁用 | 没有任何理由的 high；用 high 跑机械任务 |

硬规则：
- `high` 必须由 Hermes 在 brief 里写明理由（一句话），并在台账记 `effort=high + 理由`。
- 同一任务连续两次验收失败 → 第三次允许升级到 `high`，同时要求 Codex 先交「失败归因」而非补丁。
- 单日 high 次数上限 2；超限需人同意（额度）。
- effort 变更不改变验收标准：`high` 不等于可信，只等于更少借口。

---

## 5. 验收门（G1–G6，全绿才可提交）

| 门 | 检查 | 命令 / 动作 | 不通过的处理 |
|----|------|-------------|--------------|
| **G1** 范围 | diff 只碰 brief 允许路径 | `git status --short` + `git diff --stat`，逐条对照 brief | 直接回滚越界文件 |
| **G2** 编译 | 全 workspace 编译 | `cargo build --workspace --all-targets` | 打回，不讨论 |
| **G3** 测试 | 目标 crate + 全量 | `cargo test -p <crate>` 再 `cargo test --workspace` | 打回；先看是不是改了测试（见 G5） |
| **G4** 静态 | fmt + clippy | `cargo fmt --check`；`cargo clippy --workspace --all-targets -- -D warnings` | 打回 |
| **G5** 反作弊 | 测试是否被削弱 | `git diff -- '*tests*'`，比对断言强度/删除的用例 | 判定 FAIL，回滚测试改动 |
| **G6** 语义 | 抽 3 条关键行为，逐条在 `codex-src` 找到对应实现 | `rg -n` 定位，读到原文，确认 `path:line` | 标 UNVERIFIED，进 §11 |

补充：
- `./x.sh` 是唯一入口（M0 建立），内容等价 G2+G3+G4。门禁输出必须贴**真实末尾 10 行**，
  不接受「全部通过」这种转述。
- 首次跑某 crate 的测试要清 `target` 之外的污染：`git stash -u` 前后各跑一次不可省略——
  确保测的是提交后的状态，不是工作区巧合。
- G6 抽样不是抽查运气：每个里程碑固定取「最容易被简化的 3 条」——错误路径、边界输入、
  顺序依赖。

---

## 6. 反幻觉审查（针对 Codex 的文字输出）

Codex 的最终消息（`-o` 文件）只是**待审材料**。Hermes 按这张表逐条过：

| Codex 的说法 | Hermes 的动作 | 判据 |
|--------------|----------------|------|
| 「上游行为是 X」 | 要 `path:line`；自己 `rg` 复核原文 | 给不出 = E3，标 UNVERIFIED |
| 「测试已通过」 | 自己重跑，看退出码与用例数 | 无输出 = 未执行 |
| 「测试覆盖了边界」 | 打开测试文件读断言，而不是读名字 | 只断言 happy path = 未覆盖 |
| 「这是最小改动」 | `git diff --stat` 对比 brief 范围 | 超范围 = 范围漂移 |
| 「已处理错误」 | 搜 `unwrap|expect|let _ =|unwrap_or_default` | 命中即驳回 |
| 「上游也是这么写的」 | 允许结构相近，不允许 >10 行连续复制 | 需给出自己的组织方式 |
| 具体数字（LOC/覆盖率/耗时） | 一律重算，不引用 | 数字不可引用 |
| 「剩余工作只剩 X」 | 自己按 §11 核一遍清单 | 漏项 = 交付不完整 |

通用三条：
1. **先要证据，再要结论**。顺序反了就是在被说服。
2. **任何「完成」必须能被一条命令复现**；不能复现的完成等于没做。
3. Codex 主动提出的「顺手优化/额外发现」→ 不进本次 diff，转 §11 待办。

---

## 7. 任务台账（追加，不删改）

状态：`PASS` / `FAIL` / `BLOCKED` / `SUPERSEDED`。任务号 `MC-NNNN` 单调递增。

| 任务 | 日期 | 里程碑 | 摘要 | effort | 状态 | 证据 |
|------|------|--------|------|--------|------|------|
| MC-0000 | 2026-11-03 | M0 | 建立协作文档 + `AGENTS.md`；锁定上游 f9e7b19 | — | PASS | 本文件；`git log` |

### 条目模板（新任务追加到上表下方「明细」）

```markdown
### MC-NNNN · <YYYY-MM-DD> · <里程碑> <模块>
- 目标：
- effort：low|medium|high（high 需理由）
- 命令：<实际执行的 codex exec 行>
- 结果：PASS|FAIL|BLOCKED
- 证据：E0 <命令 + 末尾输出要点> / E1 <path:line>
- 裁决：R-NNN（若有）
- 遗留：<转 §11 的条目号>
```

明细从 MC-0001 起写在 §10 之后（保持台账表格短，明细追加在末尾，避免表格被撑爆）。

---

## 8. 裁决表（R-NNN，只追加；作废用 SUPERSEDED，不删）

| 编号 | 裁决 | 理由 | 状态 |
|------|------|------|------|
| R-001 | 上游锁定 `f9e7b19`，升级需新裁决 | 学习型重写怕上游噪声；比对必须有唯一点 | ACTIVE |
| R-002 | 一律 `--sandbox danger-full-access` 派单 | WSL+root 下 workspace-write 常失败；边界改由流程承担 | ACTIVE |
| R-003 | 禁止引入上游 crate / vendor 上游代码 | 否则「学习重写」退化为「依赖搬运」 | ACTIVE |
| R-004 | 依赖基线仅 §6 of AGENTS.md 所列 | 控制理解成本，避免生态蔓延 | ACTIVE |
| R-005 | 策略引擎手写解析器，不用 starlark crate | 上游用 starlark-rust；我们只需前缀规则子集，手写更可读 | ACTIVE |

---

## 9. 漂移体检（每 5 个任务或每次里程碑收尾做一次）

逐项回答，任何一项「否」→ 停下修流程，不要继续堆功能：

1. 最近 5 个任务的 diff 是否都落在 brief 声明的路径内？
2. 是否有模块的 `NOTES.md` 里出现无出处结论（E3 冒充 E1）？
3. 当前总 LOC 是否仍在里程碑预算内（详见 `AGENTS.md` §3 表）？
4. 是否出现「为通过测试而改测试」的痕迹（G5 历史）？
5. 人是否能不看代码复述上一个模块的语义？不能 → 速度超过理解，降速。

体检结论写入 §10 末尾的「体检记录」，一条一行。

---

## 10. 体检记录 & 任务明细（追加区）

### 体检记录

| 日期 | 范围 | 结论 | 动作 |
|------|------|------|------|
| 2026-11-03 | 立项 | 目录为空，无代码可体检 | 建立约束文档，先装工具链 |

### 任务明细

（从 MC-0001 开始追加）

---

## 11. 未决事项 / 下一步（可改写，保持短）

| 编号 | 事项 | 级别 | 备注 |
|------|------|------|------|
| P0-1 | 安装 Rust 工具链（`rustup` + stable，含 clippy/rustfmt） | 阻塞 M0 | 本机 `cargo`/`rustc` 均不存在 |
| P0-2 | `git init` + 建立 `.gitignore` | 阻塞派单 | Codex 需要 git 仓库；`codex-src/` 必须忽略 |
| P1-1 | 建立 `x.sh` 门禁脚本（fmt+clippy+test） | M0 | G2–G4 的唯一入口 |
| P1-2 | 写 `docs/SPEC-M1.md`（mc-protocol 子集边界） | M1 前置 | 先定「做什么子集」，否则必漂移 |
| P2-1 | 决策：`mc-agent` 是否需要 tokio，或同步 + 阻塞即可 | M7 前 | 影响依赖基线 R-004 |

---

## 12. 文档变更日志

| 日期 | 变更 | 原因 |
|------|------|------|
| 2026-11-03 | 建立本文件；确立 E0–E3 证据分级、G1–G6 门禁、effort 预算、裁决 R-001–R-005 | 立项；把「Codex 文字不可作指导」落成可执行流程 |
