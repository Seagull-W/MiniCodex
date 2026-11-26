# S6 · 从 SSE 字节流得到一轮模型回复

一次 HTTP 读取可能只拿到汉字的前两个字节，也可能拿到三条完整事件。S6 要让同一个响应经任意切块后都能得到相同的 `ModelTurn`，同时让调用方在响应结束前就能看到文本。同步的 S1–S5 继续使用原有接口，S7 通过 `AsyncLlmClient::sample` 接入这条异步链路。

```text
HTTP 字节块 → SseParser → Data / Done
                           ↓
                   解码 Chat Completions 增量
                           ├── on_delta → 会话订阅者
                           └── ToolCallAssembler → ModelTurn
```

## 从字节到事件

`SseParser::feed` 只接受字节，每次返回零个或多个事件。保留未完成帧的原始字节，遇空行后处理整帧，支持 LF 和 CRLF。以 `:` 开头的行是心跳或注释。同一帧内多条 `data:` 行的值用换行符连接，去掉冒号后至多一个空格。只有注释的帧没有输出。非法 UTF-8 返回 `LlmError::Protocol`。独占数据值 `[DONE]` 的帧只产生一次 `Done`，此后 `feed` 返回空列表。

先对照测试，手工标出 `data: 雪\r\n\r\n` 每个字节切分点。实现正确时，第一块无论停在“雪”的内部还是第一个换行后，都不会提前发出事件。

## 从增量到工具调用

`ToolCallAssembler::push` 接受文本和工具增量。首个片段通常给出调用 ID 和函数名，后续片段可能只含一部分参数。若索引 1 的片段先于索引 0 到达，各自的参数仍须与各自的 ID 绑定。文本按接收顺序拼接，有工具也可以同时有文本。

`finish` 消耗累积状态，从小到大按 `index` 产出 S1 的 `ToolCall`，此时才将每个完整参数串解析为 JSON。缺失 ID、名称或参数 JSON 不合法均为协议错误。直到这里，工具只是一项请求，执行权在会话层。

## 从模型配置到一次采样

1. `OpenAiConfig::from_env` 读取非空的 `OPENAI_API_KEY`。缺省的 `OPENAI_BASE_URL` 和 `MINICODEX_MODEL` 分别采用 `https://api.openai.com/v1` 和 `gpt-4o-mini`。配置错误可以指出缺少哪个变量，错误信息应保护密钥。
2. `OpenAiClient::sample` 向基址下的 `chat/completions` 发送 Bearer 鉴权 JSON POST。把 S1 的历史映射为 role 消息，工具结果以 `tool_call_id` 对应原调用。把 S2 的工具清单封装为 function 定义。请求包含模型名、`stream: true` 和用量选项 `stream_options.include_usage`。
3. 收到 429 或 5xx 状态时，最多尝试三次（含首次请求）。两次重试前分别等待 100 ms 和 200 ms。400 等其他失败状态立即返回 `LlmError::Http`。成功响应开始后逐块读取，遇到格式错误、提前 EOF 或传输中断，返回对应错误并结束此次采样。这能避免客户端已经交付的增量在重试中重复出现。
4. 每收到一条有效的内容增量就调用 `on_delta` 并送入累积器。只有角色信息的块可以略过。用量可能在 `choices: []` 的独立帧中，服务端省略用量时使用 `Usage::default()`。`[DONE]` 到达后交付完整回复与用量。回调只借用到采样 future 结束，future 支持 `Send`，便于 S7 在会话任务中驱动及取消它。

## 验收

运行 `./x.sh s6`。八个测试覆盖任意字节切分、注释及结束帧、文本拼接、工具交错、无效 JSON、两类可重试状态、400 和重试耗尽。HTTP 测试使用环回地址、临时端口与虚构密钥，主动关闭测试客户端的代理。服务端会等到回调收到第一段文本才发送 `[DONE]`，因此整包读完再调用回调的实现会被验收用例发现。[模块骨架](../../src/llm.rs) · [验收测试](../../tests/s6_llm_streaming.rs)

Review 时可以用两条测试分别指出 SSE 事件和工具调用参数各自在什么时候完整。再指出接收首段文本后，如果连接断开，调用方能保留哪部分显示、能否提交这一轮历史。
