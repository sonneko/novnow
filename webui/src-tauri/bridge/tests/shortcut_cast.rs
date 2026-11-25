//! §10.0 固定ショートカットは metadata の mode: 先頭キャラへ着地する。
mod common;

use common::{ok, sample, text, TempDir};
use novnow_native_bridge::NativeSession;
use serde_json::json;

#[test]
fn resolves_the_first_character_from_the_scene_mode_field() {
    let dir = TempDir::new("shortcut-cast");
    let root = sample(&dir, "sample");
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));

    let found = ok(&session.run(
        "ui.shortcut.cast",
        &json!({
            "scene": "01-04-01-01"
        })
        .to_string(),
    ));
    assert_eq!(found["index"], json!(0));
}
