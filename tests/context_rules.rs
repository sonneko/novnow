//! `core::resolve::context_rules`（§9.5.2 参照ペインの優先順位 / §15.4 ContextPacket との共用）
//! のテスト。
//!
//! `PLANS.md` §5.4 の作業項目 5 に対応する。**§9.5.2 の 9 セクションの順**と
//! **§15.4.1 の 9 ブロックの順**は固定（MUST）なので、表をそのまま期待値にする。
//! 「UI と LLM で情報源と優先順位を一致させる」（§15.4 MUST）は、両方が
//! [`PaneSection::packet_blocks`] の 1 枚の対応表を引くことで担保される。

use novnow::core::diagnostics::{Diagnostic, DiagnosticCode};
use novnow::core::resolve::context_rules::{
    self, Layout, MaterialSource, PacketBlock, PaneSection, Scope, PANE_FARTHEST, UPPER_PLOT_LINES,
};

/// §9.5.2 の優先順位を確かめるための枠（行 index）。
///
/// ```text
///  0 ..  1  話（episode）の始まり
///  2 ..  3  場面（scene）の始まり
///  4 ..  7  節（section）
///  5        カーソル行
/// ```
fn layout() -> Layout {
    Layout {
        cursor_line: 5,
        section: Some(4..8),
        scene: Some(2..10),
        episode: Some(0..20),
    }
}

// ============================================================ 優先順位（§9.5.2）

#[test]
fn the_priority_ladder_is_cursor_section_scene_episode() {
    // 「カーソル行 > 同一節 > 同一場面 > 同一話」（v0 踏襲・固定）
    assert_eq!(Scope::ALL.map(|scope| scope.order()), [0, 1, 2, 3]);
    assert!(Scope::CursorLine < Scope::Section);
    assert!(Scope::Section < Scope::Scene);
    assert!(Scope::Scene < Scope::Episode);
    assert_eq!(
        Scope::ALL.map(|scope| scope.key()),
        ["cursor-line", "section", "scene", "episode"]
    );
}

#[test]
fn the_pane_shows_out_to_the_same_episode() {
    // §9.5.2 の優先順位は「同一話」まで。別の話の項目は出さない
    for scope in Scope::ALL {
        assert!(PANE_FARTHEST.includes(scope), "{scope:?}");
    }
    assert!(!Scope::Section.includes(Scope::Scene));
    assert!(Scope::Section.includes(Scope::Section));
    assert!(Scope::Section.includes(Scope::CursorLine));
}

#[test]
fn the_nearest_scope_wins() {
    let found = layout();
    // カーソル行は節にも場面にも話にも含まれるが、**最も近い 1 つ**だけを返す
    assert_eq!(found.scope_of_line(5), Some(Scope::CursorLine));
    assert_eq!(found.scope_of_line(4), Some(Scope::Section));
    assert_eq!(found.scope_of_line(7), Some(Scope::Section));
    assert_eq!(found.scope_of_line(2), Some(Scope::Scene));
    assert_eq!(found.scope_of_line(9), Some(Scope::Scene));
    assert_eq!(found.scope_of_line(0), Some(Scope::Episode));
    assert_eq!(found.scope_of_line(19), Some(Scope::Episode));
}

#[test]
fn a_line_outside_everything_has_no_scope() {
    let found = layout();
    assert_eq!(found.scope_of_line(20), None);
    assert_eq!(found.scope_of_line(999), None);
    // 枠を何も持たなければカーソル行だけ
    let bare = Layout {
        cursor_line: 3,
        ..Layout::default()
    };
    assert_eq!(bare.scope_of_line(3), Some(Scope::CursorLine));
    assert_eq!(bare.scope_of_line(2), None);
}

#[test]
fn items_are_ranked_by_scope_then_by_line() {
    let found = layout();
    let ranked = found.ranked(
        PANE_FARTHEST,
        [
            (0, "話の冒頭"),
            (25, "別の話"),
            (5, "カーソル行"),
            (2, "場面"),
            (4, "節"),
        ],
    );
    assert_eq!(
        ranked
            .iter()
            .map(|(scope, line, item)| format!("{scope:?}@{line}:{item}"))
            .collect::<Vec<_>>()
            .join(" / "),
        "CursorLine@5:カーソル行 / Section@4:節 / Scene@2:場面 / Episode@0:話の冒頭"
    );
    // 範囲外（別の話）は**捨てる**
    assert_eq!(ranked.len(), 4);
}

#[test]
fn a_narrower_cutoff_drops_the_farther_items() {
    let found = layout();
    let ranked = found.ranked(
        Scope::Section,
        [(0, "話"), (2, "場面"), (4, "節"), (5, "行")],
    );
    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0].2, "行");
    assert_eq!(ranked[1].2, "節");
}

#[test]
fn the_same_ladder_serves_the_pane_and_the_packet() {
    // §15.4 の MUST「UI と LLM で情報源と優先順位を一致させる」。
    // 同じ `Layout` を使い、**上限だけ**を変える: ペインは同一話まで（§9.5.2）、
    // パケットの `6 lexicon` は「当該節に現れる用語」（§15.4.1）なので節まで
    let found = layout();
    let items = [(4, "節の用語"), (2, "場面の用語"), (0, "話の用語")];
    assert_eq!(found.ranked(PANE_FARTHEST, items).len(), 3);
    let packet = found.ranked(Scope::Section, items);
    assert_eq!(packet.len(), 1);
    assert_eq!(packet[0].2, "節の用語");
}

#[test]
fn the_plot_section_falls_back_to_the_scene() {
    // §9.5.2 の 1「話プロットの当該節テキスト。**無ければ場面テキスト**」
    let found = layout();
    assert_eq!(found.plot_span(), Some(4..8));
    let empty = Layout {
        section: Some(4..4),
        ..layout()
    };
    assert_eq!(empty.plot_span(), Some(2..10));
    let none = Layout {
        section: None,
        ..layout()
    };
    assert_eq!(none.plot_span(), Some(2..10));
    let bare = Layout::default();
    assert_eq!(bare.plot_span(), None);
}

// ============================================================ 表示順（§9.5.2 の 9 セクション）

#[test]
fn the_pane_sections_are_in_the_spec_order() {
    let orders: Vec<u8> = PaneSection::ALL.iter().map(|found| found.order()).collect();
    assert_eq!(orders, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(
        PaneSection::ALL.map(|found| found.key()),
        [
            "plot",
            "upper-plot",
            "cast",
            "words",
            "senses",
            "space",
            "materials",
            "illustrations",
            "diagnostics"
        ]
    );
    // 宣言順（派生した `Ord`）が表示順と一致するので、UI は enum をそのまま並べられる
    for left in PaneSection::ALL {
        for right in PaneSection::ALL {
            assert_eq!(left.cmp(&right), left.order().cmp(&right.order()));
        }
    }
}

#[test]
fn only_senses_space_and_illustrations_hide_their_heading() {
    // §9.5.2 の**節の出現規則（MUST）**: 5・6・8 は内容が無ければ見出しごと出さない。
    // 1〜4・7・9 は中身だけ省略する（「—」も書かない）
    let hidden = PaneSection::ALL
        .iter()
        .filter(|found| found.hides_heading_when_empty())
        .map(|found| found.order())
        .collect::<Vec<_>>();
    assert_eq!(hidden, vec![5, 6, 8]);
    assert!(!PaneSection::Plot.hides_heading_when_empty());
    assert!(PaneSection::Senses.hides_heading_when_empty());
    assert!(PaneSection::Space.hides_heading_when_empty());
    assert!(PaneSection::Illustrations.hides_heading_when_empty());
}

#[test]
fn the_upper_plot_is_two_lines() {
    // §9.5.2 の 2「章プロットの `summary` 1 行 → 全体プロットの `summary` 1 行。計 2 行だけ」
    assert_eq!(UPPER_PLOT_LINES, 2);
}

#[test]
fn the_material_section_orders_its_three_sources() {
    // §9.5.2 の 7「明示参照 → 本文の引用注釈 → 条件一致候補の順」
    let orders = [
        MaterialSource::Explicit,
        MaterialSource::Quote,
        MaterialSource::Condition,
    ]
    .iter()
    .map(|source| PaneSection::Materials.source_order(*source))
    .collect::<Vec<_>>();
    assert_eq!(orders, vec![Some(0), Some(1), Some(2)]);
    // この 3 経路の順は**資料セクションだけ**の特則（他はスコープ順）
    assert_eq!(
        PaneSection::Plot.source_order(MaterialSource::Explicit),
        None
    );
    assert_eq!(PaneSection::Words.source_order(MaterialSource::Quote), None);
    assert!(MaterialSource::Explicit < MaterialSource::Condition);
}

// ============================================================ §15.4.1 の 9 ブロック

#[test]
fn the_packet_blocks_are_in_the_spec_order() {
    let orders: Vec<u8> = PacketBlock::ALL.iter().map(|found| found.order()).collect();
    assert_eq!(orders, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    // 見出しは**固定**（LLM 側のパース安定性のため、§15.4.1）
    assert_eq!(
        PacketBlock::ALL.map(|found| found.heading()),
        [
            "## 1 project",
            "## 2 outline",
            "## 3 plot",
            "## 4 target",
            "## 5 cast",
            "## 6 lexicon",
            "## 7 materials",
            "## 8 constraints",
            "## 9 history"
        ]
    );
    for left in PacketBlock::ALL {
        for right in PacketBlock::ALL {
            assert_eq!(left.cmp(&right), left.order().cmp(&right.order()));
        }
    }
}

#[test]
fn project_and_constraints_cannot_be_dropped() {
    // §5.9.2「`constraints` と `project` は常に含まれる（外せない、MUST）」
    let mandatory = PacketBlock::ALL
        .iter()
        .filter(|found| found.is_mandatory())
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        mandatory,
        vec![PacketBlock::Project, PacketBlock::Constraints]
    );
}

#[test]
fn four_blocks_are_never_trimmed() {
    // §15.4.1「決して削らないもの（MUST）: `5 cast`（有効モード）、`3 plot` の当該節、
    // `8 constraints`、`1 project` の文体規約」
    let kept = PacketBlock::ALL
        .iter()
        .filter(|found| found.is_never_trimmed())
        .map(|found| found.order())
        .collect::<Vec<_>>();
    assert_eq!(kept, vec![1, 3, 5, 8]);
}

#[test]
fn every_pane_section_points_at_its_packet_source() {
    // §15.4 MUST「UI と LLM で情報源と優先順位を一致させる」＝この 1 枚の対応表を両方が引く
    let rows = [
        (PaneSection::Plot, vec![PacketBlock::Plot]),
        (PaneSection::UpperPlot, vec![PacketBlock::Outline]),
        (PaneSection::Cast, vec![PacketBlock::Cast]),
        (PaneSection::Words, vec![PacketBlock::Lexicon]),
        (PaneSection::Senses, vec![PacketBlock::Plot]),
        (PaneSection::Space, vec![PacketBlock::Plot]),
        (PaneSection::Materials, vec![PacketBlock::Materials]),
        (
            PaneSection::Illustrations,
            vec![PacketBlock::Plot, PacketBlock::Target],
        ),
        (PaneSection::Diagnostics, vec![]),
    ];
    for (section, blocks) in rows {
        assert_eq!(section.packet_blocks().to_vec(), blocks, "{section:?}");
    }
}

#[test]
fn the_packet_blocks_point_back_at_the_pane() {
    assert_eq!(
        PacketBlock::Plot.pane_sections(),
        vec![
            PaneSection::Plot,
            PaneSection::Senses,
            PaneSection::Space,
            PaneSection::Illustrations
        ]
    );
    assert_eq!(
        PacketBlock::Outline.pane_sections(),
        vec![PaneSection::UpperPlot]
    );
    assert_eq!(PacketBlock::Cast.pane_sections(), vec![PaneSection::Cast]);
    assert_eq!(
        PacketBlock::Lexicon.pane_sections(),
        vec![PaneSection::Words]
    );
    assert_eq!(
        PacketBlock::Materials.pane_sections(),
        vec![PaneSection::Materials]
    );
    assert_eq!(
        PacketBlock::Target.pane_sections(),
        vec![PaneSection::Illustrations]
    );
    // ペインに出ないブロック（§9.5.2 の 9 セクションに無い）
    assert!(PacketBlock::Project.pane_sections().is_empty());
    assert!(PacketBlock::Constraints.pane_sections().is_empty());
    assert!(PacketBlock::History.pane_sections().is_empty());
}

// ============================================================ §9.5.2 の 9（診断の順）

fn tag(list: &[Diagnostic]) -> String {
    list.iter()
        .map(|found| {
            format!(
                "{}@{}",
                found.code.as_str(),
                found
                    .range
                    .clone()
                    .map_or("-".to_string(), |range| range.start.to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

#[test]
fn diagnostics_are_sorted_error_then_warning_then_position() {
    // §9.5.2 の 9「`E` → `W` の順、各 1 行」
    let mut list = vec![
        Diagnostic::simple(DiagnosticCode::W006, Some(10..20)),
        Diagnostic::simple(DiagnosticCode::I050, Some(0..1)),
        Diagnostic::simple(DiagnosticCode::E005, Some(30..40)),
        Diagnostic::simple(DiagnosticCode::E002, Some(5..6)),
        Diagnostic::simple(DiagnosticCode::W031, Some(2..3)),
    ];
    context_rules::sort_diagnostics(&mut list);
    assert_eq!(tag(&list), "E002@5 / E005@30 / W031@2 / W006@10 / I050@0");
}

#[test]
fn a_diagnostic_without_a_position_sorts_first_in_its_severity() {
    // §7.1 の `Anchor::Status` は文書位置を持たない（`range: None`）。
    // 位置 0 として扱うので、同じ重大度の中では先頭に来る
    let mut list = vec![
        Diagnostic::simple(DiagnosticCode::E002, Some(5..6)),
        Diagnostic::simple(DiagnosticCode::E005, None),
    ];
    context_rules::sort_diagnostics(&mut list);
    assert_eq!(tag(&list), "E005@- / E002@5");
}

#[test]
fn the_same_position_sorts_by_code() {
    let mut list = vec![
        Diagnostic::simple(DiagnosticCode::E005, Some(5..6)),
        Diagnostic::simple(DiagnosticCode::E002, Some(5..6)),
    ];
    context_rules::sort_diagnostics(&mut list);
    assert_eq!(tag(&list), "E002@5 / E005@5");
}

#[test]
fn codes_alone_sort_by_severity_then_by_code() {
    let mut list = vec![
        DiagnosticCode::W006,
        DiagnosticCode::E005,
        DiagnosticCode::I050,
        DiagnosticCode::E002,
    ];
    context_rules::sort_codes(&mut list);
    assert_eq!(
        list.iter().map(|found| found.as_str()).collect::<Vec<_>>(),
        vec!["E002", "E005", "W006", "I050"]
    );
    assert_eq!(context_rules::severity_rank(list[0].severity()), 0);
    assert_eq!(context_rules::severity_rank(list[2].severity()), 1);
    assert_eq!(context_rules::severity_rank(list[3].severity()), 2);
}
