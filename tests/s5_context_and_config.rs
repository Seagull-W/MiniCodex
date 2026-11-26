use std::fs;

use minicodex::context::{
    AgentsDoc, AppConfig, ApprovalMode, ContextError, build_system_prompt, discover_agents_md,
    load_layered_config, merge_toml,
};
use tempfile::TempDir;

#[test]
fn deep_merge_keeps_siblings_and_replaces_arrays_and_type_conflicts() {
    let base = toml::from_str(
        "[shell]\ntimeout = 7\nargs = ['-c']\n[shell.env]\nkeep = 'yes'\nedit = 'old'\n[log]\nlevel = 'info'",
    )
    .unwrap();
    let overlay =
        toml::from_str("log = false\n[shell]\nargs = ['-l']\n[shell.env]\nedit = 'new'").unwrap();
    let expected: toml::Value = toml::from_str(
        "log = false\n[shell]\ntimeout = 7\nargs = ['-l']\n[shell.env]\nkeep = 'yes'\nedit = 'new'",
    )
    .unwrap();
    assert_eq!(merge_toml(base, overlay), expected);
}

#[test]
fn four_layers_only_override_keys_they_supply() {
    let dir = TempDir::new().unwrap();
    let user = dir.path().join("user.toml");
    let project = dir.path().join("project.toml");
    fs::write(
        &user,
        "model = 'user'\nmax_steps = 3\nshell_timeout_secs = 19",
    )
    .unwrap();
    fs::write(&project, "model = 'project'\nmax_steps = 5").unwrap();
    let cli = toml::from_str("model = 'cli'").unwrap();
    let result = load_layered_config(Some(&user), Some(&project), cli).unwrap();
    assert_eq!(
        result,
        AppConfig {
            model: "cli".into(),
            max_steps: 5,
            shell_timeout_secs: 19,
            ..AppConfig::default()
        }
    );
}

#[test]
fn missing_optional_layers_use_defaults_and_keep_cli_values() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("missing.toml");
    let cli = toml::from_str("approval_mode = 'never'").unwrap();
    assert_eq!(
        load_layered_config(Some(&missing), None, cli).unwrap(),
        AppConfig {
            approval_mode: ApprovalMode::Never,
            ..AppConfig::default()
        }
    );
}

#[test]
fn syntax_error_in_existing_file_reports_its_path() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("project.toml");
    fs::write(&project, "model = [").unwrap();
    let err = load_layered_config(None, Some(&project), toml::Table::new()).unwrap_err();
    assert!(matches!(err, ContextError::Config { path: Some(path), .. } if path == project));
}

#[test]
fn nearest_git_root_limits_discovery_and_docs_follow_directory_order() {
    let outer = TempDir::new().unwrap();
    let repo = outer.path().join("repo");
    let sub = repo.join("src");
    let cwd = sub.join("nested");
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir(outer.path().join(".git")).unwrap();
    fs::write(repo.join(".git"), "gitdir: /worktrees/example\n").unwrap();
    fs::write(outer.path().join("AGENTS.md"), "outer").unwrap();
    fs::write(repo.join("AGENTS.md"), "root").unwrap();
    fs::write(sub.join("AGENTS.md"), "sub").unwrap();
    fs::write(cwd.join("AGENTS.md"), "leaf").unwrap();
    assert_eq!(
        discover_agents_md(&cwd, 100).unwrap(),
        vec![
            AgentsDoc {
                path: repo.join("AGENTS.md"),
                content: "root".into()
            },
            AgentsDoc {
                path: sub.join("AGENTS.md"),
                content: "sub".into()
            },
            AgentsDoc {
                path: cwd.join("AGENTS.md"),
                content: "leaf".into()
            },
        ]
    );
}

#[test]
fn override_replaces_only_the_document_in_the_same_directory() {
    let repo = TempDir::new().unwrap();
    let sub = repo.path().join("src");
    fs::create_dir(repo.path().join(".git")).unwrap();
    fs::create_dir(&sub).unwrap();
    fs::write(repo.path().join("AGENTS.md"), "ancestor").unwrap();
    fs::write(sub.join("AGENTS.md"), "regular").unwrap();
    fs::write(sub.join("AGENTS.override.md"), "local").unwrap();
    assert_eq!(
        discover_agents_md(&sub, 100).unwrap(),
        vec![
            AgentsDoc {
                path: repo.path().join("AGENTS.md"),
                content: "ancestor".into()
            },
            AgentsDoc {
                path: sub.join("AGENTS.override.md"),
                content: "local".into()
            },
        ]
    );
}

#[test]
fn all_docs_share_one_budget_and_truncation_stops_on_a_utf8_boundary() {
    let repo = TempDir::new().unwrap();
    let sub = repo.path().join("src");
    fs::create_dir(repo.path().join(".git")).unwrap();
    fs::create_dir(&sub).unwrap();
    fs::write(repo.path().join("AGENTS.md"), "abc").unwrap();
    fs::write(sub.join("AGENTS.md"), "中文!").unwrap();
    assert_eq!(
        discover_agents_md(&sub, 7).unwrap(),
        vec![
            AgentsDoc {
                path: repo.path().join("AGENTS.md"),
                content: "abc".into()
            },
            AgentsDoc {
                path: sub.join("AGENTS.md"),
                content: "中".into()
            },
        ]
    );
    assert!(discover_agents_md(&sub, 0).unwrap().is_empty());
}

#[test]
fn system_prompt_identifies_working_directory_and_keeps_document_order() {
    let repo = TempDir::new().unwrap();
    let cwd = repo.path().join("src");
    let docs = [
        AgentsDoc {
            path: repo.path().join("AGENTS.md"),
            content: "root rule".into(),
        },
        AgentsDoc {
            path: cwd.join("AGENTS.override.md"),
            content: "local rule".into(),
        },
    ];
    let prompt = build_system_prompt("base instruction", &docs, &cwd);
    let mut suffix = prompt.as_str();
    for part in [
        "base instruction",
        cwd.to_str().unwrap(),
        docs[0].path.to_str().unwrap(),
        "root rule",
        docs[1].path.to_str().unwrap(),
        "local rule",
    ] {
        suffix = suffix
            .split_once(part)
            .unwrap_or_else(|| panic!("缺少或乱序：{part}"))
            .1;
    }
}
