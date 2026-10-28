//! パス履歴（§13.2・§13.3.4）と時刻の書式。
//!
//! - 場所: §13.2.1 の表（OS ごと・`NOVNOW_RECENT_FILE`・`XDG_STATE_HOME`）。
//! - 形: キーは `path` / `opened_at` だけ・32 件・同じパスは先頭へ・壊れていたら空（§13.2.2）。
//! - 書き込み: 一時ファイルを使わず直接上書き（§13.2.2 第 2.9 版）。

mod sandbox;

use std::path::PathBuf;
use std::sync::Arc;

use novnow::adapters::clock_system::{civil_date, rfc3339, yyyymmdd};
use novnow::adapters::recent_file::{location, RecentFile};
use novnow::adapters::storage_fs::FsGate;
use novnow::core::policy::recent::{
    PathItem, ProjectItem, RecentPaths, RecentProjects, RECENT_MAX,
};
use novnow::ports::env::Os;

use sandbox::{FakeEnv, FixedClock, TempDir, NOW};

// ============================================================ 場所（§13.2.1）

#[test]
fn the_location_follows_the_table_of_13_2_1() {
    let home = PathBuf::from("/home/akari");
    let linux = FakeEnv::new(Os::Linux).home(&home);
    assert_eq!(
        location(&linux),
        Some(home.join(".local/state/novnow/recent.json"))
    );
    let xdg = linux.clone().var("XDG_STATE_HOME", "/var/state");
    assert_eq!(
        location(&xdg),
        Some(PathBuf::from("/var/state/novnow/recent.json"))
    );
    // XDG の値が相対パスなら無効（XDG Base Directory の規則）
    let relative = linux.clone().var("XDG_STATE_HOME", "state");
    assert_eq!(
        location(&relative),
        Some(home.join(".local/state/novnow/recent.json"))
    );
    let bsd = FakeEnv::new(Os::OtherUnix).home(&home);
    assert_eq!(location(&bsd), location(&linux));
    let mac = FakeEnv::new(Os::MacOs).home(&PathBuf::from("/Users/akari"));
    assert_eq!(
        location(&mac),
        Some(PathBuf::from(
            "/Users/akari/Library/Application Support/novnow/recent.json"
        ))
    );
    let windows = FakeEnv::new(Os::Windows).var("APPDATA", r"C:\Users\akari\AppData\Roaming");
    assert_eq!(
        location(&windows),
        Some(
            PathBuf::from(r"C:\Users\akari\AppData\Roaming")
                .join("novnow")
                .join("recent.json")
        )
    );
}

#[test]
fn the_override_variable_wins_and_missing_homes_give_no_history() {
    let custom = FakeEnv::new(Os::MacOs).var("NOVNOW_RECENT_FILE", "/portable/recent.json");
    assert_eq!(
        location(&custom),
        Some(PathBuf::from("/portable/recent.json"))
    );
    assert_eq!(location(&FakeEnv::new(Os::Linux)), None);
    assert_eq!(location(&FakeEnv::new(Os::Windows)), None);
    assert_eq!(location(&FakeEnv::new(Os::Web)), None);
}

// ============================================================ 形（§13.2.2）

fn item(path: &str) -> PathItem {
    PathItem {
        path: path.to_owned(),
        opened_at: "2026-10-13T00:00:00Z".to_owned(),
    }
}

#[test]
fn pushing_moves_an_existing_path_to_the_front() {
    let mut recent = RecentPaths::default();
    recent.push(item("/a"));
    recent.push(item("/b"));
    recent.push(item("/a"));
    assert_eq!(recent.keys(), vec!["/a", "/b"]);
    assert!(recent.remove("/b"));
    assert!(!recent.remove("/b"));
    assert_eq!(recent.keys(), vec!["/a"]);
}

#[test]
fn at_most_32_items_are_kept() {
    let mut recent = RecentPaths::default();
    for n in 0..40 {
        recent.push(item(&format!("/novel/{n}")));
    }
    assert_eq!(recent.items.len(), RECENT_MAX);
    assert_eq!(recent.keys()[0], "/novel/39");
    assert_eq!(recent.keys()[31], "/novel/8");
}

#[test]
fn a_broken_or_unknown_history_reads_as_empty() {
    for text in [
        "",
        "{",
        "[]",
        "{\"schema_version\":2,\"items\":[]}",
        "{\"schema_version\":1,\"items\":[{\"path\":1}]}",
    ] {
        assert_eq!(RecentPaths::parse(text), RecentPaths::default(), "{text:?}");
    }
}

#[test]
fn only_the_two_keys_are_written() {
    // 手で足されたキー（タイトル・最後の節）は読むときに落ち、書き戻すときに消える（§13.2.2 MUST）
    let text = r#"{"schema_version":1,"theme":"dark","items":[
        {"path":"/a","opened_at":"2026-10-09T12:00:00+09:00","title":"星屑","last_scene":"01-04"}
    ]}"#;
    let recent = RecentPaths::parse(text);
    let value: serde_json::Value = serde_json::from_str(&recent.to_json()).expect("json");
    assert_eq!(
        value,
        serde_json::json!({
            "schema_version": 1,
            "items": [{ "path": "/a", "opened_at": "2026-10-09T12:00:00+09:00" }]
        })
    );
}

#[test]
fn duplicates_in_a_hand_edited_file_keep_the_newest() {
    let text = r#"{"schema_version":1,"items":[
        {"path":"/a","opened_at":"3"},{"path":"/b","opened_at":"2"},{"path":"/a","opened_at":"1"}
    ]}"#;
    let recent = RecentPaths::parse(text);
    assert_eq!(recent.keys(), vec!["/a", "/b"]);
    assert_eq!(recent.items[0].opened_at, "3");
}

#[test]
fn the_web_history_uses_the_same_rules_with_ids() {
    // §13.3.4 の `recent_projects`
    let mut recent = RecentProjects::default();
    for id in ["sample", "b3c1", "sample"] {
        recent.push(ProjectItem {
            id: id.to_owned(),
            opened_at: "2026-10-13T00:00:00Z".to_owned(),
        });
    }
    assert_eq!(recent.keys(), vec!["sample", "b3c1"]);
    let value: serde_json::Value = serde_json::from_str(&recent.to_json()).expect("json");
    assert_eq!(
        value["items"][0],
        serde_json::json!({"id": "sample", "opened_at": "2026-10-13T00:00:00Z"})
    );
    assert_eq!(RecentProjects::from_value(Some(value)), recent);
    assert_eq!(RecentProjects::from_value(None), RecentProjects::default());
}

// ============================================================ 履歴ファイル

#[test]
fn the_history_file_is_overwritten_in_place() {
    let dir = TempDir::new("recent-file");
    let history = dir.join("state/novnow/recent.json");
    let gate = Arc::new(FsGate::new(&FakeEnv::with_history(&history)));
    let recent = RecentFile::new(gate);
    assert_eq!(recent.path(), Some(history.clone()));
    assert_eq!(recent.load(), RecentPaths::default());
    let first = recent
        .push(&PathBuf::from("/novels/a"), &NOW)
        .expect("push");
    assert_eq!(first.items[0].opened_at, "2026-10-13T03:04:05Z");
    recent
        .push(
            &PathBuf::from("/novels/b"),
            &FixedClock {
                millis: NOW.millis + 1_000,
                offset: 540,
            },
        )
        .expect("push");
    let loaded = recent.load();
    assert_eq!(loaded.keys(), vec!["/novels/b", "/novels/a"]);
    assert_eq!(loaded.items[0].opened_at, "2026-10-13T12:04:06+09:00");
    // 壊れていたら空として扱い、上書きする（§13.2.2）
    std::fs::write(&history, "{ broken").expect("write");
    assert_eq!(recent.load(), RecentPaths::default());
    recent
        .push(&PathBuf::from("/novels/c"), &NOW)
        .expect("push");
    assert_eq!(recent.load().keys(), vec!["/novels/c"]);
    // 一時ファイルを作らない
    let names: Vec<_> = std::fs::read_dir(history.parent().expect("parent"))
        .expect("read_dir")
        .flatten()
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(names.len(), 1);
    recent.remove("/novels/c").expect("remove");
    assert_eq!(recent.load().keys(), Vec::<&str>::new());
}

#[test]
fn without_a_location_nothing_is_written() {
    let gate = Arc::new(FsGate::new(&FakeEnv::new(Os::Linux)));
    let recent = RecentFile::new(gate);
    assert_eq!(recent.path(), None);
    let pushed = recent
        .push(&PathBuf::from("/novels/a"), &NOW)
        .expect("push");
    assert_eq!(pushed.keys(), vec!["/novels/a"]);
    assert_eq!(recent.load(), RecentPaths::default());
}

// ============================================================ 時刻の書式

#[test]
fn rfc3339_and_dates() {
    assert_eq!(rfc3339(NOW.millis, 0), "2026-10-13T03:04:05Z");
    assert_eq!(rfc3339(NOW.millis, 540), "2026-10-13T12:04:05+09:00");
    assert_eq!(rfc3339(NOW.millis, -330), "2026-10-12T21:34:05-05:30");
    assert_eq!(rfc3339(0, 0), "1970-01-01T00:00:00Z");
    assert_eq!(rfc3339(-1_000, 0), "1969-12-31T23:59:59Z");
    assert_eq!(rfc3339(951_825_600_000, 0), "2000-02-29T12:00:00Z");
    // 2026-12-31T15:30Z は東京では翌年の元日
    assert_eq!(civil_date(1_798_731_000_000, 540), (2027, 1, 1));
    assert_eq!(yyyymmdd(1_798_731_000_000, 0), "20261231");
    assert_eq!(yyyymmdd(1_798_731_000_000, 540), "20270101");
}

#[test]
fn removing_a_history_row_keeps_the_project_directory_untouched() {
    let dir = TempDir::new("recent-delete-keeps-project");
    let project = dir.join("novel");
    std::fs::create_dir_all(&project).expect("project dir");
    let manuscript = project.join("project.toml");
    std::fs::write(&manuscript, "original manuscript").expect("project data");
    let history = dir.join("state/novnow/recent.json");
    let gate = Arc::new(FsGate::new(&FakeEnv::with_history(&history)));
    let recent = RecentFile::new(gate);
    recent.push(&project, &NOW).expect("store row");
    assert_eq!(recent.load().items.len(), 1);
    recent
        .remove(&project.to_string_lossy())
        .expect("remove row");
    assert!(recent.load().items.is_empty());
    assert_eq!(
        std::fs::read_to_string(manuscript).expect("project exists"),
        "original manuscript"
    );
}
