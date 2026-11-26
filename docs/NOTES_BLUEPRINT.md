# 双笔记收口清单

每阶段先由学习者完成代码，在 MiniCodex 根目录执行表中命令并完成 Code Review。届时从真实实现留下一段运行轨迹、一个失败用例和一项 Rust 取舍，再进入站点写作。合写的 Rust 章节在前一阶段保留审阅素材，后一阶段通过后成稿。

| 阶段验收 | `docs/mini-codex/`，一阶段一篇 | `docs/rust-notes/`，共五篇 |
|---|---|---|
| `./x.sh s1` | 1 · 工具出错后模型如何纠正 | 1 · 用枚举、借用和 Result 表达一轮对话 |
| `./x.sh s2` | 2 · 工具发现、派发与受限 Shell | 2 · 从泛型执行器到异构工具注册表 |
| `./x.sh s3` | 3 · 从命令参数到审批决定 | 为第 3 篇留存切片匹配和闭包审批用例 |
| `./x.sh s4` | 4 · 多文件补丁何时可以写盘 | 3 · 用显式类型表达策略结果和补丁错误（S3–S4） |
| `./x.sh s5` | 5 · 分层配置与项目指令的来源 | 4 · 用 Path、Option 与递归值装配上下文 |
| `./x.sh s6` | 6 · SSE 字节如何成为一次模型回复 | 为第 5 篇留存跨块解析和异步回调用例 |
| `./x.sh s7` | 7 · 中断与恢复各保留了什么 | 5 · Future、通道和协作式取消（S6–S7） |

- [ ] 在 `/home/seagull/seagull-llm` 按 `AGENTS.md`、`HTML_FORMATS.md`、`writing/README.md` 和 `writing/publishing.md` 核对体例与发布流程。每篇配套 HTML 正文和 Markdown 元数据，使用 `data-design="linux-notebook"`。
- [ ] 首篇机制笔记和栏目首页同时建立。按实际文章接入栏目 `index.html`、`sidebar.ts`、`content-catalog.ts`、`sections.json` 和 `config.mts`，核对章节顺序和双向入口。
- [ ] 站点内容定稿后在站点根目录依次运行以下四项验证。首次建栏另跑 `npm run test:styles`，从栏目和侧边栏分别打开实际文章核对页面。

```bash
npm run tags
npm run test:content
npm run build
node scripts/test-content-discovery.mjs --built
```
