//! IndexedDB（§13.3）の純粋な部分（`adapters::storage_idb` の上半分）。
//!
//! ブラウザとの接続（`wasm32` だけの `web` モジュール）はここでは動かせない（CI の WASM ジョブが
//! コンパイルを確かめ、実行の確認は M7 の Web UI で行う。`PLANS.md` §9.2 B-96）。ここでは
//! レコードの形・`app` ストアの許可キー（§13.3.4 MUST「テストで検証」）・残容量・v0 の取り込みの
//! 計画・ID の採番・時刻の書式を確かめる。

use novnow::adapters::clock_system::{parse_utc, rfc3339, rfc3339_millis};
use novnow::adapters::storage_idb::{
    is_low_space, low_space_notice, unavailable_notice, uuid_v4, AppRecord, FileRecord, Origin,
    ProjectRecord, APP, APP_KEYS, BY_PROJECT, BY_UPDATED, DB_NAME, DB_VERSION, FILES,
    LOW_SPACE_BYTES, PROJECTS, RECENT_PROJECTS_KEY, SAMPLE_ID,
};
use novnow::core::diagnostics::DiagnosticCode;
use novnow::core::policy::recent::{ProjectItem, RecentProjects};
use novnow::core::policy::ProjectPath;

const SPEC: &str = include_str!("../docs/specifications/platform.md");

/// §13.3 の本文（§13.3 から §13.4 の手前まで）。
fn section_13_3() -> &'static str {
    let start = SPEC.find("### 13.3 Web: IndexedDB").expect("§13.3");
    let end = SPEC[start..].find("### 13.4").expect("§13.4") + start;
    &SPEC[start..end]
}

fn path(text: &str) -> ProjectPath {
    ProjectPath::new(text).expect("相対パス")
}

#[test]
fn the_names_match_the_schema_table() {
    let spec = section_13_3();
    assert!(spec.contains(&format!("DB 名 `{DB_NAME}` / バージョン `{DB_VERSION}`")));
    for name in [
        PROJECTS,
        FILES,
        APP,
        BY_UPDATED,
        BY_PROJECT,
        RECENT_PROJECTS_KEY,
        SAMPLE_ID,
    ] {
        assert!(spec.contains(&format!("`{name}`")), "{name}");
    }
}

#[test]
fn the_app_store_allows_only_recent_projects() {
    // §13.3.4「`app` ストアに他のキーを書くことは禁止（MUST、テストで検証）」
    assert_eq!(APP_KEYS, &["recent_projects"]);
    let mut recent = RecentProjects::default();
    recent.push(ProjectItem {
        id: "sample".to_owned(),
        opened_at: "2026-10-13T03:04:05.000Z".to_owned(),
    });
    let record = AppRecord::recent(&recent);
    assert_eq!(record.key, RECENT_PROJECTS_KEY);
    let value = serde_json::to_value(&record).expect("json");
    assert_eq!(
        value,
        serde_json::json!({
            "key": "recent_projects",
            "value": {
                "schema_version": 1,
                "items": [{ "id": "sample", "opened_at": "2026-10-13T03:04:05.000Z" }]
            }
        })
    );
    assert_eq!(RecentProjects::from_value(Some(record.value)), recent);
}

#[test]
fn records_have_exactly_the_fields_of_the_schema_table() {
    let project = ProjectRecord::new(
        "sample",
        "sample",
        Origin::Sample,
        "2026-10-13T03:04:05.000Z",
    );
    let value = serde_json::to_value(&project).expect("json");
    let keys: Vec<&String> = value.as_object().expect("object").keys().collect();
    assert_eq!(
        keys,
        vec![
            "created_at",
            "id",
            "name",
            "origin",
            "schema_version",
            "updated_at"
        ]
    );
    assert_eq!(value["origin"], "sample");
    let file = FileRecord::new(
        "sample",
        &path("contents/01-04.txt"),
        "本文",
        "2026-10-13T03:04:05.000Z",
    );
    let value = serde_json::to_value(&file).expect("json");
    let keys: Vec<&String> = value.as_object().expect("object").keys().collect();
    assert_eq!(keys, vec!["path", "project_id", "text", "updated_at"]);
    assert_eq!(file.project_path(), Some(path("contents/01-04.txt")));
    // 先頭スラッシュ（§13.3.2 で禁止）の手で壊されたレコードは読み飛ばせる
    let broken = FileRecord {
        path: "/etc/passwd".to_owned(),
        ..file.clone()
    };
    assert_eq!(broken.project_path(), None);
}

#[test]
fn origins_are_the_three_of_the_schema() {
    let origins: Vec<&str> = [Origin::Sample, Origin::New, Origin::Import]
        .into_iter()
        .map(Origin::as_str)
        .collect();
    assert_eq!(origins, vec!["sample", "new", "import"]);
    assert!(section_13_3().contains(r#"`"sample" | "new" | "import"`"#));
}

#[test]
fn file_stamps_come_from_the_update_time_and_length() {
    let file = FileRecord::new("p", &path("a.md"), "あいう", "2026-10-13T03:04:05.123Z");
    let stamp = file.stamp();
    assert_eq!(stamp.len, 9);
    assert_eq!(stamp.modified_ms, Some(1_791_860_645_123));
}

#[test]
fn low_space_is_under_ten_megabytes() {
    assert_eq!(LOW_SPACE_BYTES, 10_485_760.0);
    assert!(is_low_space(100.0e6, 95.0e6));
    assert!(!is_low_space(100.0e6, 80.0e6));
    assert_eq!(low_space_notice().code, DiagnosticCode::W062);
    assert_eq!(unavailable_notice().code, DiagnosticCode::E062);
}

#[test]
fn uuids_are_version_4() {
    let id = uuid_v4([0xff; 16]);
    assert_eq!(id, "ffffffff-ffff-4fff-bfff-ffffffffffff");
    let id = uuid_v4([0; 16]);
    assert_eq!(id, "00000000-0000-4000-8000-000000000000");
}

// ============================================================ 時刻（IndexedDB のレコードの日時）

#[test]
fn utc_millis_round_trip_and_sort_as_text() {
    let stamps = [0_i64, 999, 1_791_860_645_123, 951_825_600_000, -1_000];
    for millis in stamps {
        let text = rfc3339_millis(millis);
        assert_eq!(parse_utc(&text), Some(millis), "{text}");
    }
    assert_eq!(
        rfc3339_millis(1_791_860_645_123),
        "2026-10-13T03:04:05.123Z"
    );
    // 秒までの `Z` も読める（`rfc3339` のずれ 0）
    assert_eq!(
        parse_utc(&rfc3339(1_791_860_645_000, 0)),
        Some(1_791_860_645_000)
    );
    // ずれ付き・壊れた形は読まない
    for bad in [
        "2026-10-13T12:04:05+09:00",
        "2026-13-01T00:00:00Z",
        "2026-10-13T03:04:05.1Z",
        "x",
    ] {
        assert_eq!(parse_utc(bad), None, "{bad}");
    }
    // 固定幅なので文字列の順が時刻の順（`by_updated` の索引）
    let mut texts: Vec<String> = [1_791_860_645_123, 951_825_600_000, 1_791_860_645_124]
        .into_iter()
        .map(rfc3339_millis)
        .collect();
    texts.sort();
    assert_eq!(
        texts
            .iter()
            .map(|text| parse_utc(text).expect("parse"))
            .collect::<Vec<_>>(),
        vec![951_825_600_000, 1_791_860_645_123, 1_791_860_645_124]
    );
}

#[test]
fn a_web_project_title_must_be_unique_except_for_its_own_rename() {
    use novnow::adapters::storage_idb::{name_available, Origin, ProjectRecord};
    let projects = [ProjectRecord::new("a", "星屑", Origin::New, "now")];
    assert!(!name_available(&projects, "星屑", None));
    assert!(!name_available(&projects, " 星屑 ", Some("b")));
    assert!(name_available(&projects, "星屑", Some("a")));
    assert!(name_available(&projects, "新作", None));
    assert!(!name_available(&projects, " ", None));
}
