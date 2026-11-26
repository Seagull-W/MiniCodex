use minicodex::{
    Agent, AgentError, FnToolExecutor, Message, ModelTurn, ScriptedClient, ToolCall, TurnOutcome,
};
use serde_json::json;

#[test]
fn single_step_direct_answer_without_tools() {
    let client = ScriptedClient::new([Ok(ModelTurn::text("你好，我是 MiniCodex。"))]);
    let tools = FnToolExecutor::new(|_| unreachable!("本用例不应触发任何工具调用"));
    let mut agent = Agent::new(client, tools, Some("保持简洁".into()), 4);

    let outcome = agent.run_turn("打个招呼").expect("单步对话应成功");

    assert_eq!(
        outcome,
        TurnOutcome {
            final_text: "你好，我是 MiniCodex。".into(),
            steps: 1,
            tool_calls_executed: 0,
        }
    );
    assert_eq!(
        agent.history(),
        &[
            Message::System("保持简洁".into()),
            Message::User("打个招呼".into()),
            Message::Assistant {
                text: Some("你好，我是 MiniCodex。".into()),
                tool_calls: vec![],
            },
        ]
    );
}

#[test]
fn multi_step_tool_loop_feeds_results_back_to_model() {
    let call = ToolCall {
        call_id: "call_1".into(),
        name: "read_version".into(),
        arguments: json!({ "file": "Cargo.toml" }),
    };
    let client = ScriptedClient::new([
        Ok(ModelTurn::with_tools(
            Some("先查一下版本号".into()),
            vec![call.clone()],
        )),
        Ok(ModelTurn::text("当前版本是 0.1.0")),
    ]);
    let tools = FnToolExecutor::new(|c| {
        assert_eq!(c.name, "read_version");
        Ok("0.1.0".into())
    });
    let mut agent = Agent::new(client, tools, None, 5);

    let outcome = agent.run_turn("现在是什么版本？").expect("多步循环应成功");

    assert_eq!(
        outcome,
        TurnOutcome {
            final_text: "当前版本是 0.1.0".into(),
            steps: 2,
            tool_calls_executed: 1,
        }
    );

    // 验证发给模型的两次请求快照：第二次必须带着工具调用和工具结果
    let requests = agent.client().recorded_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], vec![Message::User("现在是什么版本？".into())]);
    assert_eq!(
        requests[1],
        vec![
            Message::User("现在是什么版本？".into()),
            Message::Assistant {
                text: Some("先查一下版本号".into()),
                tool_calls: vec![call],
            },
            Message::ToolResult {
                call_id: "call_1".into(),
                tool_name: "read_version".into(),
                output: "0.1.0".into(),
                is_error: false,
            },
        ]
    );
}

#[test]
fn multiple_tool_calls_in_one_step_preserve_order_and_call_ids() {
    let call_a = ToolCall {
        call_id: "c_a".into(),
        name: "echo".into(),
        arguments: json!({ "value": "first" }),
    };
    let call_b = ToolCall {
        call_id: "c_b".into(),
        name: "echo".into(),
        arguments: json!({ "value": "second" }),
    };
    let client = ScriptedClient::new([
        Ok(ModelTurn::with_tools(
            None,
            vec![call_a.clone(), call_b.clone()],
        )),
        Ok(ModelTurn::text("两条结果都收到了")),
    ]);
    let tools = FnToolExecutor::new(|c| {
        let val = c.arguments["value"].as_str().unwrap_or_default();
        Ok(format!("echo:{val}"))
    });
    let mut agent = Agent::new(client, tools, None, 4);

    let outcome = agent.run_turn("连续调用两次 echo").expect("批量工具应成功");

    assert_eq!(outcome.steps, 2);
    assert_eq!(outcome.tool_calls_executed, 2);

    let second_request = &agent.client().recorded_requests()[1];
    assert_eq!(
        second_request,
        &vec![
            Message::User("连续调用两次 echo".into()),
            Message::Assistant {
                text: None,
                tool_calls: vec![call_a, call_b],
            },
            Message::ToolResult {
                call_id: "c_a".into(),
                tool_name: "echo".into(),
                output: "echo:first".into(),
                is_error: false,
            },
            Message::ToolResult {
                call_id: "c_b".into(),
                tool_name: "echo".into(),
                output: "echo:second".into(),
                is_error: false,
            },
        ]
    );
}

#[test]
fn tool_error_is_recorded_in_history_for_model_self_correction() {
    let bad_call = ToolCall {
        call_id: "call_bad".into(),
        name: "read_file".into(),
        arguments: json!({ "path": "missing.txt" }),
    };
    let good_call = ToolCall {
        call_id: "call_good".into(),
        name: "read_file".into(),
        arguments: json!({ "path": "README.md" }),
    };
    let client = ScriptedClient::new([
        Ok(ModelTurn::with_tools(None, vec![bad_call.clone()])),
        Ok(ModelTurn::with_tools(None, vec![good_call.clone()])),
        Ok(ModelTurn::text("已改读 README.md 并完成")),
    ]);
    let tools = FnToolExecutor::new(|c| match c.arguments["path"].as_str() {
        Some("README.md") => Ok("# MiniCodex".into()),
        Some(other) => Err(format!("file not found: {other}")),
        None => Err("missing path".into()),
    });
    let mut agent = Agent::new(client, tools, None, 5);

    let outcome = agent
        .run_turn("读一下说明文档")
        .expect("工具报错不应终止循环，应交由模型自纠");

    assert_eq!(
        outcome,
        TurnOutcome {
            final_text: "已改读 README.md 并完成".into(),
            steps: 3,
            tool_calls_executed: 2,
        }
    );

    // 第二步请求里必须把第一步的工具错误标记为 is_error: true 传给模型
    let second_request = &agent.client().recorded_requests()[1];
    assert_eq!(
        second_request.last(),
        Some(&Message::ToolResult {
            call_id: "call_bad".into(),
            tool_name: "read_file".into(),
            output: "file not found: missing.txt".into(),
            is_error: true,
        })
    );
}

#[test]
fn model_error_aborts_turn_immediately() {
    let client = ScriptedClient::new([Err(AgentError::Model("rate limit exceeded".into()))]);
    let tools = FnToolExecutor::new(|_| unreachable!("模型报错后不应调用工具"));
    let mut agent = Agent::new(client, tools, None, 5);

    let err = agent
        .run_turn("触发模型错误")
        .expect_err("模型故障必须向上传播");

    assert_eq!(err, AgentError::Model("rate limit exceeded".into()));
}

#[test]
fn exceeding_max_steps_stops_infinite_tool_loop() {
    let looping_call = ToolCall {
        call_id: "loop_1".into(),
        name: "ping".into(),
        arguments: json!({}),
    };
    let client = ScriptedClient::new([
        Ok(ModelTurn::with_tools(None, vec![looping_call.clone()])),
        Ok(ModelTurn::with_tools(None, vec![looping_call.clone()])),
        Ok(ModelTurn::with_tools(None, vec![looping_call])),
    ]);
    let tools = FnToolExecutor::new(|_| Ok("pong".into()));
    let mut agent = Agent::new(client, tools, None, 2);

    let err = agent
        .run_turn("一直 ping 下去")
        .expect_err("超过 max_steps 必须中止");

    assert_eq!(err, AgentError::MaxStepsExceeded { max_steps: 2 });
    assert_eq!(agent.client().recorded_requests().len(), 2);
}

#[test]
fn zero_max_steps_immediately_returns_max_steps_exceeded() {
    let client = ScriptedClient::new([Ok(ModelTurn::text("不应被调用"))]);
    let tools = FnToolExecutor::new(|_| unreachable!());
    let mut agent = Agent::new(client, tools, None, 0);

    let err = agent
        .run_turn("max_steps 为 0")
        .expect_err("0 步上限必须直接失败");

    assert_eq!(err, AgentError::MaxStepsExceeded { max_steps: 0 });
    assert_eq!(agent.client().recorded_requests().len(), 0);
}

#[test]
fn empty_model_response_returns_error() {
    let client = ScriptedClient::new([Ok(ModelTurn {
        text: None,
        tool_calls: vec![],
    })]);
    let tools = FnToolExecutor::new(|_| unreachable!());
    let mut agent = Agent::new(client, tools, None, 3);

    let err = agent
        .run_turn("空响应测试")
        .expect_err("既无文本又无工具调用应报错");

    assert_eq!(err, AgentError::EmptyResponse);
}

#[test]
fn consecutive_turns_share_session_history() {
    let client = ScriptedClient::new([
        Ok(ModelTurn::text("我记住了，你叫 Alice。")),
        Ok(ModelTurn::text("你刚才说你叫 Alice。")),
    ]);
    let tools = FnToolExecutor::new(|_| unreachable!());
    let mut agent = Agent::new(client, tools, Some("你是记忆良好的助手".into()), 3);

    let t1 = agent.run_turn("我叫 Alice").expect("第一轮应成功");
    let t2 = agent.run_turn("我叫什么？").expect("第二轮应成功");

    assert_eq!(t1.final_text, "我记住了，你叫 Alice。");
    assert_eq!(t2.final_text, "你刚才说你叫 Alice。");

    let second_request = &agent.client().recorded_requests()[1];
    assert_eq!(
        second_request,
        &vec![
            Message::System("你是记忆良好的助手".into()),
            Message::User("我叫 Alice".into()),
            Message::Assistant {
                text: Some("我记住了，你叫 Alice。".into()),
                tool_calls: vec![],
            },
            Message::User("我叫什么？".into()),
        ]
    );
}
