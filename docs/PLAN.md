# MiniCodex 整体规划

> 定位：这是**课程大纲**。`AGENTS.md` 是纪律，`docs/COLLAB.md` 是流程，本文件回答「先学什么、学到什么算会、每一步交给我看什么」。
> 修订方式：本文件可整体改写（它描述计划，不是历史），但每次改要在末尾记一行。

---

## 1. 交付物是什么

不是「一个能跑的 codex 复刻」。交付物是：

- 9 个模块，每模块 300–1500 行，**由你亲手写**；
- 每个模块配一份教学文档（`docs/modules/M<N>-*.md`），读它就能动手；
- 每个模块结束时，你能不看代码回答：数据从哪来、谁调用它、失败怎么办、边界在哪、上游为什么这么设计。

代码由 Codex 打字，但**结构由你定、语义由你判断、测试由你验收**。这就是"学会"和"抄完"的区别。

---

## 2. 课程：tracer bullet（先跑通一条细线，再逐段加厚）

完整拆解见 `docs/ARCHITECTURE.md`。顺序按"每完成一段就能看到可观察现象"排，不再按 crate 排。

| 段 | 做什么 | 依赖 | 看到什么现象 | 旧编号 |
|----|--------|------|--------------|--------|
| **R0** | 骨架 + `x.sh` | — | `./x.sh` 绿 | M0 ✅ |
| **R1** | 最小类型子集：`TurnInput`、6 个事件、4 个条目、3 个 ID | — | 手写 JSON → 事件 → 编码回 JSON，`type` 是 `task_started` | M1（缩到 40%） |
| **R2** | 假模型 + 单轮循环（脚本给 3 个事件） | R1 | 终端依次出现 `task_started` / `agent_message` / `task_complete` | M7 骨架 |
| **R3** | 工具回环：`shell` 一个工具，执行→回写→再采样 | R2 | 中间多出 `exec_command_begin/end`，`call_id` 配对 | M4 前半 |
| **R4** | `apply_patch`，失败原子化 | R3 | 上下文不匹配时文件不变 + error 事件 | M5 |
| **R5** | 配置分层 + AGENTS.md 注入 + 请求组装 | R2 | 加一行 AGENTS.md，请求体跟着变 | M2 + M3 |
| **R6** | JSONL 持久化 + 恢复 | R2 | 杀掉进程后能接着上次会话 | M6 |
| **R7** | 真模型 SSE、重试、usage | R2 | 换真客户端，事件序列形状不变 | M7 剩余 |
| **R8** | 前端收口 `--json` / `-o` / 退出码 | R7 | `mc-cli --json` 每行合法 JSON | M8 |

三条顺序理由（来自代码结构，不是偏好）：

1. **R2 必须先于细节**：`run_turn`（`turn.rs:178`）的骨架决定后面每个模块被调用的位置；先有骨架，类型的字段才有意义。
2. **R3/R4 在 R5 之前**：工具输出的形状（`FunctionCallOutput`）会影响历史与提示词，先做工具才能看懂"历史里为什么要存工具输出"。
3. **R7 最后**：SSE 是最脏的一段（重试、断流、usage、加密内容），放在假模型验证过循环之后，出错时能分清"循环错了"还是"解析错了"。

旧的 M1–M8 编号不作废：它对应 `AGENTS.md` 的 crate 骨架与 `docs/modules/` 文档名，映射见上表最后一列。

---

## 3. 每个模块的信息流水线

```
DeepWiki（E2 地图） ──┐
                      ├─→ Codex 解读（E3 草稿） ─→ Hermes 逐条核对（E1） ─→ 教学文档 ─→ 你手写 ─→ 验收
codex-src（E1 地形） ─┘                                                                    ↑
                                                              对照实验 / 测试（E0）────────┘
```

具体分工：

| 环节 | 谁做 | 产出 | 注意 |
|------|------|------|------|
| 画地图 | Hermes，用 DeepWiki + 目录扫描 | 本文件 §2、每模块的"上游在哪" | DeepWiki 是 AI 生成的二手资料，指向的 commit 和我们钉的 `f9e7b19` **不一样**，只能用来找方向 |
| 精读代码 | **Codex**（只读，禁写） | 结构化解读：类型表、不变量、错误路径、测试候选、3 个易误解点 | 它的每条结论都必须带 `path:line`；没有就作废 |
| 校对 | Hermes | 逐条 `rg` 到原文确认；错的删掉，含糊的改成具体 | 这一步不能省，否则文档会变成"看起来很有道理"的幻觉 |
| 写教学文档 | Hermes | `docs/modules/M<N>-*.md` | 面向"要动手写的人"，不是面向评审 |
| 手写代码 | **你** | `crates/mc-*/` | Codex 不代写；卡住时让 Codex 解释某段语义，不让它贴代码 |
| 验收 | Hermes + 你 | `./x.sh` + 你复述 | 你讲不清 = 没过 |

**为什么让 Codex 做解读而不是写代码**：读代码是它的强项（上下文大、不怕枯燥），但它的结论不可信，所以只让它产出"待核对的草稿"；写代码是学习本身，必须你自己来。

---

## 4. 教学文档模板（`docs/modules/M<N>-<name>.md`）

每个模块一份，固定七节。这个模板的作用是：你打开文档就知道今天要做什么，不需要再问我。

```markdown
# M<N> <crate 名>

## 0. 一句话 + 位置
这个模块在整条链路的哪个环节；依赖谁、被谁依赖（模块名，不是文件名）。

## 1. 上游在哪（只读这些）
| 文件 | 行数 | 读它为了什么 |
阅读顺序：1→2→3，每个文件读完在 §3 里补一条结论。

## 2. 数据结构
类型名 / 字段 / 为什么是这个形状 / path:line。只列子集内的，超出的明确写"不做"。

## 3. 语义规则（本项目要复刻的部分）
- 不变量：……
- 顺序：……（谁先谁后，为什么）
- 错误路径：……（每种失败长什么样，path:line）

## 4. 必须写的测试（清单）
10 条左右的测试名 + 断言什么。其中至少 3 条错误路径、3 条边界。

## 5. 手写步骤
Step 1 …（≤100 行，可编译）
Step 2 …
每一步结束跑 `./x.sh`，绿了再下一步。哪一步卡住，回到 §1 读源码，不要问 Codex 要代码。

## 6. 三个常见误解
来自 Codex 解读、经 Hermes 核对保留下来的坑。

## 7. 验收问题（答不出就不算过）
1. …（5 个左右，覆盖 数据来源/调用方/失败/边界/设计理由）
```

---

## 5. 节奏与额度

| 项目 | 约定 |
|------|------|
| 一个模块的周期 | 2–4 次专注时间（每次 1–2 小时）：读文档 → 写 Step 1–2 → 写 Step 3–N → 补测试 |
| Codex 解读派单 | 每模块 1 次，`effort=medium`（只读、产出结构化分析，不写代码） |
| Codex 答疑 | 随时，`effort=low`，问"这段为什么这样"，不许要现成实现 |
| `effort=high` | 只用于 M4 策略三态、M7 事件时序，或连续两次验收不过 |
| 全课程总量 | 9 个模块 ≈ 6–8k 行；按每周 2–3 次专注时间估，8–10 周 |

额度纪律：一次派单只问一个模块的一个问题域。把"帮我看看 protocol 里都有什么"这种开放式问题换成"这三个类型在 X 场景下的行为差异，逐条给 path:line"。

---

## 6. 进度看板

状态：`未开始` / `解读中` / `文档就绪` / `手写中` / `已验收`。

| 模块 | 文档 | 解读 | 手写 | 验收 | 备注 |
|------|------|------|------|------|------|
| M0 骨架 | — | — | — | ✅ 2026-11-03 | 已提交 1e2c1b4 |
| M1 mc-protocol | ✅ 就绪 | ✅ 3 份（MC-0005/6/7） | 待你开工 | — | 子集边界见 §7 |
| M2 mc-config | — | — | — | — | |
| M3 mc-agents-md | — | — | — | — | |
| M4 mc-exec + mc-policy | — | — | — | — | 可能需要 `unsafe` 之外的进程 API |
| M5 mc-patch | — | — | — | — | |
| M6 mc-rollout | — | — | — | — | |
| M7 mc-agent | — | — | — | — | 依赖决策 P2-1（tokio 或同步） |
| M8 mc-cli | — | — | — | — | |

---

## 7. M1 子集边界（第一份实例，用来校准后面的颗粒度；2026-11-03 按源码校订）

上游 `protocol` crate 有 3.4 万行、70+ 模块；我们只做 4 个文件里的**会话内核**：

| 我们的模块 | 上游来源 | 行数（上游） | 我们做多少 |
|-----------|----------|--------------|-----------|
| `mc_protocol::ids` | `protocol/src/session_id.rs`、`thread_id.rs`、`response_item_id.rs` | 各 <120 | 全部照做；`RolloutId` 只是 `ThreadId` 的别名 |
| `mc_protocol::items` | `models.rs` `ResponseItem`（1069–1341）、`ContentItem`（882–913）、payload（2364–2379） | 4802 | 只做 `Message / Reasoning / FunctionCall / FunctionCallOutput / Other`；`ContentItem` 只做 `InputText / OutputText` |
| `mc_protocol::turn` | `items.rs` 47–293 | 950 | 只做 `UserMessage / AgentMessage / Reasoning / CommandExecution`（tag 是 PascalCase） |
| `mc_protocol::event` | `protocol.rs` `EventMsg`（1528–1700）+ 六个事件结构 | 6803 | 只做 `Error / TurnStarted / TurnComplete / AgentMessage / ExecCommandBegin / ExecCommandEnd`；注意 `task_started`/`task_complete` |
| `mc_protocol::op` | `protocol.rs` `Op`（694–941）、`turn_input.rs` | — | 只做 `TurnInput`（去掉 `oneshot` 应答）/ `Interrupt`；**`Op` 无 serde** |

明确不做：realtime、MCP、hooks、guardian、permissions、plan_tool、review_format、openai_models、local_media、memory、apps、plugins、code_mode。

判断标准：**M7 的单轮循环用不到的，就不做。** 每次想加东西时回来对一遍这句。

教学文档：[`docs/modules/M1-protocol.md`](modules/M1-protocol.md)。

---

## 8. 变更记录

| 日期 | 变更 |
|------|------|
| 2026-11-03 | 建立本规划：三段式课程、五段信息流水线、七节教学文档模板、M1 子集边界 |
