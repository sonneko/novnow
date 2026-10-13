//! アイデアと資料のワークスペースの操作（`core::features::ideas` / `materials_ops`、M8、
//! `PLANS.md` §9.2 B-130・B-131）。

mod novel;
mod world;

use novnow::core::features::{ideas, materials_ops};
use novnow::core::id::SceneId;
use novnow::core::model::idea::IdeaStatus;
use novnow::core::model::material::MaterialStatus;
use novnow::core::syntax::materials_index;

const IDEAS: &str = "# I0007 学院長の動機
status: keep
created: 2026-10-09T12:04:11+09:00
tags: 学院長, 動機
学院長は何を守ろうとしているのか。

# I0009 雨の日の図書室
status: dropped

# I0010 旧校舎の鍵
本文だけ。
";

#[test]
fn ideas_are_listed_with_status_tags_and_editable_lines() {
    let project = novel::novel_with(ideas::PATH, IDEAS);
    let rows = ideas::rows(&project);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].id, "I0007");
    assert_eq!(rows[0].title, "学院長の動機");
    assert_eq!(rows[0].status, IdeaStatus::Keep);
    assert_eq!(rows[0].tags, vec!["学院長", "動機"]);
    assert_eq!(rows[0].lines, 0..5, "末尾の空行を含まない");
    assert_eq!(rows[1].status, IdeaStatus::Dropped);
    assert_eq!(rows[2].status, IdeaStatus::Inbox, "status が無ければ inbox");
}

#[test]
fn capture_appends_an_inbox_idea_with_the_next_number() {
    assert_eq!(
        ideas::next_number(IDEAS),
        11,
        "最大値 + 1（欠番は埋めない）"
    );
    let (text, id) = ideas::capture(IDEAS, " 時計塔の鐘 ", "2026-10-13T21:40:00+09:00");
    assert_eq!(id, "I0011");
    assert!(text.starts_with(IDEAS));
    assert!(text.ends_with(
        "本文だけ。\n\n# I0011 時計塔の鐘\nstatus: inbox\ncreated: 2026-10-13T21:40:00+09:00\n"
    ));
    let (first, id) = ideas::capture("", "最初", "t");
    assert_eq!(id, "I0001");
    assert_eq!(first, "# I0001 最初\nstatus: inbox\ncreated: t\n");
}

#[test]
fn set_field_replaces_or_inserts_after_the_last_field() {
    let dropped = ideas::set_field(IDEAS, 0, "status", "dropped");
    assert!(dropped.starts_with("# I0007 学院長の動機\nstatus: dropped\ncreated:"));
    assert_eq!(
        dropped.lines().count(),
        IDEAS.lines().count(),
        "行は消さない"
    );
    let promoted = ideas::set_field(IDEAS, 2, "promoted-to", "words.md#旧校舎の鍵");
    assert!(promoted.contains("# I0010 旧校舎の鍵\npromoted-to: words.md#旧校舎の鍵\n本文だけ。"));
    let promoted = ideas::set_field(IDEAS, 0, "promoted-to", "x");
    assert!(promoted.contains("tags: 学院長, 動機\npromoted-to: x\n学院長は"));
    assert_eq!(
        ideas::content(IDEAS, 0),
        Some((
            "学院長の動機".to_owned(),
            "学院長は何を守ろうとしているのか。".to_owned()
        ))
    );
}

#[test]
fn promotion_targets_append_to_plots_words_and_characters() {
    let project = novel::novel();
    let digits = project.digits();
    let scene = SceneId {
        chapter: 1,
        episode: 1,
        scene: 2,
        section: 0,
    };
    // 閉じていない最後の節の後ろ: 閉じてから足す
    let text = project.text("plots/01-01.txt");
    let (updated, id) =
        ideas::append_plot_section(text, scene, &["時計塔の鐘".to_owned()], digits, &project);
    assert!(
        updated.ends_with("二人で帰る。\n;\n時計塔の鐘\n;\n"),
        "{updated}"
    );
    assert_eq!(id.section, 2);
    // 場面が無ければ見出しを書く
    let (created, id) = ideas::append_plot_section(
        "",
        SceneId { scene: 3, ..scene },
        &["鐘".to_owned()],
        digits,
        &project,
    );
    assert_eq!(created, "@01-01-03\n鐘\n;\n");
    assert_eq!(id.section, 1);
    let words = ideas::append_term("# 既存\ndefinition: 既存の用語。\n", "時計塔", "");
    assert_eq!(
        words,
        "# 既存\ndefinition: 既存の用語。\n\n# 時計塔\ndefinition: 時計塔\n"
    );
    let characters = ideas::append_character_memo(world::CHARACTERS, 0, "鐘の音が苦手");
    let first_end = characters.find("\n# ").unwrap_or(characters.len());
    assert!(
        characters[..first_end].contains("鐘の音が苦手"),
        "最初のキャラの最後"
    );
}

#[test]
fn proposing_a_material_issues_the_next_id_and_never_reuses_one() {
    let index = world::INDEX;
    let before = materials_index::parse(index).index.expect("index");
    let created = materials_ops::propose(
        index,
        "時計塔の構造",
        "鐘は 3 つ。",
        "2026-10-13T21:40:00+09:00",
    )
    .expect("propose");
    let after = materials_index::parse(&created.index).index.expect("index");
    assert_eq!(after.items.len(), before.items.len() + 1);
    let item = after.items.last().expect("item");
    assert_eq!(item.id, created.id);
    assert_eq!(item.status, "proposed");
    assert_eq!(item.added_by, "human");
    assert_eq!(created.path, format!("materials/inbox/{}.md", created.id));
    assert_eq!(
        created.body,
        format!(
            "# 時計塔の構造\n<!-- novnow:{} -->\n\n鐘は 3 つ。\n",
            created.id
        )
    );
    assert!(after.next_id > before.next_id);
    // 却下しても回収しない: 次の提案は別の ID
    let rejected = materials_ops::set_status(
        &created.index,
        &created.id,
        MaterialStatus::Rejected,
        "2026-10-13T22:00:00+09:00",
    )
    .expect("reject");
    assert_eq!(rejected.rename, None, "却下は inbox に残す");
    let again = materials_ops::propose(&rejected.index, "別の資料", "", "t").expect("propose");
    assert_ne!(again.id, created.id);
    // 採用は docs/ へ動かす
    let approved = materials_ops::set_status(
        &again.index,
        &again.id,
        MaterialStatus::Approved,
        "2026-10-13T22:10:00+09:00",
    )
    .expect("approve");
    assert_eq!(
        approved.rename,
        Some((
            format!("materials/inbox/{}.md", again.id),
            format!("materials/docs/{}.md", again.id)
        ))
    );
    let parsed = materials_index::parse(&approved.index)
        .index
        .expect("index");
    let item = parsed
        .items
        .iter()
        .find(|item| item.id == again.id)
        .expect("item");
    assert_eq!(item.status, "approved");
    assert_eq!(
        item.updated_at.as_deref(),
        Some("2026-10-13T22:10:00+09:00")
    );
    // E025（JSON として読めない）なら何もしない
    assert!(materials_ops::propose("{ broken", "x", "", "t").is_none());
    assert!(materials_ops::touch("{ broken", "materials/docs/M0137.md", "t").is_none());
}
