use std::time::Duration;

use minicodex::llm::{
    AsyncLlmClient, LlmError, OpenAiClient, OpenAiConfig, SseEvent, SseParser, StreamDelta,
    ToolCallAssembler, Usage,
};
use minicodex::tools::ToolSpec;
use minicodex::{Message, ModelTurn, ToolCall};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

#[test]
fn every_byte_boundary_including_inside_utf8_and_crlf_can_split_a_frame() {
    let frame = "data: 雪\r\n\r\n".as_bytes();
    for split in 0..frame.len() {
        let mut parser = SseParser::default();
        assert!(parser.feed(&frame[..split]).unwrap().is_empty());
        assert_eq!(
            parser.feed(&frame[split..]),
            Ok(vec![SseEvent::Data("雪".into())])
        );
    }
}

#[test]
fn comments_are_ignored_data_lines_join_and_done_ends_the_stream() {
    let mut parser = SseParser::default();
    let bytes = b": heartbeat\n\ndata: a\n: mid-frame\ndata: b\n\ndata: [DONE]\n\n";
    assert_eq!(
        parser.feed(bytes),
        Ok(vec![SseEvent::Data("a\nb".into()), SseEvent::Done])
    );
    assert!(parser.is_done());
    assert!(parser.feed(b"data: [DONE]\n\n").unwrap().is_empty());
}

#[test]
fn text_deltas_form_one_model_turn() {
    let mut assembler = ToolCallAssembler::default();
    assembler.push(StreamDelta::Text("早".into()));
    assembler.push(StreamDelta::Text("上好".into()));
    assert_eq!(assembler.finish(), Ok(ModelTurn::text("早上好")));
}

fn fragment(index: usize, id: Option<&str>, name: Option<&str>, arguments: &str) -> StreamDelta {
    StreamDelta::ToolCall {
        index,
        id: id.map(str::to_owned),
        name: name.map(str::to_owned),
        arguments: arguments.into(),
    }
}

#[test]
fn interleaved_indices_keep_identity_order_and_json_arguments() {
    let mut assembler = ToolCallAssembler::default();
    assembler.push(fragment(1, Some("b"), Some("write"), r#"{"value":"#));
    assembler.push(fragment(0, Some("a"), Some("read"), r#"{"path":"#));
    assembler.push(StreamDelta::Text("分两步".into()));
    assembler.push(fragment(0, None, None, r#""README"}"#));
    assembler.push(fragment(1, None, None, "42}"));
    assert_eq!(
        assembler.finish(),
        Ok(ModelTurn::with_tools(
            Some("分两步".into()),
            vec![
                ToolCall {
                    call_id: "a".into(),
                    name: "read".into(),
                    arguments: json!({"path":"README"})
                },
                ToolCall {
                    call_id: "b".into(),
                    name: "write".into(),
                    arguments: json!({"value":42})
                },
            ]
        ))
    );
}

#[test]
fn incomplete_arguments_at_end_of_stream_are_a_protocol_error() {
    let mut assembler = ToolCallAssembler::default();
    assembler.push(fragment(0, Some("a"), Some("read"), "{"));
    assert!(matches!(assembler.finish(), Err(LlmError::Protocol(_))));
}

// 离线端点接收 reqwest 的 JSON POST，在每次尝试中核对历史、鉴权与工具 Schema。
async fn accept_request(listener: &TcpListener) -> TcpStream {
    let (socket, _) = listener.accept().await.unwrap();
    let mut request = BufReader::new(socket);
    let mut line = String::new();
    request.read_line(&mut line).await.unwrap();
    assert_eq!(line, "POST /v1/chat/completions HTTP/1.1\r\n");

    let mut length = None;
    let mut authorized = false;
    loop {
        line.clear();
        assert_ne!(request.read_line(&mut line).await.unwrap(), 0);
        if line == "\r\n" {
            break;
        }
        let (name, value) = line.trim_end().split_once(':').unwrap();
        if name.eq_ignore_ascii_case("content-length") {
            length = Some(value.trim().parse::<usize>().unwrap());
        }
        if name.eq_ignore_ascii_case("authorization") {
            authorized = value.trim() == "Bearer fixture-key";
        }
    }
    assert!(authorized);
    let mut body = vec![0; length.expect("reqwest 的 JSON 请求提供 Content-Length")];
    request.read_exact(&mut body).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["model"], "fixture-model");
    assert_eq!(payload["stream"], true);
    assert_eq!(payload["stream_options"]["include_usage"], true);
    assert_eq!(
        payload["messages"],
        json!([{"role":"user", "content":"你好"}])
    );
    let function = &payload["tools"][0];
    assert_eq!(function["type"], "function");
    assert_eq!(function["function"]["parameters"], json!({"type":"object"}));
    assert_eq!(function["function"]["name"], "lookup");
    request.into_inner()
}

const FIRST: &str = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"你好\"}}]}\n\n";
const TAIL: &str = "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\ndata: [DONE]\n\n";

// 每次 HTTP 请求消费一个状态码。200 响应等到客户端交付首个增量后才发送尾帧。
async fn sample_script(
    statuses: &[u16],
) -> (Result<(ModelTurn, Usage), LlmError>, Vec<StreamDelta>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let mut client = OpenAiClient::new(
        OpenAiConfig {
            api_key: "fixture-key".into(),
            base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
            model: "fixture-model".into(),
        },
        reqwest::Client::builder().no_proxy().build().unwrap(),
    );
    let history = [Message::User("你好".into())];
    let tools = [ToolSpec {
        name: "lookup".into(),
        description: "查询".into(),
        parameters: json!({"type":"object"}),
    }];
    let (sent, mut received) = oneshot::channel();
    let mut first_delta = Some(sent);
    let mut deltas = Vec::new();
    let mut record = |delta| {
        deltas.push(delta);
        if let Some(sent) = first_delta.take() {
            sent.send(()).unwrap();
        }
    };
    let mut script = statuses.iter();
    let server = async {
        loop {
            let mut socket = accept_request(&listener).await;
            let status = *script.next().expect("客户端发送了额外请求");
            let (kind, length) = match status {
                200 => ("text/event-stream", FIRST.len() + TAIL.len()),
                _ => ("text/plain", "fixture error".len()),
            };
            let header = format!(
                "HTTP/1.1 {status} Fixture\r\nContent-Type: {kind}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
            );
            socket.write_all(header.as_bytes()).await.unwrap();
            if status == 200 {
                socket.write_all(FIRST.as_bytes()).await.unwrap();
                (&mut received).await.expect("客户端交付首段文本");
                socket.write_all(TAIL.as_bytes()).await.unwrap();
            } else {
                socket.write_all(b"fixture error").await.unwrap();
            }
        }
    };
    let response = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            result = client.sample(&history, &tools, &mut record) => result,
            () = server => unreachable!(),
        }
    })
    .await
    .expect("离线请求应在时限内结束");
    assert_eq!(script.len(), 0, "客户端遗漏了请求");
    (response, deltas)
}

#[tokio::test]
async fn retries_429_and_500_then_delivers_deltas_before_done_and_returns_usage() {
    let (response, deltas) = sample_script(&[429, 500, 200]).await;
    assert_eq!(deltas, [StreamDelta::Text("你好".into())]);
    assert_eq!(
        response,
        Ok((
            ModelTurn::text("你好"),
            Usage {
                prompt_tokens: 5,
                completion_tokens: 2
            }
        ))
    );
}

#[tokio::test]
async fn bad_request_stops_after_one_attempt() {
    let (response, deltas) = sample_script(&[400]).await;
    assert!(matches!(response, Err(LlmError::Http { status: 400, .. })));
    assert!(deltas.is_empty());
}

#[tokio::test]
async fn persistent_server_error_exhausts_three_attempts() {
    let (response, deltas) = sample_script(&[500, 500, 500]).await;
    assert!(matches!(response, Err(LlmError::Http { status: 500, .. })));
    assert!(deltas.is_empty());
}
