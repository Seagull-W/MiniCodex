use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use minicodex::policy::{
    Decision, ExecPolicy, PolicyError, PolicyGuard, PrefixRule, split_command,
};
use minicodex::tools::{ShellTool, Tool};
use serde_json::json;
use tempfile::TempDir;

fn words(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

fn rule(prefix: &[&str], decision: Decision) -> PrefixRule {
    PrefixRule {
        prefix: words(prefix),
        decision,
    }
}

fn shell(root: &TempDir) -> ShellTool {
    ShellTool::new(root.path(), Duration::from_secs(2), 256)
}

#[test]
fn splits_plain_command_on_whitespace() {
    assert_eq!(
        split_command("  git\tstatus  --short "),
        Ok(words(&["git", "status", "--short"]))
    );
}

#[test]
fn keeps_quoted_spaces_and_empty_arguments() {
    assert_eq!(
        split_command("printf \"two words\" '一 二' ''"),
        Ok(words(&["printf", "two words", "一 二", ""]))
    );
}

#[test]
fn returns_typed_errors_for_unclosed_quotes_or_empty_command() {
    assert_eq!(
        split_command("echo 'hello"),
        Err(PolicyError::UnclosedQuote('\''))
    );
    assert_eq!(
        split_command("echo \"hello"),
        Err(PolicyError::UnclosedQuote('"'))
    );
    assert_eq!(split_command("   "), Err(PolicyError::EmptyCommand));
}

#[test]
fn an_allow_prefix_matches_only_whole_arguments() {
    assert_eq!(
        ExecPolicy::default().evaluate(&words(&["git", "status"])),
        Decision::Ask
    );
    let policy = ExecPolicy::new(vec![rule(&["git", "status"], Decision::Allow)]);
    assert_eq!(
        policy.evaluate(&words(&["git", "status", "--short"])),
        Decision::Allow
    );
    assert_eq!(
        policy.evaluate(&words(&["git", "status-extra"])),
        Decision::Ask
    );
    assert_eq!(policy.evaluate(&words(&["git"])), Decision::Ask);
}

#[test]
fn deny_wins_in_both_rule_orders_even_if_allow_is_more_specific() {
    let denial = Decision::Deny {
        reason: "该目录暂停写入".into(),
    };
    let allow = rule(&["touch", "marker"], Decision::Allow);
    let deny = rule(&["touch"], denial.clone());
    let argv = words(&["touch", "marker"]);
    for rules in [vec![allow.clone(), deny.clone()], vec![deny, allow]] {
        assert_eq!(ExecPolicy::new(rules).evaluate(&argv), denial);
    }
}

#[test]
fn allowed_command_runs_directly_and_preserves_quoted_arguments() {
    let root = TempDir::new().unwrap();
    let policy = ExecPolicy::new(vec![rule(&["printf", "%s"], Decision::Allow)]);
    let guard = PolicyGuard::new(shell(&root), policy, |_: &[String]| {
        panic!("Allow 路径意外申请审批")
    });
    let output = guard
        .call(json!({"command": "printf '%s' 'a ; b'"}))
        .unwrap();
    assert_eq!(output, "a ; b");
}

#[test]
fn ask_passes_exact_argv_to_approval_before_running_command() {
    let root = TempDir::new().unwrap();
    let marker = root.path().join("my file");
    let approvals = AtomicUsize::new(0);
    let guard = PolicyGuard::new(shell(&root), ExecPolicy::default(), |argv: &[String]| {
        assert_eq!(argv, words(&["touch", "my file"]));
        assert!(!marker.exists());
        approvals.fetch_add(1, Ordering::Relaxed);
        true
    });
    guard.call(json!({"command": "touch 'my file'"})).unwrap();
    assert_eq!(approvals.load(Ordering::Relaxed), 1);
    assert!(marker.is_file());
}

#[test]
fn rejected_approval_leaves_workdir_untouched() {
    let root = TempDir::new().unwrap();
    let approvals = AtomicUsize::new(0);
    let guard = PolicyGuard::new(shell(&root), ExecPolicy::default(), |_: &[String]| {
        approvals.fetch_add(1, Ordering::Relaxed);
        false
    });
    assert!(guard.call(json!({"command": "touch marker"})).is_err());
    assert_eq!(approvals.load(Ordering::Relaxed), 1);
    assert!(!root.path().join("marker").exists());
}

#[test]
fn denied_command_returns_rule_reason_without_asking_or_writing() {
    let root = TempDir::new().unwrap();
    let policy = ExecPolicy::new(vec![rule(
        &["touch"],
        Decision::Deny {
            reason: "只读目录".into(),
        },
    )]);
    let guard = PolicyGuard::new(shell(&root), policy, |_: &[String]| {
        panic!("Deny 路径意外申请审批")
    });
    let error = guard.call(json!({"command": "touch marker"})).unwrap_err();
    assert!(error.contains("只读目录"), "{error}");
    assert!(!root.path().join("marker").exists());
}
