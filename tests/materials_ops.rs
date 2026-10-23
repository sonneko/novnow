//! 資料の操作（`core::features::materials_ops`・`core::text::sha256`）と、参照ペインの資料
//! （`core::features::reference`、§16.4）。M8、`docs/history.md` B-131・B-134〜B-136。

mod world;

use novnow::core::diagnostics::{self, DiagnosticCode};
use novnow::core::features::{materials_ops, plots, reference, sections};
use novnow::core::id::SceneId;
use novnow::core::model::document::Document;
use novnow::core::model::material::MaterialStatus;
use novnow::core::syntax::{materials_index, plot_episode};
use novnow::core::text::lexer::FieldKey;
use novnow::core::text::sha256;

use world::{world, world_with, CONTENTS_PATH, EPISODE_PATH};

fn id(chapter: u16, episode: u16, scene: u16, section: u16) -> SceneId {
    SceneId {
        chapter,
        episode,
        scene,
        section,
    }
}

// ============================================================ SHA-256（FIPS 180-4）

#[test]
fn sha256_matches_the_fips_examples_and_the_block_boundaries() {
    let cases: [(&[u8], &str); 6] = [
        (
            b"",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        // 2 ブロック目にはみ出す 56 byte（FIPS 180-4 の例）
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
        // 55 byte = 1 ブロックに長さが収まる最長
        (
            &[b'a'; 55],
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
        ),
        (
            &[b'a'; 56],
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
        ),
        (
            &[b'a'; 64],
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
        ),
    ];
    for (data, expected) in cases {
        assert_eq!(sha256::hex(data), expected, "{data:?}");
    }
    assert_eq!(sha256::digest(b"abc")[..4], [0xba, 0x78, 0x16, 0xbf]);
    let million = vec![b'a'; 1_000_000];
    assert_eq!(
        sha256::hex(&million),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
    // checksum は §5.8.2 の例の形（`sha256:` + 小文字 16 進）。UTF-8 の byte に対して取る
    assert_eq!(
        materials_ops::checksum("資料"),
        "sha256:58e891bce783aadf2fe59702d8efa2697f3b23ab6d8317f9a20a069b25b66e2a"
    );
}

// ============================================================ 索引の書き換え

const INDEX: &str = r#"{
  "schema_version": 1,
  "next_id": 140,
  "items": [
    {
      "id": "M0137",
      "title": "科学の甲子園 本戦の運営と実技課題",
      "path": "materials/docs/M0137-kagaku-koshien.md",
      "source": { "kind": "url", "url": "https://example.org/report/2026", "accessed_at": "2026-10-01T09:12:00+09:00" },
      "category": "取材",
      "conditions": [
        { "kind": "scope", "value": "@02-03 .. @02-08" },
        { "kind": "ref",   "value": "'科学の甲子園" },
        { "kind": "text",  "value": "大会運営の手順を描写するとき" }
      ],
      "saved_at": "2026-10-01T09:20:33+09:00",
      "tags": ["大会", "高校", "運営"],
      "status": "approved",
      "added_by": "human",
      "summary": "本戦の 1 日のタイムテーブルと実技課題の採点基準。"
    },
    {
      "id": "M0138",
      "title": "Old School Building 旧校舎の歴史",
      "path": "materials/inbox/M0138.md",
      "source": { "kind": "custom", "text": "" },
      "category": "文献",
      "conditions": [],
      "saved_at": "2026-10-05T10:00:00+09:00",
      "tags": [],
      "status": "proposed",
      "added_by": "human"
    },
    {
      "id": "M0139",
      "title": "甲子園球場の歴史",
      "path": "materials/inbox/M0139.md",
      "source": { "kind": "url", "url": "https://example.org/stadium" },
      "category": "Web",
      "conditions": [],
      "saved_at": "2026-10-06T10:00:00+09:00",
      "tags": [],
      "status": "rejected",
      "added_by": "human"
    }
  ]
}
"#;

fn with_index(text: &str) -> novnow::core::features::Project {
    let mut project = world_with(materials_ops::INDEX_PATH, text);
    project.insert(Document::new(
        "materials/docs/M0137-kagaku-koshien.md",
        "# 科学の甲子園 本戦の運営と実技課題\n<!-- novnow:M0137 -->\n\n実技課題は午後に行う。\n",
    ));
    project.insert(Document::new(
        "materials/inbox/M0138.md",
        "# Old School Building 旧校舎の歴史\n<!-- novnow:M0138 -->\n\n明治の木造校舎。\n",
    ));
    project
}

fn ids(hits: &[materials_ops::Hit]) -> Vec<&str> {
    hits.iter().map(|hit| hit.id.as_str()).collect()
}

#[test]
fn search_follows_the_query_language_of_16_3() {
    let project = with_index(INDEX);
    let search = |query: &str| materials_ops::search(&project, query);
    // 素のテキスト: 却下は既定で出さない（B-134）
    assert_eq!(ids(&search("甲子園")), vec!["M0137"]);
    assert_eq!(ids(&search("甲子園 !rejected")), vec!["M0139"]);
    // 空の検索は却下以外の全部（saved_at の新しい順）
    assert_eq!(ids(&search("")), vec!["M0138", "M0137"]);
    // `#カテゴリ` は完全一致
    assert_eq!(ids(&search("#取材")), vec!["M0137"]);
    assert_eq!(ids(&search("#取")), Vec::<&str>::new());
    // `@範囲` は scope と交差
    assert_eq!(ids(&search("@02-05")), vec!["M0137"]);
    assert_eq!(ids(&search("@01-01")), Vec::<&str>::new());
    // `'参照` は ref 条件と一致（`{}` は外して比べる）
    assert_eq!(ids(&search("'科学の甲子園")), vec!["M0137"]);
    assert_eq!(ids(&search("'{科学の甲子園}")), vec!["M0137"]);
    // ID の前方一致（大文字小文字を問わない）
    assert_eq!(ids(&search("M013")), vec!["M0138", "M0137"]);
    assert_eq!(ids(&search("m0138")), vec!["M0138"]);
    // `!status` と `~日付`
    assert_eq!(ids(&search("!proposed")), vec!["M0138"]);
    assert_eq!(ids(&search("~2026-10-05")), vec!["M0138"]);
    // AND。あいまい一致（§6.1.3 の正規化 ＋ 大文字小文字・全角英字）
    assert_eq!(ids(&search("old SCHOOL")), vec!["M0138"]);
    assert_eq!(ids(&search("ＯＬＤ")), vec!["M0138"]);
    assert_eq!(ids(&search("old 甲子園")), Vec::<&str>::new());
    // 本文も見る（重みは一番低い）
    assert_eq!(ids(&search("明治")), vec!["M0138"]);
    let hits = search("実技");
    assert_eq!(ids(&hits), vec!["M0137"]);
    assert_eq!(hits[0].date, "2026-10-01");
    assert_eq!(hits[0].category, "取材");
    assert_eq!(hits[0].source, "https://example.org/report/2026");
}

#[test]
fn search_ranks_title_over_tag_over_summary_over_body() {
    let index = r#"{
  "schema_version": 1, "next_id": 4,
  "items": [
    { "id": "M0001", "title": "雑記", "path": "materials/docs/M0001.md", "source": { "kind": "custom", "text": "x" },
      "category": "", "conditions": [], "saved_at": "2026-10-09", "tags": [], "status": "approved", "added_by": "human" },
    { "id": "M0002", "title": "雑記", "path": "materials/docs/M0002.md", "source": { "kind": "custom", "text": "x" },
      "category": "", "conditions": [], "saved_at": "2026-10-08", "tags": [], "status": "approved", "added_by": "human",
      "summary": "時計塔の点検" },
    { "id": "M0003", "title": "雑記", "path": "materials/docs/M0003.md", "source": { "kind": "custom", "text": "x" },
      "category": "", "conditions": [], "saved_at": "2026-10-07", "tags": ["時計塔"], "status": "approved", "added_by": "human" },
    { "id": "M0004", "title": "時計塔", "path": "materials/docs/M0004.md", "source": { "kind": "custom", "text": "x" },
      "category": "", "conditions": [], "saved_at": "2026-10-06", "tags": [], "status": "approved", "added_by": "human" }
  ]
}"#;
    let mut project = world_with(materials_ops::INDEX_PATH, index);
    project.insert(Document::new(
        "materials/docs/M0001.md",
        "# 雑記\n時計塔の話。\n",
    ));
    let hits = materials_ops::search(&project, "時計塔");
    assert_eq!(ids(&hits), vec!["M0004", "M0003", "M0002", "M0001"]);
    let scores: Vec<u32> = hits.iter().map(|hit| hit.score).collect();
    assert_eq!(scores, vec![4, 3, 2, 1]);
}

#[test]
fn draft_of_takes_the_plain_words_and_the_first_category() {
    assert_eq!(
        materials_ops::draft_of("甲子園 #取材 @02-03 運営 #文献 'アルス"),
        ("甲子園 運営".to_owned(), "取材".to_owned())
    );
    assert_eq!(
        materials_ops::draft_of("M0137 !approved"),
        (String::new(), String::new())
    );
}

#[test]
fn update_rewrites_the_reviewed_fields_and_keeps_what_it_did_not_touch() {
    let item = materials_ops::items(INDEX)
        .into_iter()
        .find(|item| item.id == "M0137")
        .expect("M0137");
    let mut review = materials_ops::Review::of(&item);
    assert_eq!(review.tags, "大会, 高校, 運営");
    assert_eq!(
        review.conditions,
        "scope: @02-03 .. @02-08\nref: '科学の甲子園\ntext: 大会運営の手順を描写するとき"
    );
    assert_eq!(review.source, "https://example.org/report/2026");
    review.title = "  科学の甲子園 本戦  ".to_owned();
    review.tags = "大会、高校,  決勝 ".to_owned();
    review.conditions =
        "scope: @02-03 .. @02-09\nref: '科学の甲子園\n手順の描写\ntag: 大会".to_owned();
    let updated =
        materials_ops::update(INDEX, "M0137", &review, "2026-10-13T22:00:00+09:00").expect("ok");
    let parsed = materials_index::parse(&updated).index.expect("index");
    let item = &parsed.items[0];
    assert_eq!(item.title, "科学の甲子園 本戦");
    assert_eq!(item.tags, vec!["大会", "高校", "決勝"]);
    let conditions: Vec<(&str, &str)> = item
        .conditions
        .iter()
        .map(|found| (found.kind.as_str(), found.value.as_str()))
        .collect();
    assert_eq!(
        conditions,
        vec![
            ("scope", "@02-03 .. @02-09"),
            ("ref", "'科学の甲子園"),
            ("text", "手順の描写"),
            ("tag", "大会"),
        ]
    );
    assert_eq!(
        item.updated_at.as_deref(),
        Some("2026-10-13T22:00:00+09:00")
    );
    // 出典に触れていなければ accessed_at も残る
    assert_eq!(
        item.source.accessed_at.as_deref(),
        Some("2026-10-01T09:12:00+09:00")
    );
    // 出典を変えたら書き直す（URL でなければ custom の自由文）。フォームは打鍵ごとに書くので、
    // 書き直しても accessed_at は残す（途中の打鍵で消えないように）
    review.source = "図書館の郷土資料".to_owned();
    let updated = materials_ops::update(&updated, "M0137", &review, "t").expect("ok");
    let parsed = materials_index::parse(&updated).index.expect("index");
    assert_eq!(parsed.items[0].source.kind, "custom");
    assert_eq!(parsed.items[0].source.shown(), "図書館の郷土資料");
    assert!(parsed.items[0].source.url.is_none());
    assert_eq!(
        parsed.items[0].source.accessed_at.as_deref(),
        Some("2026-10-01T09:12:00+09:00")
    );
    // 空にすれば出典なし（W065 の条件）
    review.source = String::new();
    let updated = materials_ops::update(&updated, "M0137", &review, "t").expect("ok");
    let parsed = materials_index::parse(&updated).index.expect("index");
    assert!(materials_ops::has_no_source(&parsed.items[0]));
    // 無い ID・E025 は何もしない
    assert!(materials_ops::update(INDEX, "M9999", &review, "t").is_none());
    assert!(materials_ops::update("{ broken", "M0137", &review, "t").is_none());
}

#[test]
fn touch_writes_updated_at_and_the_checksum_of_the_saved_body() {
    let body = "# 科学の甲子園\n本文\n";
    let updated = materials_ops::touch(
        INDEX,
        "materials/docs/M0137-kagaku-koshien.md",
        "2026-10-13T22:30:00+09:00",
        body,
    )
    .expect("registered path");
    let parsed = materials_index::parse(&updated).index.expect("index");
    assert_eq!(
        parsed.items[0].updated_at.as_deref(),
        Some("2026-10-13T22:30:00+09:00")
    );
    assert_eq!(
        parsed.items[0].checksum.as_deref(),
        Some(materials_ops::checksum(body).as_str())
    );
    // 索引に無いパスは何もしない
    assert!(materials_ops::touch(INDEX, "materials/inbox/other.md", "t", body).is_none());
}

#[test]
fn register_issues_a_new_id_and_writes_the_id_comment() {
    // H1 があり、ID コメントが無い → H1 の次の行に足す。inbox は提案
    let created = materials_ops::register(
        INDEX,
        "materials/inbox/memo.md",
        "# 取材メモ\n本文\n",
        "2026-10-13T22:00:00+09:00",
    )
    .expect("register");
    assert_eq!(
        created.id, "M0140",
        "next_id から（MD に書かれた ID は使わない）"
    );
    assert_eq!(created.body, "# 取材メモ\n<!-- novnow:M0140 -->\n本文\n");
    let parsed = materials_index::parse(&created.index).index.expect("index");
    let item = parsed.items.last().expect("item");
    assert_eq!(item.title, "取材メモ");
    assert_eq!(item.status, "proposed");
    assert_eq!(parsed.next_id, 141);
    // docs/ の下は採用。古い ID コメントは新しい ID に書き換える
    let created = materials_ops::register(
        INDEX,
        "materials/docs/old.md",
        "# 古い資料\n<!-- novnow:M0001 -->\n",
        "t",
    )
    .expect("register");
    assert!(created.body.contains("<!-- novnow:M0140 -->"));
    assert!(!created.body.contains("M0001"));
    let parsed = materials_index::parse(&created.index).index.expect("index");
    assert_eq!(
        parsed.items.last().map(|item| item.status.as_str()),
        Some("approved")
    );
    // H1 が無ければファイル名をタイトルにして H1 ごと足す
    let created =
        materials_ops::register(INDEX, "materials/inbox/no-heading.md", "本文だけ\n", "t")
            .expect("register");
    assert!(created
        .body
        .starts_with("# no-heading\n<!-- novnow:M0140 -->\n本文だけ"));
    // 登録済み・E025 なら何もしない
    assert!(materials_ops::register(INDEX, "materials/inbox/M0138.md", "", "t").is_none());
    assert!(materials_ops::register("{ broken", "materials/inbox/x.md", "", "t").is_none());
}

#[test]
fn unregistered_material_files_get_w064_and_proposals_without_a_source_get_w065() {
    let mut project = with_index(INDEX);
    project.insert(Document::new("materials/inbox/memo.md", "# 取材メモ\n"));
    assert_eq!(
        materials_ops::unregistered(&project),
        vec!["materials/inbox/memo.md".to_owned()]
    );
    let memo = project
        .document("materials/inbox/memo.md")
        .cloned()
        .expect("memo");
    let codes: Vec<DiagnosticCode> = diagnostics::document(&memo, &project)
        .iter()
        .map(|found| found.code)
        .collect();
    assert!(codes.contains(&DiagnosticCode::W064), "{codes:?}");
    // 登録済みの資料 MD には出さない
    let registered = project
        .document("materials/inbox/M0138.md")
        .cloned()
        .expect("M0138");
    assert!(diagnostics::document(&registered, &project)
        .iter()
        .all(|found| found.code != DiagnosticCode::W064));
    // W065 は出典の無い**提案**だけ（M0138）。採用済み・却下には出さない
    let parsed = materials_index::parse(INDEX);
    let w065: Vec<&str> = parsed
        .diagnostics
        .iter()
        .filter(|found| found.code == DiagnosticCode::W065)
        .map(|found| &INDEX[found.range.clone().expect("range")])
        .collect();
    assert_eq!(w065, vec!["\"M0138\""]);
}

#[test]
fn approving_moves_the_file_and_rejecting_keeps_it() {
    let rejected =
        materials_ops::set_status(INDEX, "M0138", MaterialStatus::Rejected, "t1").expect("reject");
    assert_eq!(rejected.rename, None);
    let approved =
        materials_ops::set_status(INDEX, "M0138", MaterialStatus::Approved, "t2").expect("approve");
    assert_eq!(
        approved.rename,
        Some((
            "materials/inbox/M0138.md".to_owned(),
            "materials/docs/M0138.md".to_owned()
        ))
    );
    let parsed = materials_index::parse(&approved.index)
        .index
        .expect("index");
    assert_eq!(parsed.items[1].path, "materials/docs/M0138.md");
    // 却下しても ID は回収しない（次の提案は M0140）
    let next = materials_ops::propose(&rejected.index, "次", "", "t").expect("propose");
    assert_eq!(next.id, "M0140");
}

// ============================================================ 参照ペインの資料（§16.4、B-136）

const CONDITIONS: &str = r#"{
  "schema_version": 1,
  "next_id": 205,
  "items": [
    { "id": "M0137", "title": "科学の甲子園", "path": "materials/docs/M0137.md",
      "source": { "kind": "custom", "text": "x" }, "category": "取材",
      "conditions": [ { "kind": "ref", "value": "'リリア" } ],
      "saved_at": "2026-10-01", "tags": [], "status": "approved", "added_by": "human" },
    { "id": "M0200", "title": "図書室の配置", "path": "materials/docs/M0200.md",
      "source": { "kind": "custom", "text": "x" }, "category": "取材",
      "conditions": [ { "kind": "ref", "value": "'{リリア}" } ],
      "saved_at": "2026-10-02", "tags": [], "status": "approved", "added_by": "human" },
    { "id": "M0201", "title": "第 4 話の舞台", "path": "materials/docs/M0201.md",
      "source": { "kind": "custom", "text": "x" }, "category": "取材",
      "conditions": [ { "kind": "scope", "value": "@01-04 .. @01-05" } ],
      "saved_at": "2026-10-03", "tags": [], "status": "approved", "added_by": "human" },
    { "id": "M0202", "title": "古い書架", "path": "materials/docs/M0202.md",
      "source": { "kind": "custom", "text": "x" }, "category": "取材",
      "conditions": [ { "kind": "tag", "value": "書架" } ],
      "saved_at": "2026-10-04", "tags": [], "status": "approved", "added_by": "human" },
    { "id": "M0203", "title": "提案のまま", "path": "materials/inbox/M0203.md",
      "source": { "kind": "custom", "text": "x" }, "category": "取材",
      "conditions": [ { "kind": "scope", "value": "@01" } ],
      "saved_at": "2026-10-05", "tags": [], "status": "proposed", "added_by": "human" },
    { "id": "M0204", "title": "文章の条件だけ", "path": "materials/docs/M0204.md",
      "source": { "kind": "custom", "text": "x" }, "category": "取材",
      "conditions": [ { "kind": "text", "value": "図書室の場面" } ],
      "saved_at": "2026-10-06", "tags": [], "status": "approved", "added_by": "human" }
  ]
}"#;

/// `(ID, 候補か)` の並び。
fn materials_at(project: &novnow::core::features::Project, index: usize) -> Vec<(String, bool)> {
    let document = project.document(CONTENTS_PATH).cloned().expect("doc");
    let list = sections::all(project);
    let section = &list[index];
    reference::at(&document, section.lines.clone(), section.id, project)
        .materials
        .into_iter()
        .map(|found| (found.id, found.candidate))
        .collect()
}

fn pairs(items: &[(&str, bool)]) -> Vec<(String, bool)> {
    items
        .iter()
        .map(|(id, candidate)| ((*id).to_owned(), *candidate))
        .collect()
}

#[test]
fn the_plot_material_field_is_an_explicit_reference() {
    // 話プロットの 2 節目の `material: 'M0137`（本文には書いていない）
    let project = world();
    assert_eq!(materials_at(&project, 1), pairs(&[("M0137", false)]));
    // 1 節目には無い（場面レベルにも書いていない）
    assert_eq!(materials_at(&project, 0), Vec::new());
}

#[test]
fn condition_matches_follow_the_explicit_references_and_are_marked_as_candidates() {
    let mut project = world_with(materials_ops::INDEX_PATH, CONDITIONS);
    // 1 節目のプロットに `tags: 書架`
    let plot = project.text(EPISODE_PATH).replacen(
        "check: リリアが「怒っているが見捨てない」ことが伝わること\n",
        "check: リリアが「怒っているが見捨てない」ことが伝わること\ntags: 書架, 窓\n",
        1,
    );
    project.insert(Document::new(EPISODE_PATH, plot));
    // 1 節目: 'リリア が出てくる（ref: M0137・M0200）、@01-04（scope: M0201）、タグ（tag: M0202）。
    // 提案（M0203）と text だけの条件（M0204）は出さない
    assert_eq!(
        materials_at(&project, 0),
        pairs(&[
            ("M0137", true),
            ("M0200", true),
            ("M0201", true),
            ("M0202", true)
        ])
    );
    // 2 節目: M0137 は話プロットの明示参照なので候補にしない（重複もさせない）。タグは 1 節目だけ
    assert_eq!(
        materials_at(&project, 1),
        pairs(&[("M0137", false), ("M0200", true), ("M0201", true)])
    );
}

#[test]
fn b_format_plots_read_tags_as_the_tags_key_without_w011() {
    // §5.0.6 の表では `tags` は Markdown 形式のキーだが、表にあるキーは B 形式でもそのまま読む
    // （W011 は表に無いキーだけ）。§16.4 の `tag` 条件はこれと照合する（B-136）
    let text = "@01-04-02 図書室の攻防\ntags: 書架\n本文。\n;\nタグ: 窓\n二つ目。\n";
    let plot = plot_episode::parse(text);
    let scene: Vec<(FieldKey, &str, &str)> = plot
        .scene_fields(0, FieldKey::Tags)
        .iter()
        .map(|field| (field.key, field.key_text, field.value))
        .collect();
    assert_eq!(scene, vec![(FieldKey::Tags, "tags", "書架")]);
    let section: Vec<(&str, &str)> = plot
        .section_fields(0, 1, FieldKey::Tags)
        .iter()
        .map(|field| (field.key_text, field.value))
        .collect();
    assert_eq!(section, vec![("タグ", "窓")]);
    assert!(plot.scene_fields(0, FieldKey::Unknown).is_empty());
    assert!(
        plot.diagnostics
            .iter()
            .all(|found| found.code != DiagnosticCode::W011),
        "{:?}",
        plot.diagnostics
    );
}

#[test]
fn append_line_puts_the_material_line_at_the_end_of_the_section() {
    let project = world();
    let rows = plots::episode_rows(&project, 1, 4);
    let text = project.text(EPISODE_PATH);
    let updated = plots::append_line(text, &rows, id(1, 4, 2, 1), "material: 'M0200").expect("row");
    let after = plot_episode::parse(&updated);
    let first = after.section_fields(0, 0, FieldKey::Material);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].value, "'M0200");
    // ほかの節は変わらない（2 節目の material は 1 つのまま）
    assert_eq!(after.section_fields(0, 1, FieldKey::Material).len(), 1);
    assert_eq!(updated.lines().count(), text.lines().count() + 1);
    // 最後の節（ファイル末尾の改行の手前）
    let last = plots::append_line(text, &rows, id(1, 4, 2, 4), "material: 'M0200").expect("row");
    assert!(
        last.ends_with("state: fixed\nmaterial: 'M0200\n;\n"),
        "{last}"
    );
    // 場面だけの行（節 0）は場面レベルの最後（最初の節の手前）ではなく、場面の行の範囲の最後
    assert!(plots::append_line(text, &rows, id(1, 4, 9, 1), "x").is_none());
}
