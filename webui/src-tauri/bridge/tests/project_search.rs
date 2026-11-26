//! 第2画面の上限付き全体検索と複数ファイル WorkspaceEdit。
mod common;

use common::{ok, parse, sample, text, TempDir};
use novnow_native_bridge::NativeSession;
use serde_json::json;

const NEEDLE: &str = "novnow-webui-workspace-search-needle";

#[test]
fn project_search_reports_hits_and_applies_one_multifile_workspace_edit() {
    let dir = TempDir::new("project-search");
    let root = sample(&dir, "sample");
    let manuscript = root.join("contents/01-04.cnov");
    let ideas = root.join("ideas.md");
    for path in [&manuscript, &ideas] {
        let mut contents = std::fs::read_to_string(path).expect("read fixture");
        contents.push_str(&format!("\n{NEEDLE}\n"));
        std::fs::write(path, contents).expect("edit fixture");
    }

    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));
    let search = ok(&session.run(
        "ui.search.project",
        &json!({
            "query": NEEDLE,
            "regex": false,
            "case_sensitive": true
        })
        .to_string(),
    ));
    assert_eq!(search["count"], json!(2));
    assert_eq!(search["timed_out"], json!(false));
    assert_eq!(search["hits"].as_array().expect("hits").len(), 2);

    let replaced = ok(&session.run(
        "ui.search.project",
        &json!({
            "query": NEEDLE,
            "regex": false,
            "case_sensitive": true,
            "replacement": "workspace-edit-applied"
        })
        .to_string(),
    ));
    assert_eq!(replaced["replaced"], json!(2));
    assert_eq!(replaced["paths"].as_array().expect("paths").len(), 2);
    ok(&session.run("save.now", "{}"));
    for path in [&manuscript, &ideas] {
        let contents = std::fs::read_to_string(path).expect("read after replace");
        assert!(!contents.contains(NEEDLE));
        assert!(contents.contains("workspace-edit-applied"));
    }
}
