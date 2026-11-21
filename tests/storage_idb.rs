//! IndexedDB（§13.3）の純粋な部分（`adapters::storage_idb` の上半分）。
//!
//! ブラウザとの接続（`wasm32` だけの `web` モジュール）はここでは動かせない（CI の WASM ジョブが
//! コンパイルを確かめ、実行の確認は M7 の Web UI で行う。`docs/history.md` B-96）。ここでは
//! レコードの形・`app` ストアの許可キー（§13.3.4 MUST「テストで検証」）・残容量・v0 の取り込みの
//! 計画・ID の採番・時刻の書式を確かめる。

use novnow::adapters::clock_system::{parse_utc, rfc3339, rfc3339_millis};
use novnow::adapters::storage_idb::{
    is_low_space, low_space_notice, restore_plan, snapshot_name, snapshot_room, unavailable_notice,
    uuid_v4, AppRecord, FileRecord, Origin, ProjectRecord, SnapshotFile, SnapshotNameError,
    SnapshotRecord, APP, APP_KEYS, BY_PROJECT, BY_UPDATED, DB_NAME, DB_VERSION, FILES,
    LOW_SPACE_BYTES, MAX_SNAPSHOTS, PROJECTS, RECENT_PROJECTS_KEY, SAMPLE_ID, SNAPSHOTS,
    SNAPSHOT_NAME_FORBIDDEN, SNAPSHOT_NAME_LEN,
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
        SNAPSHOTS,
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
        &path("contents/01-04.cnov"),
        "本文",
        "2026-10-13T03:04:05.000Z",
    );
    let value = serde_json::to_value(&file).expect("json");
    let keys: Vec<&String> = value.as_object().expect("object").keys().collect();
    assert_eq!(keys, vec!["path", "project_id", "text", "updated_at"]);
    assert_eq!(file.project_path(), Some(path("contents/01-04.cnov")));
    // 先頭スラッシュ（§13.3.2 で禁止）の手で壊されたレコードは読み飛ばせる
    let broken = FileRecord {
        bytes: None,
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

// ============================================================ スナップショット（§13.3.8）

/// §13.3.8 の本文（§13.3 の残り）。
fn section_13_3_8() -> &'static str {
    let spec = section_13_3();
    let start = spec.find("#### 13.3.8").expect("§13.3.8");
    &spec[start..]
}

/// §13.3.3 の表（トランザクション境界）。
fn section_13_3_3() -> &'static str {
    let spec = section_13_3();
    let start = spec.find("#### 13.3.3").expect("§13.3.3");
    let end = spec.find("#### 13.3.4").expect("§13.3.4");
    &spec[start..end]
}

fn file(project: &str, path_text: &str, text: &str) -> FileRecord {
    FileRecord::new(project, &path(path_text), text, "2026-11-12T03:04:05.000Z")
}

#[test]
fn the_snapshot_store_joined_the_schema_in_version_two() {
    // §13.3.2: 版は 2、ストアは 4 つ。名前と版は仕様書の本文と一字一句そろえる
    assert_eq!(
        DB_VERSION, 2,
        "snapshots ストアを足したので版は 2（§13.3.2）"
    );
    assert_eq!(SNAPSHOTS, "snapshots");
    let spec = section_13_3();
    assert!(spec.contains(&format!("DB 名 `{DB_NAME}` / バージョン `{DB_VERSION}`")));
    assert!(
        spec.contains("| `snapshots` | `[project_id, name]` | `by_project`（`project_id`） |"),
        "§13.3.2 の表に snapshots の行（keyPath と索引）"
    );
    // 版を上げても利用者の作品は消えない（§13.3.8 MUST）
    assert!(section_13_3_8().contains("既存のレコードはそのまま"));
    // Zip には含めない（§13.5 が扱うのはプロジェクトのファイルだけ）
    assert!(section_13_3_8().contains("Zip の書き出し・取り込みに**含めない**"));
}

#[test]
fn the_transaction_table_covers_saving_and_restoring_snapshots() {
    let table = section_13_3_3();
    assert!(
        table.contains("| スナップショットの保存 |"),
        "§13.3.3 の保存の行"
    );
    assert!(
        table.contains("| スナップショットからの復元 |"),
        "§13.3.3 の復元の行"
    );
    assert!(table.contains("`snapshots`"), "保存の行のストア");
    assert!(
        table.contains("全スナップショット"),
        "プロジェクト削除の行: スナップショットも一緒に消す（§13.3.8）"
    );
    assert!(section_13_3_8().contains("そのスナップショットも一緒に消える"));
}

#[test]
fn snapshot_records_have_exactly_the_fields_of_the_schema() {
    let records = [
        file("p", "project.toml", "schema_version = 2"),
        file("p", "contents/01.cnov", "本文"),
    ];
    let snapshot = SnapshotRecord::new("p", " 第一稿 ", &records, "2026-11-12T03:04:05.000Z");
    assert_eq!(snapshot.name, "第一稿", "名前は前後の空白を落とす");
    assert_eq!(snapshot.file_count(), 2);
    let value = serde_json::to_value(&snapshot).expect("json");
    let keys: Vec<&String> = value.as_object().expect("object").keys().collect();
    assert_eq!(keys, vec!["created_at", "files", "name", "project_id"]);
    let file_value = serde_json::to_value(&snapshot.files[0]).expect("json");
    let keys: Vec<&String> = file_value.as_object().expect("object").keys().collect();
    assert_eq!(
        keys,
        vec!["path", "text"],
        "bytes は無ければ書かない（§13.3.2）"
    );
    // パスの昇順（一覧も復元も決定的）
    let paths: Vec<&str> = snapshot
        .files
        .iter()
        .map(|item| item.path.as_str())
        .collect();
    assert_eq!(paths, vec!["contents/01.cnov", "project.toml"]);
}

#[test]
fn snapshot_size_counts_binary_records_by_their_bytes() {
    let mut binary = file("p", "assets/icon.png", "");
    binary.bytes = Some(vec![7u8; 12]);
    let snapshot = SnapshotRecord::new(
        "p",
        "素材",
        &[file("p", "contents/01.cnov", "あいう"), binary],
        "2026-11-12T03:04:05.000Z",
    );
    assert_eq!(
        snapshot.size(),
        9 + 12,
        "bytes があればその長さ、無ければ text の byte 数（§13.3.2 と同じ数え方）"
    );
}

#[test]
fn snapshot_names_follow_the_rules_of_the_spec() {
    let existing = [SnapshotRecord::new("p", "第一稿", &[], "now")];
    assert_eq!(snapshot_name(" 第二稿 ", &existing).expect("ok"), "第二稿");
    assert_eq!(
        snapshot_name("   ", &existing),
        Err(SnapshotNameError::Empty)
    );
    assert_eq!(snapshot_name("", &existing), Err(SnapshotNameError::Empty));
    assert_eq!(
        snapshot_name(&"あ".repeat(SNAPSHOT_NAME_LEN + 1), &existing),
        Err(SnapshotNameError::TooLong)
    );
    assert!(snapshot_name(&"あ".repeat(SNAPSHOT_NAME_LEN), &existing).is_ok());
    for character in SNAPSHOT_NAME_FORBIDDEN {
        assert_eq!(
            snapshot_name(&format!("第一稿{character}"), &existing),
            Err(SnapshotNameError::Forbidden),
            "{character}"
        );
    }
    assert_eq!(
        snapshot_name("第一稿\u{0}", &existing),
        Err(SnapshotNameError::Forbidden),
        "制御文字も使えない"
    );
    // 同じ名前は 1 プロジェクトに 1 つ（前後の空白を落として比べる。§13.4.1 と同じ規則）
    assert_eq!(
        snapshot_name(" 第一稿 ", &existing),
        Err(SnapshotNameError::Duplicate)
    );
    // 規則そのものは仕様書が正本（§13.3.8）
    let spec = section_13_3_8();
    assert!(spec.contains(&format!("**{SNAPSHOT_NAME_LEN} 文字**")));
    for character in SNAPSHOT_NAME_FORBIDDEN {
        assert!(spec.contains(&format!("`{character}`")), "{character}");
    }
    assert!(spec.contains("制御文字は使えない"));
}

#[test]
fn snapshots_have_a_limit_and_old_ones_are_never_dropped() {
    let full: Vec<SnapshotRecord> = (0..MAX_SNAPSHOTS)
        .map(|index| SnapshotRecord::new("p", &format!("稿{index}"), &[], "now"))
        .collect();
    assert_eq!(MAX_SNAPSHOTS, 8, "§13.3.8 の上限");
    assert!(snapshot_room(&full[..MAX_SNAPSHOTS - 1]));
    assert!(!snapshot_room(&full), "上限に達したら新しく作れない");
    let spec = section_13_3_8();
    assert!(spec.contains(&format!("**{MAX_SNAPSHOTS} 件**")));
    assert!(spec.contains("古いものを黙って捨てない"));
}

#[test]
fn a_restore_plan_replaces_the_files_and_removes_the_rest() {
    let snapshot = SnapshotRecord::new(
        "p",
        "第一稿",
        &[file("p", "a.md", "古い a"), file("p", "b.md", "古い b")],
        "now",
    );
    let current = [
        file("p", "b.md", "書き直した b"),
        file("p", "c.md", "あとから作った c"),
    ];
    let plan = restore_plan(&snapshot, &current);
    let put: Vec<(&str, &str)> = plan
        .put
        .iter()
        .map(|item| (item.path.as_str(), item.text.as_str()))
        .collect();
    assert_eq!(put, vec![("a.md", "古い a"), ("b.md", "古い b")]);
    let remove: Vec<&str> = plan.remove.iter().map(ProjectPath::as_str).collect();
    assert_eq!(
        remove,
        vec!["c.md"],
        "スナップショットに無いファイルは消える"
    );
    // 中身が同じでも書き直す（時刻がそろい、§8.5 の検出が自分の書き込みを候補にしない）
    let same = restore_plan(
        &snapshot,
        &[file("p", "a.md", "古い a"), file("p", "b.md", "古い b")],
    );
    assert_eq!(same.put.len(), 2);
    assert!(same.remove.is_empty());
    // 手で壊されたレコード（先頭スラッシュ）はどちらにも入れない
    let broken = SnapshotRecord {
        files: vec![SnapshotFile {
            path: "/etc/passwd".to_owned(),
            text: String::new(),
            bytes: None,
        }],
        ..snapshot.clone()
    };
    let current = [FileRecord {
        path: "/etc/passwd".to_owned(),
        ..file("p", "a.md", "x")
    }];
    let plan = restore_plan(&broken, &current);
    assert!(plan.put.is_empty());
    assert!(plan.remove.is_empty());
    assert!(section_13_3_8().contains("スナップショットに無いファイルは消える"));
}

#[test]
fn snapshots_are_managed_before_opening_a_project() {
    // §13.3.8: 管理はプロジェクトを選択する画面（§13.4）。編集中の画面には出さない
    let spec = section_13_3_8();
    assert!(spec.contains("プロジェクトを選択する画面（§13.4）で行い"));
    assert!(spec.contains("編集中の画面には出さない"));
    assert!(spec.contains("未保存の変更は含まない"));
    assert!(spec.contains("名前を入力させて確かめる"));
}
