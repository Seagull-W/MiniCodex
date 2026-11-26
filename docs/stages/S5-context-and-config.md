# S5 分层配置与项目指令注入

在同一个工作目录启动两次 MiniCodex，配置文件和 CLI 参数相同时，模型应收到相同的设置与项目说明。设置影响程序行为，项目说明作为文本进入 S1 的 `Message::System`。文件读取和文本组装各有独立的同步接口。[context.rs](../../src/context.rs)

## 配置从哪里来

`load_layered_config` 接受用户配置文件路径、项目配置文件路径以及只含显式 CLI 参数的 `toml::Table`。默认值来自 `AppConfig::default()`。按默认、用户、项目、CLI 的顺序合并，最后一次性转换为有类型的 `AppConfig`。

| 字段 | 内置值 | 使用处 |
|---|---|---|
| `model`、`base_url` | `gpt-4o-mini`、`https://api.openai.com/v1` | S6 的模型名与服务地址 |
| `max_steps` | `8` | S1 单轮采样上限 |
| `shell_timeout_secs` | `30` | S2 子进程超时 |
| `approval_mode` | `on_request` | S3 遇到 Ask 时可请求审批，`never` 表示不发起审批，遇到 Ask 即拦截 |

上层只覆盖它提供的键。例如用户层指定超时 19 秒和 3 步，项目层只把步数改为 5，CLI 只指定模型名。最终仍保留用户层的 19 秒、项目层的 5 步与内置服务地址。

`merge_toml` 是纯值操作。两个值均为表时递归合并同名键，保留各自独有的键。其他组合直接取上层整个值，数组也整体替换。可选文件未指定或不存在时跳过该层。已有文件读取失败或 TOML 语法损坏时，错误携带文件路径。最终值不能反序列化为 `AppConfig` 时返回配置错误。

## 工作目录会读到哪些规则

假设从 `repo/src/parser/` 运行，目录形状如下。

```text
repo/
  .git/
  AGENTS.md
  src/
    AGENTS.md
    AGENTS.override.md
    parser/
      AGENTS.md
```

`discover_agents_md` 从启动目录向上找最近的 `.git` 标记（目录或 worktree 的标记文件），再按根到叶的方向读取。上例依次得到根目录的 `AGENTS.md`、`src/AGENTS.override.md`、`parser/AGENTS.md`。同层优先选择 override，祖先的规则随同保留。没有 Git 标记时，只检查启动目录。每条 `AgentsDoc` 带所选文件的路径和 UTF-8 正文。

`max_bytes` 是本次发现中所有正文共享的预算，计量单位是字节。按目录顺序分配，最后一份可用部分取最长的合法 UTF-8 前缀，遇到截断就结束收集。首个字符也放不下时省略这份文档，预算为零时返回空列表。路径和提示词标签在正文预算之外。例如预算是 7 字节，第一份占 3 字节，第二份为“中文!”。余下 4 字节只能容纳“中”。

## 组装与验收

`build_system_prompt` 先写基础提示词和当前工作目录，再按传入顺序写每份文件的路径与正文，每段之间明确分隔。调用方可把结果作为 `Agent::new` 的 `system_prompt`，传入 `config.max_steps`。格式细节由实现者决定，测试只检查来源和正文的次序。

实现四处练习后运行 `./x.sh s5`。测试用嵌套的 Git 根区分规则来源，用缺失和损坏的临时配置文件区分可选层与错误。审阅时，用测试里的中文样本说明为何切片端点必须同时满足字节预算和字符边界。[行为验收](../../tests/s5_context_and_config.rs)
