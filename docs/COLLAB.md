# Hermes ↔ Codex 协作文档 — MiniCodex

> 性质：**追加式**。协议与教学部分（§1–§6）改动要在 §12 留痕；§7 台账、§8 裁决、§10 明细只追加，不改写。
> 分工：本文件管「怎么协作、怎么学、怎么测」；`AGENTS.md` 管「Codex 长期不许做什么」。

---

## 0. 这份文档要解决什么

把 Rust 版 `codex` 的核心模块逐个变成**自己写出来、并且讲得清**的代码。
执行是 Hermes 调度 + Codex 动手，但项目的交付物不是"能跑的程序"，而是"你脑子里的模型"。

三个会真实发生的问题：

1. **把"能跑"当"懂了"**：Codex 会给你一段通过测试但你没读过的代码。这类代码是负债。
2. **范围漂移**：学习型重写很容易被"顺手补全"拖成产品级复刻，最后没人看得懂。
3. **文字不可信**：Codex 的汇报、上游 README、注释都是二手信息。它会复述 README 当行为，
   会把一次通过当语义正确。**这不是它坏，是语言模型的默认行为**，所以流程上只认可复现的东西。

对应到本文档的三块：派单（§3）、**测试与学习（§5，主体）**、验收（§6）。

---

## 1. 角色

| 角色 | 职责 | 不做 |
|------|------|------|
| **人（学习者）** | 定方向、读 NOTES 与测试、决定在哪停、最终验收 | 不写样板代码 |
| **Hermes** | 写 brief、选思考强度、派单、复核、裁决、记账 | 不替 Codex 编造证据；不把"看起来对"当验收 |
| **Codex** | 在 brief 范围内实现 + 写测试 + 交付四件套 | 不扩范围、不加依赖、不改上游 |

Hermes 是**复核者**，不是传声筒：Codex 的结论进仓库前，Hermes 自己跑一遍、自己读一遍源码。

---

## 2. 证据分级

不是为了抓骗子，是为了**防止自欺**——学习项目里最贵的错误是"以为懂了"。

| 级别 | 含义 | 用途 |
|------|------|------|
| **E0** | 本仓库可复现命令的真实输出（build/test/clippy/stdout/退出码） | 唯一能用于验收 |
| **E1** | `codex-src/` 源码 `path:line` 原文 | 语义比对的依据 |
| **E2** | 上游 README / 注释 / 文档 | 只在和 E1 一致时引用 |
| **E3** | Codex 或任何模型的自述、总结、"我记得" | 只当线索，当不了证据 |

规则很简单：
- 写不出 `path:line` 的"上游行为"→ 标 `UNVERIFIED`，进 §11，不许写进 NOTES 的结论段。
- E3 不能升级成 E1，只能被 E0/E1 替换；替换时在台账记 `E3→E1` 并给出出处。
- E0 和 E1 打架 = 有一方理解错了，必须查清。这条最值得花时间，别糊过去。

---

## 3. 派单协议

### 3.1 前置

- 上游参考固定在 `/home/seagull/MiniCodex/codex-src`（git clone，不入库），
  锁定提交 `f9e7b19df8aa75490fc1d90f98d3d95ad716d08e`（2026-11-03）。
  不在这个提交上的行为不作依据；升级要立一条新裁决。
- 已核实：`codex-cli 0.166.0`；`model = "gpt-6-astra"`；`config.toml` 里 `model_reasoning_effort = "low"`。
- 本机工具链（2026-11-03 装好）：`rustc/cargo 1.99.1`、`clippy 0.1.99`、`rustfmt 1.10.0`，
  装在 `/root/.cargo/bin`（已写入 `/root/.bashrc`、`/root/.profile`）。
  `crates.io` 直连可用；**`static.rust-lang.org` 不可达**，升级工具链要用清华镜像
  （`RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup`），且必须
  `env -u http_proxy -u https_proxy -u all_proxy …`（本机 7897 代理是坏的，rustup 会去用它）。

### 3.2 派单命令

```bash
cd /home/seagull/MiniCodex
mkdir -p .collab/briefs .collab/raw .collab/out
codex exec \
  --sandbox danger-full-access \
  -C /home/seagull/MiniCodex \
  -m gpt-6-astra -c model_reasoning_effort=medium \
  --json \
  -o .collab/out/<TASK-ID>.md \
  "$(cat .collab/briefs/<TASK-ID>.md)" \
  > .collab/raw/<TASK-ID>.jsonl 2>&1
```

要点（已按本机 `codex exec --help` 核对）：
- `--sandbox danger-full-access`：WSL + root 下 `workspace-write` 的 bubblewrap 常失败。
  真正的边界不是沙箱，而是"干净 git 状态 + 窄 brief + 事后自己读 diff"。
- `-c model_reasoning_effort=…` **必须显式给**，否则吃 `config.toml` 的 `low`。
- `--json` 落事件流（事后能看到它到底读了哪些文件、跑了什么命令）；`-o` 只是它的自述（E3）。
- 长 brief 写成文件再 `"$(cat …)"`；`exec` 不支持的 flag 会让它 exit 2 且零产出。
- 同一 workdir 只跑一个 codex；要并行就 `git worktree`，一树一任务。

### 3.3 brief 模板

```markdown
# <TASK-ID> <里程碑> <模块>

## 范围（只许改这些路径）
- crates/mc-config/src/**
- crates/mc-config/tests/**
- notes/mc-config.md

## 上游锚点（只读）
- codex-src/codex-rs/config/src/loader/mod.rs
- codex-src/codex-rs/config/src/merge_tests.rs

## 要做什么
<3–7 条行为级描述；不写实现方案，不写"参考某某函数">

## 测试要求
- 先写测试（列出这个模块必须覆盖的行为，见 §5.3 的清单）
- 每个"上游如此"的断言附 path:line

## 不要做什么
- 不加依赖；不动 M0 骨架；不做 M3 及以后；不改 codex-src

## 交付
1) notes/mc-config.md  2) 实现 + 测试
3) 回话附 `git status --short`、`./x.sh` 真实末尾输出、未验证项

## 停止条件
见 AGENTS.md §7。
```

一份 brief = 一个模块，一屏能读完。写不出路径白名单，说明范围还没定，别派单。

---

## 4. 思考强度预算

`gpt-6-astra` 上限 `high`。**默认 medium**，按任务性质升降。

| 强度 | 用在什么任务 | 例子 |
|------|--------------|------|
| `low` | 机械、可穷举、一眼能验 | 建 workspace、改命名、写 x.sh、补测试样板 |
| `medium` | **常态**：单模块实现，需读 2–6 个上游文件 | M1/M2/M3/M5 的常规实现 |
| `high` | 语义纠缠，判错一次后面全错 | M4 策略三态、M7 事件时序、连续两次失败后的第三次 |

- `high` 要在 brief 里写一句理由，台账记 `effort=high + 理由`；单日不超过 2 次（额度）。
- 连续两次验收失败 → 第三次升 `high`，并要求先交"失败归因"而不是直接给补丁。
- 强度不改变验收标准。`high` 只意味着更少的借口，不意味着可信。

---

## 5. 怎么测试（本文档主体）

这一节的目的不是"卡质量"，而是**用测试把上游语义逼出来**。测试是学习工具：
你写不出测试，说明你还没读懂；测试写得太松，说明你还没想清楚边界在哪。

### 5.1 测试的七个层级

按这个阶梯往上爬，一个模块通常用到 L1–L5，不必每级都做。

| 级别 | 名称 | 教什么 | 长什么样 |
|------|------|--------|----------|
| L0 | 类型即文档 | 让编译器告诉你缺什么 | 先把结构体/枚举照语义写出来，`todo!()` 撑住编译 |
| L1 | 单元测试 | 单个函数的契约 | `assert_eq!(merge(a, b).timeout, 30)` |
| L2 | 往返测试 | 序列化边界 | `decode(encode(x)) == x`，含未知字段与缺字段 |
| L3 | 表驱动 | 一组规则的所有分支 | `for (input, want) in CASES { … }` |
| L4 | 错误路径 | 失败也要有确定形状 | `assert_eq!(err.kind(), ErrorKind::MissingField)` |
| L5 | 对照测试 | 上游真实行为 | 把真 `codex` 的输出存成 fixture，逐字段比 |
| L6 | 集成测试 | 整个前端 | `assert_cmd` 跑 `mc-cli --json`，断言每行 JSON 与退出码 |

**L4 是最容易被跳过、也最值钱的一级。** 学习项目里"错误路径的语义"往往比正常路径更能说明设计意图。

### 5.2 一个模块的完整循环（以 M2 config 为例）

1. **读**：`config/src/loader/mod.rs` + `config/src/types.rs`，只找"层从哪来、谁盖谁"。
2. **写 NOTES 三行**：层的来源顺序、合并是深合并还是浅覆盖、未知键怎么处理。每条带 `path:line`。
3. **列测试清单**（先写名字，不写代码）：

   ```
   default_only_uses_builtin_values
   user_layer_overrides_default
   project_layer_overrides_user
   profile_layer_overrides_project
   cli_override_beats_every_layer
   deep_merge_preserves_sibling_keys
   unknown_key_is_rejected_with_path
   missing_required_field_reports_which_one
   empty_project_file_is_noop
   ```

4. **写第一个会失败的测试**，跑一次，确认它红（红的原因必须是"没实现"，不是"编译错"）。
5. **写最小实现**让它绿。绿了立刻提交一次（可编译可测）。
6. **补下一个测试**，直到清单清空；每个测试红→绿一轮，别一次写五个。
7. **回读**：删掉为了过测试而加的分支，重跑。此时如果测试挂了，说明那条分支其实是必要的——
   这就是测试在教你东西。

清单里那十条就是"我理解的 M2"。写完还是不懂，说明该回去读源码，不是继续写代码。

### 5.3 从上游测试里学规格（性价比最高的一招）

上游的 `*_tests.rs` 是作者亲手写下的规格，比 README 可信得多。具体做法：

```bash
cd /home/seagull/MiniCodex/codex-src/codex-rs
rg --files -g '*_tests.rs' config protocol apply-patch rollout | head
sed -n '1,80p' config/src/merge_tests.rs          # 读别人的用例结构与断言粒度
```

- 看**用例名**：`cargo test -p codex-config -- --list`（需在上游仓库里跑）或直接 `rg '^\s*fn \w+\('`，
  一串名字就是一份行为清单。
- 看**断言**：它们断言具体值还是 `is_ok()`？断言具体值的部分才值得抄结构。
- **移植一个用例**：输入照抄，期望值照抄，实现自己写。这叫"规格移植"，不算抄代码，
  但要在 NOTES 里写清 `path:line` 来源。
- 看到上游测试覆盖了但你没想到的边界（空列表、重复键、`~` 展开、相对路径）→ 直接加进自己的清单。

### 5.4 对照实验（拿真 codex 当 oracle）

拿不准上游行为时，别猜、别问模型——跑一次真的，把输出冻结成 fixture。

```bash
# 例：apply_patch 对"上下文不匹配"的真实反应
cd /tmp && rm -rf oracle && mkdir oracle && cd oracle && git init -q
printf 'a\nb\nc\n' > f.txt
codex exec --sandbox danger-full-access --ephemeral -m gpt-6-astra \
  -c model_reasoning_effort=low --json \
  "在 f.txt 里把 b 改成 B，只用 apply_patch" > patch.jsonl 2>&1
```

- 把关键片段（工具调用参数、事件顺序、错误消息）挑出来存进 `tests/fixtures/`。
- 我们的实现要在同样输入下产出同样形状的输出，测试就比 fixture。
- 成本：一次配额。所以只对**语义歧义点**做，不要拿它当调试器。
- 记录进台账：`oracle: apply_patch 上下文不匹配 → 整体失败且不写入`。这就是一条 E0 级的规格。

### 5.5 好测试的六个特征

1. **名字是句子**：`unknown_key_is_rejected_with_path`，不是 `test_config_2`。
2. **一个测试一个行为**，三段用空行分开（准备 / 动作 / 断言）。
3. **断言具体值**：`assert_eq!` 优于 `assert!`，`assert!(x.is_ok())` 几乎等于没测。
4. **失败信息能自解释**：断言带上输入，`assert_eq!(got, want, "input={:?}", input)`。
5. **不依赖外部状态**：时间、随机、`$HOME`、当前目录都要注入或临时目录（`tempfile`）。
6. **测试之间不共享可变状态**：每个测试自己造目录、自己清理；不要依赖执行顺序。

### 5.6 什么时候不该写测试

- 一行 getter、纯数据构造、`Display` 转发——写了只是噪音。
- 明显由类型系统保证的事（字段是否存在），不要用测试重复一遍。
- 还在探索 API 形状时：先写 3 个粗测试定形状，形状稳了再补细节。
- 一个模块的测试规模参考：**8–15 个**，其中至少 3 个错误路径、3 个边界。
  超过 25 个通常说明模块切分过粗，或者你在测实现细节而不是行为。

### 5.7 常见坑

| 坑 | 症状 | 改法 |
|----|------|------|
| 空洞断言 | `assert!(result.is_ok())` | 断言里面具体的值 |
| 测实现 | 测试名里出现函数名/私有结构 | 改成描述外部可见行为 |
| 过度 mock | 为了测一个函数要 mock 四个 trait | 说明耦合错了，先拆接口 |
| 顺序依赖 | 单独跑绿、全量跑红 | 每个测试自己建 `TempDir` |
| 时间脆弱 | 偶发失败 | 注入 clock，或断言范围而非精确值 |
| 只测 happy path | 覆盖率好看但没测错误分支 | 强制清单里 3 条错误路径 |
| 改测试求绿 | 断言被放宽/用例被 `#[ignore]` | 这是唯一真正的失败，回滚 |

---

## 6. 验收（四步，不搞门禁矩阵）

任务结束时 Hermes 自己做这四件事，全过才提交：

1. **看范围**：`git status --short` + `git diff --stat`，对照 brief 的路径白名单。越界就回滚那一块。
2. **自己跑**：`./x.sh`（fmt + clippy + `test --workspace`）。贴真实末尾 10 行，不接受"全部通过"。
3. **读测试**：抽 2 个测试读断言（不是读名字），确认断的是值、有错误路径。
4. **听人讲**：请学习者用三句话复述这个模块——讲不出就说明这轮该停下来重读，而不是继续 M+1。

补充：
- 抽一条最关键的语义，自己 `rg` 到 `codex-src/…:line` 读原文，确认 NOTES 没写错。
- Codex 在回话里提到的"顺手优化/额外发现"，一律不进本次 diff，记到 §11 待办。
- 检查 diff 里有没有 `unwrap()`/`let _ =` 把错误吞掉——这不是洁癖，是错误路径没被测到的信号。

---

## 7. 任务台账（追加，不删改）

状态：`PASS` / `FAIL` / `BLOCKED` / `SUPERSEDED`。任务号单调递增。

| 任务 | 日期 | 里程碑 | 摘要 | effort | 状态 | 证据 |
|------|------|--------|------|--------|------|------|
| MC-0000 | 2026-11-03 | M0 | 建立协作文档 + `AGENTS.md`；锁定上游 f9e7b19 | — | PASS | 本文件；`git log` |
| MC-0001 | 2026-11-03 | M0 | 冒烟：验证 `AGENTS.md` 真被注入、派单 flag 可用 | low | PASS | E0 见 §10 |
| MC-0002 | 2026-11-03 | M0 | 文档改版：约束瘦身 + 新增「怎么测试」教学 | — | PASS | §5；§12 |
| MC-0003 | 2026-11-03 | M0 | 装 Rust 工具链并验证 serde 编译 | — | PASS | E0 见 §10 |
| MC-0004 | 2026-11-03 | M0 | 骨架：workspace + 9 空 crate + x.sh + 工具链钉版本 | low | PASS | E0 见 §10 |

### 条目模板

```markdown
### MC-NNNN · <YYYY-MM-DD> · <里程碑> <模块>
- 目标：
- effort：low|medium|high（high 需理由）
- 命令：<实际执行的 codex exec 行>
- 结果：PASS|FAIL|BLOCKED
- 证据：E0 <命令 + 末尾输出要点> / E1 <path:line>
- 学到：<这个模块原来以为 X，实际是 Y>
- 遗留：<§11 条目号>
```

「学到」这一栏比「结果」重要。台账没有这一栏，说明这轮只是在推进度。

---

## 8. 裁决表（只追加；作废标 SUPERSEDED，不删）

| 编号 | 裁决 | 理由 | 状态 |
|------|------|------|------|
| R-001 | 上游锁定 `f9e7b19`，升级需新裁决 | 比对必须有唯一点，否则结论无法复用 | ACTIVE |
| R-002 | 派单一律 `--sandbox danger-full-access` | WSL+root 下 workspace-write 常失败 | ACTIVE |
| R-003 | 不引入上游 crate、不 vendor 上游代码 | 否则"学习重写"退化成"依赖搬运" | ACTIVE |
| R-004 | 依赖基线限于 `AGENTS.md` §6 所列 | 一个依赖就是一片要读的代码 | ACTIVE |
| R-005 | 策略引擎手写解析器，不用 starlark crate | 我们只需前缀规则子集，手写更可读 | ACTIVE |
| R-006 | 约束只写"会真出问题"的，不写礼节性规则 | 规则越密越没人读；教学靠 §5，不靠禁令 | ACTIVE |
| R-007 | `rust-toolchain.toml` 钉版本号（现为 `1.99.1`），不退化成 `stable` | 学习重写要比对行为，编译器漂移会制造假差异；钉版本的前提是把该版本 toolchain 真装出来 | ACTIVE |

---

## 9. 漂移体检（每 5 个任务或里程碑收尾一次）

逐项回答，任何一项"否"就停下修流程，别继续堆功能：

1. 最近 5 个任务的 diff 是否都在 brief 的路径白名单内？
2. NOTES 里有没有不带 `path:line` 的结论（E3 冒充 E1）？
3. 当前 LOC 是否还在 AGENTS.md §3 的预算内？
4. 最近有没有"改测试求绿"的痕迹？
5. **你能不能不看代码讲清上一个模块？** 不能 → 速度超过理解，降速。

体检结论写进 §10「体检记录」，一条一行。

---

## 10. 体检记录 & 任务明细（追加区）

### 体检记录

| 日期 | 范围 | 结论 | 动作 |
|------|------|------|------|
| 2026-11-03 | 立项 | 目录为空，无代码可体检 | 先立文档，再装工具链 |

### 任务明细

#### MC-0001 · 2026-11-03 · M0 冒烟验证
- 目标：确认 `AGENTS.md` 在 `codex exec` 下被真实注入，且 §3.2 的 flag 组合可用。
- effort：low
- 命令：`codex exec --sandbox read-only --ephemeral -C /home/seagull/MiniCodex -m gpt-6-astra -c model_reasoning_effort=low --json -o .collab/out/smoke.md "只读任务：…引用 R1/R9…"`
  （R1/R9 是改版前的规则编号，现已收敛为 R1–R7，见 §12）
- 结果：PASS（exit 0）
- 证据：E0 — 模型逐字引用了 `AGENTS.md` 的规则行，并正确报出 M2 的 `800–1200` 行与三个上游锚点路径；
  原始事件流 `.collab/raw/smoke.jsonl`（不入库）。
- 学到：`AGENTS.md` 注入链路有效，可作为长期约束入口；`--ephemeral` 不落会话文件，适合探针。
- 遗留：无

#### MC-0003 · 2026-11-03 · 工具链
- 目标：装上 Rust 工具链，解锁 M0。
- 过程：默认走 `static.rust-lang.org` 失败（本机不可达）→ 改用清华镜像直接取 `rustup-init`；
  第一次仍超时，原因是 shell 里 7897 代理是坏的而 rustup 会读 `https_proxy` →
  `env -u http_proxy -u https_proxy -u all_proxy` 后成功。
- 结果：PASS
- 证据：E0 —
  `rustc 1.99.1 (58cf8f9c3 2026-10-13)` / `cargo 1.99.1` / `clippy 0.1.99` / `rustfmt 1.10.0-stable`；
  `cargo new` 探针工程加 `serde`+`serde_json` 后 `cargo build` 退出码 0（`crates.io` 直连可用）。
- 学到：这台机器"GitHub 能连、Rust 官方源不能连、crates.io 能连、7897 代理是死的"——
  以后再遇到装不上，先看代理变量，再看镜像，不要以为是网络全断。
- 遗留：无（P0-1 关闭）

#### MC-0004 · 2026-11-03 · M0 骨架
- 目标：workspace + 9 个空 crate + `x.sh` + `rust-toolchain.toml`，不含业务逻辑。
- effort：low（机械任务，符合 §4 预算）
- 命令：`codex exec --sandbox danger-full-access -C /home/seagull/MiniCodex -m gpt-6-astra -c model_reasoning_effort=low --json -o .collab/out/MC-0004.md "$(cat .collab/briefs/MC-0004.md)"`
- 结果：PASS（Codex 先 BLOCKED，Hermes 修环境后通过）
- 证据：E0 —
  - Codex 产出 21 个文件，`lib.rs` 只有一行 `//!` 说明，`x.sh` 可执行，`cargo tree` 只有 9 个本地 crate；
  - `./x.sh` 退出码 0（fmt 干净 + clippy 无警告 + 9 个 crate 共 0 个测试全部 ok）；
  - `cargo build --workspace --all-targets` 无警告；`cargo fmt --all --check` 干净；
  - 全仓库无 `[dependencies]`，即零外部依赖。
- 过程：Codex 按停止条件正确停下——`rust-toolchain.toml` 钉 `1.99.1`，但 rustup 只注册了 `stable`，
  门禁报 `toolchain '1.99.1-…' is not installed`。它没有改 `x.sh` 的严格度，只交报告，这是对的行为。
  修复发生在 Hermes 侧：装上版本化的 `1.99.1` 工具链，`x.sh` 随即全绿。
- 学到：**「钉版本」的前提是那个版本真的装得出来**。`rustc --version` 显示 1.99.1 不等于有
  `1.99.1` 这个 toolchain；只有 `stable` 别名时，钉版本的表单会把所有人挡在门外。
- 遗留：无（P1-1 关闭）

#### MC-0002 · 2026-11-03 · 文档改版
- 目标：用户指出原版过度防御、缺"怎么测试"。改为教学导向。
- 改动：`AGENTS.md` 12 条铁律 → 7 条（每条带"为什么"），删掉"违反后果/回滚"判决语气与禁止清单；
  本文件新增 §5 测试教学（七级阶梯、TDD 循环、读上游测试、对照实验、好测试六特征、何时不写、常见坑），
  验收从 G1–G6 矩阵收敛为四步，新增裁决 R-006（约束只写会真出问题的）。
- 结果：PASS
- 证据：本文件 §5/§6/§12；`git log`
- 学到：反作弊式的门禁解决的是"信任问题"，而这里的问题是"理解问题"——工具选错了，规则越多越无效。
- 遗留：P0-1 工具链

---

## 11. 未决事项 / 下一步（可改写，保持短）

| 编号 | 事项 | 级别 | 备注 |
|------|------|------|------|
| P0-1 | ~~安装 Rust 工具链~~ | 已完成 2026-11-03 | rustc/cargo 1.99.1 + clippy + rustfmt，见 MC-0003 |
| P1-1 | ~~建 `x.sh`（fmt + clippy + test）~~ | 已完成 2026-11-03 | 见 MC-0004；版本化 toolchain 1.99.1 已装 |
| P1-2 | 写 `docs/SPEC-M1.md`（mc-protocol 子集边界） | M1 前置 | 先定子集，否则必漂移 |
| P2-1 | `mc-agent` 用 tokio 还是同步阻塞 | M7 前 | 影响依赖基线（R-004） |
| — | ~~`git init` + `.gitignore`~~ | 已完成 | 2026-11-03 |

---

## 12. 文档变更日志

| 日期 | 变更 | 原因 |
|------|------|------|
| 2026-11-03 | 建立本文件：E0–E3 分级、G1–G6 门禁、effort 预算、裁决 R-001–R-005 | 立项；把"Codex 文字不可作指导"落成流程 |
| 2026-11-03 | 改版：约束瘦身；新增 §5 测试教学；验收改四步；加 R-006 | 过度防御偏离目标——这是给人学，不是给机器审 |
