use std::fs;
use std::path::Path;

use minicodex::patch::{
    ApplyPatchTool, FilePatch, Patch, PatchError, PatchSummary, apply_patch_atomic, parse_patch,
};
use minicodex::tools::Tool;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn add_file_through_tool_preserves_exact_text() {
    let root = TempDir::new().unwrap();
    let tool = ApplyPatchTool::new(root.path());
    assert_eq!(tool.spec().name, "apply_patch");
    let text = "ADD notes.txt\nCONTENT \"第一行\\n最后一行\"";
    assert!(tool.call(json!({ "patch": text })).is_ok());
    assert_eq!(
        fs::read_to_string(root.path().join("notes.txt")).unwrap(),
        "第一行\n最后一行"
    );
}

#[test]
fn update_applies_hunks_in_order_to_one_file() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("notes.txt");
    fs::write(&path, "alpha\nbeta\n").unwrap();
    let patch = parse_patch(
        "UPDATE notes.txt\nSEARCH \"alpha\\nbeta\"\nREPLACE \"gamma\"\nSEARCH \"gamma\"\nREPLACE \"delta\"",
    )
    .unwrap();
    assert_eq!(apply_patch_atomic(root.path(), &patch).unwrap().updated, 1);
    assert_eq!(fs::read_to_string(path).unwrap(), "delta\n");
}

#[test]
fn delete_reports_a_missing_file_on_second_attempt() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("obsolete");
    fs::write(&path, "old").unwrap();
    let patch = parse_patch("DELETE obsolete").unwrap();
    assert_eq!(apply_patch_atomic(root.path(), &patch).unwrap().deleted, 1);
    assert!(!path.exists());
    assert!(matches!(
        apply_patch_atomic(root.path(), &patch),
        Err(PatchError::MissingFile(_))
    ));
}

#[test]
fn a_batch_commits_add_update_and_delete_together() {
    let root = TempDir::new().unwrap();
    let dir = root.path();
    fs::write(dir.join("version"), "1").unwrap();
    fs::write(dir.join("old"), "Remove me").unwrap();
    let patch = parse_patch(
        "ADD new\nCONTENT \"ok\\n\"\nUPDATE version\nSEARCH \"1\"\nREPLACE \"2\"\nDELETE old",
    )
    .unwrap();
    assert_eq!(
        apply_patch_atomic(dir, &patch).unwrap(),
        PatchSummary {
            added: 1,
            updated: 1,
            deleted: 1
        }
    );
    assert_eq!(fs::read_to_string(dir.join("new")).unwrap(), "ok\n");
    assert_eq!(fs::read_to_string(dir.join("version")).unwrap(), "2");
    assert!(!dir.join("old").exists());
}

#[test]
fn later_missing_context_preserves_the_first_file_byte_for_byte() {
    let root = TempDir::new().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::write(&first, b"old\r\n").unwrap();
    fs::write(&second, b"unchanged").unwrap();
    let patch = parse_patch(
        "UPDATE first\nSEARCH \"old\"\nREPLACE \"new\"\nUPDATE second\nSEARCH \"absent\"\nREPLACE \"x\"",
    )
    .unwrap();
    assert!(matches!(
        apply_patch_atomic(root.path(), &patch),
        Err(PatchError::ContextNotFound(path)) if path.ends_with("second")
    ));
    assert_eq!(fs::read(first).unwrap(), b"old\r\n");
    assert_eq!(fs::read(second).unwrap(), b"unchanged");
}

#[test]
fn ambiguous_search_preserves_file() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("data");
    fs::write(&path, "相同\n相同\n").unwrap();
    let patch = parse_patch("UPDATE data\nSEARCH \"相同\"\nREPLACE \"新\"").unwrap();
    assert!(matches!(
        apply_patch_atomic(root.path(), &patch),
        Err(PatchError::AmbiguousContext(_))
    ));
    assert_eq!(fs::read_to_string(path).unwrap(), "相同\n相同\n");
}

#[test]
fn path_traversal_is_rejected_even_for_hand_built_patches() {
    let sandbox = TempDir::new().unwrap();
    let root = sandbox.path().join("root");
    fs::create_dir(&root).unwrap();
    let patch = Patch {
        files: vec![FilePatch::AddFile {
            path: Path::new("../escape").into(),
            content: "outside".into(),
        }],
    };
    assert!(matches!(
        apply_patch_atomic(&root, &patch),
        Err(PatchError::PathOutsideRoot(_))
    ));
    assert!(!sandbox.path().join("escape").exists());
}

#[cfg(unix)]
#[test]
fn symlink_parent_cannot_redirect_an_add_outside_root() {
    let sandbox = TempDir::new().unwrap();
    let root = sandbox.path().join("repo");
    let outside = sandbox.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
    let patch = parse_patch("ADD link/file\nCONTENT \"data\"").unwrap();
    assert!(matches!(
        apply_patch_atomic(&root, &patch),
        Err(PatchError::PathOutsideRoot(_))
    ));
    assert!(!outside.join("file").exists());
}

#[test]
fn malformed_replacement_reports_its_line_and_tool_preserves_the_file() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("data");
    fs::write(&path, "old").unwrap();
    let input = "UPDATE data\nSEARCH \"old\"\nREPLACE missing-json-quotes";
    assert!(matches!(
        parse_patch(input),
        Err(PatchError::Syntax { line: 3, .. })
    ));
    assert!(
        ApplyPatchTool::new(root.path())
            .call(json!({ "patch": input }))
            .is_err()
    );
    assert_eq!(fs::read_to_string(path).unwrap(), "old");
}
