use std::collections::VecDeque;
use std::fs;
use std::future::pending;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use minicodex::llm::{AsyncLlmClient, LlmError, StreamDelta, Usage};
use minicodex::rollout::{RolloutError, RolloutRecord, RolloutStore};
use minicodex::session::{Event, Op, Session, SessionHandle, SessionMode};
use minicodex::tools::{Tool, ToolRegistry, ToolSpec};
use minicodex::{Message, ModelTurn, ToolCall};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::time::timeout;

fn store(root: &TempDir) -> RolloutStore {
    RolloutStore::new(root.path().join("history.jsonl"))
}

fn tool_call() -> ToolCall {
    ToolCall {
        call_id: "c1".into(),
        name: "count".into(),
        arguments: json!({}),
    }
}

fn tool_history() -> Vec<Message> {
    vec![
        Message::User("报数".into()),
        Message::Assistant {
            text: None,
            tool_calls: vec![tool_call()],
        },
        Message::ToolResult {
            call_id: "c1".into(),
            tool_name: "count".into(),
            output: "1".into(),
            is_error: false,
        },
        Message::Assistant {
            text: Some("共一次".into()),
            tool_calls: vec![],
        },
    ]
}

struct Counter(Arc<AtomicUsize>);

impl Tool for Counter {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "count".into(),
            description: "报数".into(),
            parameters: json!({"type":"object"}),
        }
    }

    fn call(&self, _: Value) -> Result<String, String> {
        Ok((self.0.fetch_add(1, Ordering::SeqCst) + 1).to_string())
    }
}

struct Step {
    delta: &'static str,
    // None 表示模型已发出首段，连接依然挂起。
    reply: Option<ModelTurn>,
}

struct Model {
    first_request: Option<Vec<Message>>,
    steps: VecDeque<Step>,
}

impl AsyncLlmClient for Model {
    async fn sample(
        &mut self,
        history: &[Message],
        _: &[ToolSpec],
        on_delta: &mut (dyn FnMut(StreamDelta) + Send),
    ) -> Result<(ModelTurn, Usage), LlmError> {
        if let Some(expected) = self.first_request.take() {
            assert_eq!(history, expected);
        }
        let step = self.steps.pop_front().expect("出现剧本外的采样请求");
        if !step.delta.is_empty() {
            on_delta(StreamDelta::Text(step.delta.into()));
        }
        match step.reply {
            Some(turn) => Ok((turn, Usage::default())),
            None => pending().await,
        }
    }
}

async fn next(handle: &mut SessionHandle) -> Event {
    timeout(Duration::from_secs(5), handle.events.recv())
        .await
        .expect("等待事件超时")
        .expect("会话提前关闭")
}

async fn submit(handle: &SessionHandle, text: &str) {
    handle.ops.send(Op::UserTurn(text.into())).await.unwrap();
}

#[tokio::test]
async fn appends_one_json_line_per_message_and_replays_in_order() {
    let dir = TempDir::new().unwrap();
    let log = store(&dir);
    let mut messages = vec![Message::System("第一行\n第二行".into())];
    messages.extend(tool_history());
    for message in &messages {
        log.append(&RolloutRecord {
            message: message.clone(),
        })
        .unwrap();
    }
    let raw = fs::read_to_string(&log.path).unwrap();
    assert!(raw.ends_with('\n'));
    let records: Vec<RolloutRecord> = raw
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), messages.len());
    let loaded = log.load().unwrap();
    assert_eq!(loaded.records, records);
    assert_eq!(loaded.skipped_bad_lines, 0);
    assert_eq!(loaded.replay(), messages);
}

#[tokio::test]
async fn skips_blank_malformed_and_wrong_shape_lines_and_keeps_later_records() {
    let dir = TempDir::new().unwrap();
    let log = store(&dir);
    let first = RolloutRecord {
        message: Message::User("前".into()),
    };
    let last = RolloutRecord {
        message: Message::User("后".into()),
    };
    let fixture = format!(
        "{}\n  \n{{broken\n{{}}\n{}",
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&last).unwrap()
    );
    fs::write(&log.path, fixture).unwrap();
    let loaded = log.load().unwrap();
    assert_eq!(loaded.skipped_bad_lines, 3);
    assert_eq!(loaded.records, vec![first, last]);
}

#[tokio::test]
async fn zero_byte_log_returns_a_distinct_error() {
    let dir = TempDir::new().unwrap();
    let log = store(&dir);
    fs::write(&log.path, []).unwrap();
    assert!(matches!(log.load(), Err(RolloutError::EmptyFile)));
}

#[tokio::test(start_paused = true)]
async fn resume_error_reaches_the_frontend_and_closes_the_event_stream() {
    let dir = TempDir::new().unwrap();
    let log = store(&dir);
    fs::write(&log.path, []).unwrap();
    let model = Model {
        first_request: None,
        steps: VecDeque::new(),
    };
    let mut handle = Session {
        client: model,
        tools: ToolRegistry::new(),
        store: log,
        max_steps: 1,
    }
    .spawn(SessionMode::Resume);
    assert!(matches!(next(&mut handle).await, Event::Error { message } if !message.is_empty()));
    assert_eq!(
        timeout(Duration::from_secs(5), handle.events.recv())
            .await
            .unwrap(),
        None
    );
}

#[tokio::test(start_paused = true)]
async fn resume_supplies_history_to_model_without_executing_old_calls() {
    let dir = TempDir::new().unwrap();
    let log = store(&dir);
    let mut history = tool_history();
    for message in &history {
        log.append(&RolloutRecord {
            message: message.clone(),
        })
        .unwrap();
    }
    history.push(Message::User("继续".into()));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(Counter(calls.clone())));
    let model = Model {
        first_request: Some(history),
        steps: [Step {
            delta: "",
            reply: Some(ModelTurn::text("收到")),
        }]
        .into(),
    };
    let mut handle = Session {
        client: model,
        tools,
        store: log,
        max_steps: 2,
    }
    .spawn(SessionMode::Resume);
    submit(&handle, "继续").await;
    assert_eq!(next(&mut handle).await, Event::TurnStarted);
    assert_eq!(
        next(&mut handle).await,
        Event::TurnCompleted {
            final_text: "收到".into()
        }
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn turn_streams_progress_executes_one_tool_and_persists_before_completion() {
    let dir = TempDir::new().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(Counter(calls.clone())));
    let model = Model {
        first_request: Some(vec![Message::User("报数".into())]),
        steps: [
            Step {
                delta: "",
                reply: Some(ModelTurn::with_tools(None, vec![tool_call()])),
            },
            Step {
                delta: "共一次",
                reply: Some(ModelTurn::text("共一次")),
            },
        ]
        .into(),
    };
    let mut handle = Session {
        client: model,
        tools,
        store: store(&dir),
        max_steps: 2,
    }
    .spawn(SessionMode::New);
    submit(&handle, "报数").await;
    for event in [
        Event::TurnStarted,
        Event::ToolCallBegin {
            call_id: "c1".into(),
            name: "count".into(),
        },
        Event::ToolCallEnd {
            call_id: "c1".into(),
            output: "1".into(),
            is_error: false,
        },
        Event::TextDelta {
            text: "共一次".into(),
        },
        Event::TurnCompleted {
            final_text: "共一次".into(),
        },
    ] {
        assert_eq!(next(&mut handle).await, event);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(store(&dir).load().unwrap().replay(), tool_history());
}

#[tokio::test(start_paused = true)]
async fn interrupt_ends_a_pending_sample_and_the_next_turn_can_complete() {
    let dir = TempDir::new().unwrap();
    let model = Model {
        first_request: None,
        steps: [
            Step {
                delta: "已开始",
                reply: None,
            },
            Step {
                delta: "",
                reply: Some(ModelTurn::text("第二轮")),
            },
        ]
        .into(),
    };
    let mut handle = Session {
        client: model,
        tools: ToolRegistry::new(),
        store: store(&dir),
        max_steps: 1,
    }
    .spawn(SessionMode::New);
    submit(&handle, "等中断").await;
    assert_eq!(next(&mut handle).await, Event::TurnStarted);
    // 模型要等到取消才会结束。收到增量，就是可以发中断的确定性同步点。
    assert_eq!(
        next(&mut handle).await,
        Event::TextDelta {
            text: "已开始".into()
        }
    );
    handle.ops.send(Op::Interrupt).await.unwrap();
    assert_eq!(next(&mut handle).await, Event::TurnAborted);
    submit(&handle, "再来").await;
    assert_eq!(next(&mut handle).await, Event::TurnStarted);
    assert_eq!(
        next(&mut handle).await,
        Event::TurnCompleted {
            final_text: "第二轮".into()
        }
    );
}

#[tokio::test]
async fn every_event_round_trips_as_one_json_line() {
    let events = [
        Event::TurnStarted,
        Event::TextDelta {
            text: "换\n行".into(),
        },
        Event::ToolCallBegin {
            call_id: "t".into(),
            name: "count".into(),
        },
        Event::ToolCallEnd {
            call_id: "t".into(),
            output: "坏\n参数".into(),
            is_error: true,
        },
        Event::TurnCompleted {
            final_text: "好".into(),
        },
        Event::TurnAborted,
        Event::Error {
            message: "错误".into(),
        },
    ];
    for event in events {
        let line = serde_json::to_string(&event).unwrap();
        assert_eq!(line.lines().count(), 1);
        let value: Value = serde_json::from_str(&line).unwrap();
        assert!(value["type"].is_string());
        assert_eq!(serde_json::from_value::<Event>(value).unwrap(), event);
    }
}
