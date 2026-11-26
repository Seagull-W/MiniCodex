use std::fs;
use std::time::{Duration, Instant};

use minicodex::tools::{ShellTool, Tool, ToolRegistry, ToolSpec, truncate_utf8_output};
use minicodex::{ToolCall, ToolExecutor};
use serde_json::{Value, json};
use tempfile::TempDir;

struct Echo(&'static str);

impl Tool for Echo {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.0.into(),
            description: "回显 text".into(),
            parameters: json!({
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"]
            }),
        }
    }

    fn call(&self, args: Value) -> Result<String, String> {
        args["text"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "缺少 text".to_owned())
    }
}

#[test]
fn dispatches_arguments_and_tool_errors_through_s1_executor() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(Echo("echo")));
    let executor: &mut dyn ToolExecutor = &mut registry;
    let mut call = ToolCall {
        call_id: "c1".into(),
        name: "echo".into(),
        arguments: json!({"text": "hello"}),
    };
    assert_eq!(executor.execute(&call), Ok("hello".into()));
    call.arguments = json!({});
    assert_eq!(executor.execute(&call), Err("缺少 text".into()));
}

#[test]
fn unknown_tool_error_identifies_requested_name() {
    let call = ToolCall {
        call_id: "c2".into(),
        name: "missing_tool".into(),
        arguments: json!({}),
    };
    let error = ToolRegistry::new().execute(&call).unwrap_err();
    assert!(error.contains("missing_tool"), "{error}");
}

#[test]
fn exports_each_tools_full_schema_in_name_order() {
    let shell = ShellTool::new(".", Duration::from_secs(1), 128);
    let shell_spec = shell.spec();
    assert_eq!(shell_spec.name, "shell");
    assert_eq!(shell_spec.parameters["required"], json!(["argv"]));
    assert_eq!(shell_spec.parameters["properties"]["argv"]["type"], "array");

    let mut registry = ToolRegistry::new();
    let echo = Echo("echo");
    let echo_spec = echo.spec();
    registry.register(Box::new(shell));
    registry.register(Box::new(echo));
    assert_eq!(registry.specs(), vec![echo_spec, shell_spec]);
}

#[test]
fn truncates_to_longest_complete_utf8_prefix() {
    for (budget, prefix) in [
        (0, ""),
        (2, ""),
        (3, "你"),
        (4, "你"),
        (6, "你好"),
        (10, "你好!"),
    ] {
        assert_eq!(truncate_utf8_output("你好!", budget), prefix);
    }
}

#[test]
fn captures_stdout_and_stderr_in_configured_workdir() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("fixture"), "from-file").unwrap();
    let shell = ShellTool::new(root.path(), Duration::from_secs(2), 128);
    let output = shell
        .call(json!({"argv": [
            "sh", "-c", "cat fixture && printf 'from-stderr' >&2"
        ]}))
        .unwrap();
    assert!(
        output.contains("from-file") && output.contains("from-stderr"),
        "{output}"
    );
}

#[test]
fn bounds_the_shell_output_at_a_character_boundary() {
    let root = TempDir::new().unwrap();
    let shell = ShellTool::new(root.path(), Duration::from_secs(2), 4);
    assert_eq!(
        shell.call(json!({"argv": ["printf", "你好!"]})),
        Ok("你".into())
    );
}

#[test]
fn nonzero_exit_reports_code_and_captured_diagnostic() {
    let root = TempDir::new().unwrap();
    let shell = ShellTool::new(root.path(), Duration::from_secs(2), 128);
    let error = shell
        .call(json!({"argv": [
            "sh", "-c", "printf 'bad input' >&2; exit 7"
        ]}))
        .unwrap_err();
    assert!(
        error.contains('7') && error.contains("bad input"),
        "{error}"
    );
}

#[test]
fn timeout_terminates_and_reaps_the_started_child() {
    let root = TempDir::new().unwrap();
    let shell = ShellTool::new(root.path(), Duration::from_secs(1), 128);
    // exec 保留 PID，让探针检验的正是 ShellTool 启动的直接子进程。
    let started = Instant::now();
    let error = shell
        .call(json!({"argv": [
            "sh", "-c", "echo $$ > child.pid; exec sleep 8"
        ]}))
        .unwrap_err();
    assert!(
        started.elapsed() < Duration::from_secs(6),
        "超时后仍在等待命令自然退出"
    );
    assert!(error.contains("timeout"), "{error}");
    let pid: u32 = fs::read_to_string(root.path().join("child.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let probe = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .unwrap();
    assert!(!probe.status.success(), "超时返回后 PID {pid} 仍然存在");
}
