//! S1 演示入口：用脚本化模型 + 真实读文件工具，直观演示一次带“报错自纠”的 Agent 循环。

use std::fs;

use minicodex::{Agent, FnToolExecutor, Message, ModelTurn, ScriptedClient, ToolCall};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 剧本：模拟模型第 1 步猜错了大小写路径，看到工具报错后在第 2 步自纠，第 3 步总结答案。
    let client = ScriptedClient::new([
        Ok(ModelTurn::with_tools(
            Some("我先读一下 cargo.toml 看看项目名称。".into()),
            vec![ToolCall {
                call_id: "call_1".into(),
                name: "read_file".into(),
                arguments: json!({ "path": "cargo.toml" }),
            }],
        )),
        Ok(ModelTurn::with_tools(
            Some("路径不对，Linux 区分大小写，改读 Cargo.toml。".into()),
            vec![ToolCall {
                call_id: "call_2".into(),
                name: "read_file".into(),
                arguments: json!({ "path": "Cargo.toml" }),
            }],
        )),
        Ok(ModelTurn::text(
            "已确认：当前仓库 Cargo.toml 中的 package 名称是 `minicodex`。",
        )),
    ]);

    let tools = FnToolExecutor::new(|call| match call.name.as_str() {
        "read_file" => {
            let path = call
                .arguments
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing string field `path`".to_string())?;
            fs::read_to_string(path).map_err(|err| format!("failed to read `{path}`: {err}"))
        }
        other => Err(format!("unknown tool `{other}`")),
    });

    let mut agent = Agent::new(
        client,
        tools,
        Some("你是一个严谨的 Rust 编程助手，优先使用工具核实事实。".into()),
        5,
    );

    let outcome = agent.run_turn("帮我查一下这个项目的 Cargo.toml 里的包名是什么")?;

    println!("=== MiniCodex S1 运行轨迹 ===");
    for (idx, msg) in agent.history().iter().enumerate() {
        print_message(idx, msg);
    }
    println!("\n=== 本轮汇总 ===");
    println!("steps: {}", outcome.steps);
    println!("tool_calls_executed: {}", outcome.tool_calls_executed);
    println!("final_text: {}", outcome.final_text);

    Ok(())
}

fn print_message(idx: usize, msg: &Message) {
    match msg {
        Message::System(text) => println!("[{idx}] SYSTEM: {text}"),
        Message::User(text) => println!("[{idx}] USER: {text}"),
        Message::Assistant { text, tool_calls } => {
            println!("[{idx}] ASSISTANT:");
            if let Some(t) = text {
                println!("      text: {t}");
            }
            for call in tool_calls {
                println!(
                    "      call [{}] {}({})",
                    call.call_id, call.name, call.arguments
                );
            }
        }
        Message::ToolResult {
            call_id,
            tool_name,
            output,
            is_error,
        } => {
            let tag = if *is_error { "ERROR" } else { "OK" };
            let preview = output.lines().take(3).collect::<Vec<_>>().join(" | ");
            println!("[{idx}] TOOL_RESULT [{call_id}] {tool_name} ({tag}): {preview}");
        }
    }
}
