//! `core::syntax::characters_md`（`characters.md`、§5.5）のテスト。
//!
//! `PLANS.md` §5.3 完了条件の 1（§5.5.7 の例が期待どおりにパースできる）・
//! 2（未知フィールドキーを含むファイルの round-trip がバイト一致）・
//! 3（壊れた入力でパニックしない）に対応する。
//!
//! 比較は [`lines_tag`] / [`characters_tag`] / [`diags_tag`] が作る短い文字列で行う
//! （`tests/plot_outline.rs` と同じ流儀）。

use std::ops::Range;

use novnow::core::diagnostics::{Arg, Diagnostic};
use novnow::core::syntax::characters_md::{
    self, Character, Characters, Line, ModeDef, ModeRange, Relation, Section, SectionKey,
};
use novnow::core::syntax::contents::Comment;
use novnow::core::text::lexer::FieldKey;

/// §5.5.7 の完全な例（恩田恵）。**仕様のコード例をそのまま**使う（§5.3 完了条件 1）。
const SPEC_EXAMPLE: &str = r#"# 恩田恵
reading: おんだめぐみ
alias: めぐみ, 恵
role: メインヒロイン
default-mode: A
color: #8fb8ff

## attributes
属性: A高校サイエンス部員（高1→高2）。常にマイPCを持ち歩くテック女子。女子力は高い。
実績: 足音解析型自動扉の開発、科学の甲子園本戦における実技課題のプログラム・構造設計完遂、数理の翼夏季セミナー参加。
興味対象: 情報科学、プログラミング、工学、ハードウェア制御。
きっかけ: 幼少期にキーボードを叩いて自分の指示通りに機械が動いた瞬間の万能感と感動。

## tone
「*で…」「*です…」「…あの…*」

## modes

### A 日常
range: @01 ..
personality: おとなしく控えめ。'葵あおい の後ろに隠れていることが多い。
emotion: 平穏を好み、あおいと一緒にいられる時間を何よりの幸福と感じる。
role: 主人公の傍らに寄り添う可憐な「嫁」キャラ。
「*だよ…」「えへへ、*」

### B 開発・実装
range: @01 ..
personality: キーボードを叩く手つきは極めて鋭く、エラーの原因を冷徹に潰す職人的集中力を見せる。
emotion: 論理が完璧に噛み合い、コードが意図通りに動く瞬間の快感。
role: 実技・情報課題における最高峰のエンジニア。
「*ないと…！」「*ないんです」

### C 混乱・言語化障害
range: @01 .. @02
personality: 高度な仕様やエラー原因を他人に言葉で説明しようとするとパニックを起こしタジタジになる。
emotion: 「なぜ伝わらないのか」という焦燥感と、自分のコミュニケーション能力の低さへの絶望。
role: '葵あおい との「翻訳百合」イベントを発生させるトリガー。
「*あの…その…」「…伝わらない…！」

### C 混乱・言語化障害（改善後）
range: @03 ..
personality: 言語化の訓練が実り、詰まりながらも最後まで説明を完遂できるようになった。
emotion: 焦燥は残るが、伝わったときの達成感を知っている。
role: 成長を読者に示す指標。
「*あの…でも、つまり*」

### D あおい依存・感傷
range: @01 ..
personality: '葵あおい に対して感情が溢れ、半泣きで抱きついたり離れなくなったりする極めて重い感情を見せる。
emotion: 自分を理解し世界と繋いでくれるあおいへの絶対的帰依と、手放したくないという執着。
role: 主人公との絆の深さを読者に印象付けるメインヒロイン。
「あおいちゃん…*」「*いなくなったら…」

## relations
'葵あおい: 魂のパートナー。あおい無しでは他者と円滑な開発ができないが、あおいとなら最強になれる。
'瀬戸晴香: 技術の師弟関係。ミスを未然に防ぐ「絶対失敗しない設計哲学」を学ぶ。

## notes
口癖サンプル:
あおいちゃんが行くなら私も……。
処理速度が足りないのはアルゴリズムの効率が悪いからで……あ、あの……その……。
動けばいいだけじゃダメなんです。絶対に途中で落ちないシステムにしないと……！
"#;

// ============================================================ タグ

fn range_text(range: &Range<usize>) -> String {
    format!("{}..{}", range.start, range.end)
}

fn indices(found: &[usize]) -> String {
    if found.is_empty() {
        return "-".to_owned();
    }
    let parts: Vec<String> = found.iter().map(usize::to_string).collect();
    parts.join(",")
}

fn join(parts: impl Iterator<Item = String>) -> String {
    parts.collect::<Vec<String>>().join("\n")
}

fn comment_tag(comment: Option<&Comment<'_>>) -> String {
    match comment {
        Some(found) => format!("{}@{}", found.text, range_text(&found.range)),
        None => String::new(),
    }
}

fn pattern_texts(found: &characters_md::PatternLine<'_>) -> String {
    let parts: Vec<String> = found
        .patterns
        .iter()
        .map(|pattern| format!("{}@{}", pattern.text, range_text(&pattern.range)))
        .collect();
    parts.join(",")
}

fn line_tag(line: &Line<'_>) -> String {
    match line {
        Line::Heading(found) => format!(
            "h{}[{}|@{}|c{}]",
            found.level,
            found.text,
            found.text_at,
            comment_tag(found.comment.as_ref())
        ),
        Line::Field(found) => {
            let mut out = format!("field[{}|{:?}|v{}", found.key_text, found.key, found.value);
            if !found.extra.is_empty() {
                out.push_str("|x");
                out.push_str(&found.extra.join("\u{23ce}"));
            }
            out.push(']');
            out
        }
        Line::Continuation(found) => format!("cont({})[{}]", found.indent, found.text),
        Line::Bullet(found) => format!("bullet[{}|@{}]", found.text, found.marker_at),
        Line::Pattern(found) => format!("pattern[{}]", pattern_texts(found)),
        Line::Text(found) => format!("text[{found}]"),
        Line::Comment(found) => format!("comment[{}]", comment_tag(Some(&found.comment))),
        Line::Blank(found) => format!("blank[{found}]"),
    }
}

fn mode_range_tag(found: Option<&ModeRange<'_>>) -> String {
    match found {
        Some(found) => format!("{}|ok{}|l{}", found.text, found.valid, found.line),
        None => "-".to_owned(),
    }
}

fn mode_tag(found: &ModeDef<'_>) -> String {
    format!(
        "M[{}|{}|ok{}|{}|f{}|p{}|free{}|r{}]",
        found.id,
        found.label,
        found.valid_id,
        range_text(&found.span),
        indices(&found.fields),
        indices(&found.patterns),
        indices(&found.free),
        mode_range_tag(found.range.as_ref())
    )
}

fn relation_tag(found: &Relation<'_>) -> String {
    format!(
        "R[{}|n{}|{}|l{}]",
        found.name,
        found.note,
        range_text(&found.range),
        found.line
    )
}

fn section_tag(found: &Section<'_>) -> String {
    format!(
        "S[{:?}|{}|{}|f{}|b{}|p{}|free{}|r{}|m{}]",
        found.key,
        found.key_text,
        range_text(&found.span),
        indices(&found.fields),
        indices(&found.bullets),
        indices(&found.patterns),
        indices(&found.free),
        indices(
            &found
                .relations
                .iter()
                .map(|found| found.line)
                .collect::<Vec<usize>>()
        ),
        found.modes.len()
    )
}

fn character_tag(found: &Character<'_>) -> String {
    format!(
        "C[{}|@{}|{}|f{}|free{}|s{}]",
        found.name,
        found.name_at,
        range_text(&found.span),
        indices(&found.fields),
        indices(&found.free),
        found.sections.len()
    )
}

fn characters_of(parsed: &Characters<'_>) -> String {
    let mut parts: Vec<String> = parsed.characters.iter().map(character_tag).collect();
    for found in &parsed.characters {
        for section in &found.sections {
            parts.push(section_tag(section));
            parts.extend(section.modes.iter().map(mode_tag));
            parts.extend(section.relations.iter().map(relation_tag));
        }
    }
    parts.join("\n")
}

fn arg_tag(found: &Arg) -> String {
    match found {
        Arg::Text(value) => value.clone(),
        Arg::Number(value) => value.to_string(),
    }
}

fn diag_tag(found: &Diagnostic) -> String {
    let range = match &found.range {
        Some(found) => range_text(found),
        None => "-".to_owned(),
    };
    if found.args.is_empty() {
        return format!("{}@{}", found.code.as_str(), range);
    }
    let args: Vec<String> = found.args.iter().map(arg_tag).collect();
    format!("{}@{}({})", found.code.as_str(), range, args.join(","))
}

// ============================================================ 入口ヘルパ

fn lines_tag(text: &str) -> String {
    join(characters_md::parse(text).lines.iter().map(line_tag))
}

fn line_tag_of(text: &str, index: usize) -> String {
    line_tag(&characters_md::parse(text).lines[index])
}

fn characters_tag(text: &str) -> String {
    characters_of(&characters_md::parse(text))
}

fn diags_tag(text: &str) -> String {
    join(characters_md::parse(text).diagnostics.iter().map(diag_tag))
}

fn preamble_tag(text: &str) -> String {
    let parsed = characters_md::parse(text);
    format!(
        "P{}|f{}",
        range_text(&parsed.preamble.span),
        indices(&parsed.preamble.fields)
    )
}

fn keys_of(text: &str, index: usize, at: usize, key: FieldKey) -> String {
    let parsed = characters_md::parse(text);
    let fields = parsed.section_fields(index, at, key);
    let parts: Vec<String> = fields.iter().map(|found| found.value.to_owned()).collect();
    parts.join(",")
}

// ============================================================ 行の分類

#[test]
fn lines_are_classified_one_by_one() {
    // §5.5.1 の 4 種（見出し・フィールド行・箇条書き・鉤括弧行）＋継続行・自由記述・
    // コメント・空行が 1 行ずつどう読まれるか
    let text = "# 恵\nreading: おんだめぐみ\n  続き\n- 項目\n「*だよ」\n自由記述\n// コメント\n\n";
    assert_eq!(
        lines_tag(text),
        [
            "h1[恵|@2|c]",
            "field[reading|Reading|vおんだめぐみ|x続き]",
            "cont(2)[続き]",
            "bullet[項目|@0]",
            "pattern[*だよ@3..10]",
            "text[自由記述]",
            "comment[ コメント@0..15]",
            "blank[]",
            "blank[]",
        ]
        .join("\n")
    );
    assert_eq!(characters_md::parse(text).to_text(), text);
}

// ============================================================ §5.5.7 の例（受け入れテスト）

#[test]
fn the_spec_example_has_one_character_with_five_sections() {
    let parsed = characters_md::parse(SPEC_EXAMPLE);
    assert_eq!(parsed.characters.len(), 1);
    let found = &parsed.characters[0];
    assert_eq!(found.name, "恩田恵");
    assert_eq!(found.heading, 0);
    assert_eq!(found.span, 0..parsed.lines.len());
    // §5.5.3 のキャラ直下フィールド（reading / alias / role / default-mode / color）
    assert_eq!(found.fields, vec![1, 2, 3, 4, 5]);
    let keys: Vec<String> = found
        .sections
        .iter()
        .map(|section| section.key_text.to_owned())
        .collect();
    assert_eq!(
        keys,
        vec!["attributes", "tone", "modes", "relations", "notes"]
    );
}

#[test]
fn the_spec_example_reads_the_five_modes_in_order() {
    let parsed = characters_md::parse(SPEC_EXAMPLE);
    let modes = &parsed.characters[0]
        .section(SectionKey::Modes)
        .unwrap()
        .modes;
    let ids: Vec<String> = modes
        .iter()
        .map(|mode| format!("{}|{}", mode.id, mode.label))
        .collect();
    assert_eq!(
        ids,
        vec![
            "A|日常",
            "B|開発・実装",
            "C|混乱・言語化障害",
            "C|混乱・言語化障害（改善後）",
            "D|あおい依存・感傷"
        ]
    );
    // §5.5.4「同一 mode_id を複数回定義してよい。ただし range が重ならないこと」
    assert!(modes.iter().all(|mode| mode.valid_id));
    assert_eq!(
        modes
            .iter()
            .map(|mode| mode.range.as_ref().unwrap().text)
            .collect::<Vec<&str>>(),
        vec!["@01 ..", "@01 ..", "@01 .. @02", "@03 ..", "@01 .."]
    );
    assert!(modes.iter().all(|mode| mode.range.as_ref().unwrap().valid));
}

#[test]
fn the_spec_example_does_not_report_overlapping_ranges() {
    // `### C` は 2 回あるが `@01 .. @02` と `@03 ..` は重ならない（§5.5.7 の注）。
    // 出る診断は `color:` の W011 だけ
    let at = byte_offset(SPEC_EXAMPLE, 5);
    assert_eq!(
        diags_tag(SPEC_EXAMPLE),
        format!("W011@{}..{}(color)", at, at + "color".len())
    );
}

#[test]
fn the_spec_example_warns_only_about_the_untabled_color_key() {
    // §5.5.3 は `color:` を定義しているが §5.0.6 の表に無い（PLANS.md §9.1 A-15）。
    // 表を正本として W011 を出し、値は改変せずに保持する
    let parsed = characters_md::parse(SPEC_EXAMPLE);
    assert_eq!(parsed.diagnostics.len(), 1);
    let color = &parsed.characters[0].fields[4];
    let field = parsed.field(*color).unwrap();
    assert_eq!(field.key_text, "color");
    assert_eq!(field.key, FieldKey::Unknown);
    assert_eq!(field.value, "#8fb8ff");
}

#[test]
fn the_spec_example_keeps_japanese_free_text_out_of_the_field_table() {
    // §5.5.7 の `## attributes` の中身は `属性: …` のように日本語キー＋半角コロンだが、
    // §5.0.5 のキーの形（/[a-z][a-z0-9-]{0,23}/）にも別名表にも無いので**自由記述**
    let parsed = characters_md::parse(SPEC_EXAMPLE);
    let section = parsed.characters[0]
        .section(SectionKey::Attributes)
        .unwrap();
    assert!(section.fields.is_empty());
    assert_eq!(section.free.len(), 4);
    assert!(parsed.lines[section.free[0]].raw().starts_with("属性"));
}

#[test]
fn the_spec_example_reads_the_role_patterns_per_scope() {
    let parsed = characters_md::parse(SPEC_EXAMPLE);
    let found = &parsed.characters[0];
    // §5.5.5「`## tone` 直下に置けばキャラ共通」
    let tone = found.section(SectionKey::Tone).unwrap();
    assert_eq!(tone.patterns.len(), 1);
    let line = &parsed.lines[tone.patterns[0]];
    let Line::Pattern(pattern_line) = line else {
        panic!("tone の行はパターン行");
    };
    let texts: Vec<&str> = pattern_line
        .patterns
        .iter()
        .map(|pattern| pattern.text)
        .collect();
    assert_eq!(texts, vec!["*で…", "*です…", "…あの…*"]);
    // §5.5.5「`### <mode>` 直下に置けばそのモード専用」
    let modes = &found.section(SectionKey::Modes).unwrap().modes;
    let counts: Vec<usize> = modes.iter().map(|mode| mode.patterns.len()).collect();
    assert_eq!(counts, vec![1, 1, 1, 1, 1]);
    let first = pattern_of(&parsed, modes[0].patterns[0]);
    let texts: Vec<&str> = first.patterns.iter().map(|pattern| pattern.text).collect();
    assert_eq!(texts, vec!["*だよ…", "えへへ、*"]);
}

#[test]
fn the_spec_example_reads_the_relations() {
    let parsed = characters_md::parse(SPEC_EXAMPLE);
    let section = parsed.characters[0].section(SectionKey::Relations).unwrap();
    let tags: Vec<String> = section.relations.iter().map(relation_tag).collect();
    assert_eq!(tags.len(), 2);
    assert!(tags[0].starts_with("R[葵あおい|n魂のパートナー。"));
    assert!(tags[1].starts_with("R[瀬戸晴香|n技術の師弟関係。"));
}

#[test]
fn the_spec_example_round_trips_byte_for_byte() {
    let parsed = characters_md::parse(SPEC_EXAMPLE);
    assert_eq!(parsed.to_text(), SPEC_EXAMPLE);
}

#[test]
fn the_spec_example_spans_do_not_overlap() {
    let parsed = characters_md::parse(SPEC_EXAMPLE);
    let found = &parsed.characters[0];
    let modes = &found.section(SectionKey::Modes).unwrap().modes;
    // モードの span は次の H3 まで（最後はセクションの終わり）
    assert_eq!(modes[0].span, 18..25);
    assert_eq!(modes[3].span, 39..46);
    assert_eq!(modes[4].span, 46..53);
    // セクションの span は次の H2 まで
    assert_eq!(found.sections[0].span, 7..13);
    assert_eq!(found.sections[2].span, 16..53);
    assert_eq!(found.sections[4].span, 57..parsed.lines.len());
}

// ============================================================ 見出し（§5.5.1 の 1）

#[test]
fn headings_carry_level_text_and_position() {
    let text = "# 恩田恵\n## modes\n### A 日常\n";
    assert_eq!(line_tag_of(text, 0), "h1[恩田恵|@2|c]");
    assert_eq!(line_tag_of(text, 1), "h2[modes|@3|c]");
    assert_eq!(line_tag_of(text, 2), "h3[A 日常|@4|c]");
}

#[test]
fn a_heading_takes_a_trailing_comment() {
    // §5.0.2 の行末コメント。見出しのテキストからは除かれる
    let text = "# megumi // memo\n";
    assert_eq!(line_tag_of(text, 0), "h1[megumi|@2|c memo@9..16]");
}

#[test]
fn a_heading_without_a_space_is_free_text() {
    // §5.5.2 は `"# "`（空白込み）。`#恩田恵` は 4 種に当てはまらないので自由記述
    assert_eq!(line_tag_of("#恩田恵\n", 0), "text[#恩田恵]");
    assert_eq!(characters_tag("#恩田恵\n"), "");
}

#[test]
fn a_fourth_level_heading_is_free_text() {
    // §5.5.1 は H1 / H2 / H3 だけ
    assert_eq!(line_tag_of("#### 深い\n", 0), "text[#### 深い]");
}

#[test]
fn a_bare_hash_is_a_heading_with_an_empty_text() {
    assert_eq!(line_tag_of("#\n", 0), "h1[|@1|c]");
}

// ============================================================ 箇条書き（§5.5.1 の 3）

#[test]
fn bullets_are_read_in_a_section_body() {
    let text = "# 恵\n## notes\n- 一つ\n- 二つ\n";
    assert_eq!(line_tag_of(text, 2), "bullet[一つ|@0]");
    let parsed = characters_md::parse(text);
    let section = &parsed.characters[0].sections[0];
    assert_eq!(section.bullets, vec![2, 3]);
}

#[test]
fn an_indented_bullet_keeps_its_position() {
    // §5.0.5 の継続行（インデント 2 以上）が箇条書きより先。字句器の規則が優先する
    assert_eq!(line_tag_of("  - 一つ\n", 0), "cont(2)[- 一つ]");
    // インデントの無い箇条書きだけが §5.5.1 の 3
    assert_eq!(line_tag_of(" - 一つ\n", 0), "bullet[一つ|@1]");
}

#[test]
fn a_dash_followed_by_a_value_is_not_a_bullet() {
    // §5.0.2 が `#1 位` を誤認しないのと同じ理由
    assert_eq!(line_tag_of("-5\n", 0), "text[-5]");
}

#[test]
fn a_bullet_outside_a_section_is_not_in_the_tree() {
    // §5.5.2 の `character` に箇条書きは無い（行は保持される）
    let text = "# 恵\n- 一つ\n";
    assert_eq!(characters_tag(text), "C[恵|@2|0..3|f-|free-|s0]");
    assert_eq!(line_tag_of(text, 1), "bullet[一つ|@0]");
    assert_eq!(characters_md::parse(text).to_text(), text);
}

// ============================================================ 役割語パターン行（§5.5.5）

#[test]
fn patterns_keep_byte_ranges() {
    // `「` も `」` も 3 byte
    assert_eq!(line_tag_of("「*だよ」\n", 0), "pattern[*だよ@3..10]");
}

#[test]
fn an_unclosed_bracket_is_free_text() {
    assert_eq!(line_tag_of("「*だよ\n", 0), "text[「*だよ]");
}

#[test]
fn trailing_junk_after_the_last_pattern_is_free_text() {
    // §5.5.2 `pattern_line` は `「…」` の並びだけ。残りは自由記述として保持する
    assert_eq!(line_tag_of("「*だよ」 続き\n", 0), "text[「*だよ」 続き]");
}

#[test]
fn an_empty_pattern_is_allowed() {
    assert_eq!(line_tag_of("「」\n", 0), "pattern[@3..3]");
}

// ============================================================ フィールド行（§5.5.1 の 2）

#[test]
fn known_markdown_keys_are_read_without_a_warning() {
    let text = "# 恵\nreading: おんだめぐみ\nalias: めぐみ, 恵\ndefault-mode: A\n";
    assert_eq!(diags_tag(text), "");
    let parsed = characters_md::parse(text);
    assert_eq!(parsed.characters[0].fields, vec![1, 2, 3]);
    let fields = parsed.character_fields(0, FieldKey::DefaultMode);
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].value, "A");
    // セクション経由では引けない（キャラ直下のフィールドだから）
    assert!(keys_of(text, 0, 0, FieldKey::Reading).is_empty());
}

#[test]
fn unknown_keys_warn_and_keep_their_values() {
    // `address:` は ②（追加仕様 8）、`icon:` は ②（追加仕様 24）。
    // PLANS.md §5.3 作業項目 3「実装しないが未知フィールドとして保持されること」
    let text = "# 恵\naddress: '葵あおい => あおいちゃん\nicon: assets/icons/megumi.png\n";
    let address = byte_offset(text, 1);
    let icon = byte_offset(text, 2);
    assert_eq!(
        diags_tag(text),
        format!(
            "W011@{}..{}(address)\nW011@{}..{}(icon)",
            address,
            address + "address".len(),
            icon,
            icon + "icon".len()
        )
    );
    let parsed = characters_md::parse(text);
    let address = parsed.field(parsed.characters[0].fields[0]).unwrap();
    assert_eq!(address.value, "'葵あおい => あおいちゃん");
    let icon = parsed.field(parsed.characters[0].fields[1]).unwrap();
    assert_eq!(icon.value, "assets/icons/megumi.png");
    assert_eq!(parsed.to_text(), text);
}

#[test]
fn a_continuation_line_joins_the_field_value() {
    let text = "# 恵\n## notes\nnote: 一行目\n  二行目\n    字下げは残る\n";
    assert_eq!(line_tag_of(text, 3), "cont(2)[二行目]");
    let parsed = characters_md::parse(text);
    let section = &parsed.characters[0].sections[0];
    let field = parsed.field(section.fields[0]).unwrap();
    assert_eq!(field.extra, vec!["二行目", "  字下げは残る"]);
    assert_eq!(field.full_value(), "一行目\n二行目\n  字下げは残る");
}

#[test]
fn a_japanese_key_is_free_text_not_a_field() {
    // §5.0.6 の別名（`口調` 等）ならフィールド、無ければ自由記述（§5.0.5）
    assert_eq!(
        line_tag_of("口調: おとなしい\n", 0),
        "field[口調|Tone|vおとなしい]"
    );
    assert_eq!(line_tag_of("属性: A高校\n", 0), "text[属性: A高校]");
}

// ============================================================ セクションキー（§5.5.2）

#[test]
fn the_five_section_keys_are_recognized() {
    let text = "# 恵\n## modes\n## attributes\n## tone\n## relations\n## notes\n";
    let parsed = characters_md::parse(text);
    let keys: Vec<SectionKey> = parsed.characters[0]
        .sections
        .iter()
        .map(|section| section.key)
        .collect();
    assert_eq!(
        keys,
        vec![
            SectionKey::Modes,
            SectionKey::Attributes,
            SectionKey::Tone,
            SectionKey::Relations,
            SectionKey::Notes
        ]
    );
}

#[test]
fn section_keys_accept_the_alias_table() {
    // §5.5.2「§5.0.6 の別名表に準拠」。別名は字句器の表から引く（core に日本語リテラルを置かない）
    let text = "# 恵\n## 口調\n## 関係\n## メモ\n## モード\n## 性格\n## 独自\n";
    let parsed = characters_md::parse(text);
    let keys: Vec<SectionKey> = parsed.characters[0]
        .sections
        .iter()
        .map(|section| section.key)
        .collect();
    assert_eq!(
        keys,
        vec![
            SectionKey::Tone,
            SectionKey::Relations,
            SectionKey::Notes,
            SectionKey::Modes,
            SectionKey::Other,
            SectionKey::Other
        ]
    );
    // `Other` でも書かれたままの名前は残る
    assert_eq!(parsed.characters[0].sections[4].key_text, "性格");
}

// ============================================================ モード（§5.5.4）

#[test]
fn a_mode_without_a_range_covers_everything() {
    // §5.5.4「`range` 省略時は全範囲（`@00 ..`）」
    let text = "# 恵\n## modes\n### A 日常\n### A 後半\n";
    let parsed = characters_md::parse(text);
    let modes = &parsed.characters[0].sections[0].modes;
    assert!(modes[0].range.is_none());
    // 全範囲どうしは必ず重なるので 2 件目に E017（位置は 2 件目の見出しの `mode_id`）
    let at = byte_offset(text, 3) + 4;
    assert_eq!(diags_tag(text), format!("E017@{}..{}(A)", at, at + 1));
}

#[test]
fn overlapping_ranges_for_the_same_id_report_e017() {
    let text = "# 恵\n## modes\n### A 前半\nrange: @01 .. @02\n### A 後半\nrange: @02 .. @03\n";
    let parsed = characters_md::parse(text);
    let modes = &parsed.characters[0].sections[0].modes;
    assert_eq!(modes.len(), 2);
    // 位置は 2 件目の `range` の値
    let at = byte_offset(text, 5) + parsed.field(modes[1].fields[0]).unwrap().value_at;
    assert_eq!(
        diags_tag(text),
        format!("E017@{}..{}(A)", at, at + "@02 .. @03".len())
    );
}

#[test]
fn disjoint_ranges_for_the_same_id_are_allowed() {
    let text = "# 恵\n## modes\n### C 前半\nrange: @01 .. @02\n### C 後半\nrange: @03 ..\n";
    assert_eq!(diags_tag(text), "");
}

#[test]
fn the_same_range_under_different_ids_is_allowed() {
    let text = "# 恵\n## modes\n### A 日常\nrange: @01 ..\n### B 開発\nrange: @01 ..\n";
    assert_eq!(diags_tag(text), "");
}

#[test]
fn ranges_of_different_characters_do_not_collide() {
    // §5.5.4 の重複は**同一キャラクタ内**の話
    let text = "# 恵\n## modes\n### A 日常\nrange: @01 ..\n# あおい\n## modes\n### A 平静\nrange: @01 ..\n";
    assert_eq!(diags_tag(text), "");
    assert_eq!(characters_md::parse(text).characters.len(), 2);
}

#[test]
fn a_reversed_range_reports_e016() {
    let text = "# 恵\n## modes\n### A 日常\nrange: @03 .. @01\n";
    let parsed = characters_md::parse(text);
    let modes = &parsed.characters[0].sections[0].modes;
    assert!(!modes[0].range.as_ref().unwrap().valid);
    let at = byte_offset(text, 3) + parsed.field(modes[0].fields[0]).unwrap().value_at;
    assert_eq!(
        diags_tag(text),
        format!("E016@{}..{}", at, at + "@03 .. @01".len())
    );
}

#[test]
fn an_unparsable_range_reports_e002_and_is_not_compared() {
    let text = "# 恵\n## modes\n### A 前半\nrange: 01 ..\n### A 後半\nrange: @01 ..\n";
    let parsed = characters_md::parse(text);
    let modes = &parsed.characters[0].sections[0].modes;
    assert!(!modes[0].range.as_ref().unwrap().valid);
    // 形が読めないものは全範囲扱いにしない（E002 だけを出して比較はしない）
    assert_eq!(diags_tag(text).lines().count(), 1);
    assert!(diags_tag(text).starts_with("E002@"));
}

#[test]
fn an_empty_range_value_is_treated_as_omitted() {
    // `range:` だけの行は「範囲を書かなかった」＝全範囲（§5.5.4）
    let text = "# 恵\n## modes\n### A 日常\nrange:\n";
    let parsed = characters_md::parse(text);
    let modes = &parsed.characters[0].sections[0].modes;
    assert_eq!(modes[0].range.as_ref().unwrap().text, "");
    assert_eq!(diags_tag(text), "");
}

#[test]
fn a_range_expression_may_list_several_intervals() {
    // §5.1 の `range_expr` はカンマ区切りの和集合
    let text = "# 恵\n## modes\n### A 日常\nrange: @01 .. @02, @05\n";
    let parsed = characters_md::parse(text);
    let modes = &parsed.characters[0].sections[0].modes;
    assert!(modes[0].range.as_ref().unwrap().valid);
    assert_eq!(modes[0].range.as_ref().unwrap().expr().unwrap().len(), 2);
    assert_eq!(diags_tag(text), "");
}

#[test]
fn an_invalid_mode_id_is_flagged_without_a_diagnostic() {
    // §5.5.2 `mode_id = /[A-Za-z][A-Za-z0-9_]{0,15}/`。§7 に対応コードが無いので診断しない（B-39）
    let text = "# 恵\n## modes\n### 1abc\n### toolongtoolongtool\n### ok_2\n";
    let parsed = characters_md::parse(text);
    let modes = &parsed.characters[0].sections[0].modes;
    let flags: Vec<bool> = modes.iter().map(|mode| mode.valid_id).collect();
    assert_eq!(flags, vec![false, false, true]);
    assert_eq!(diags_tag(text), "");
}

#[test]
fn a_mode_outside_a_section_is_not_in_the_tree() {
    // §5.5.2 の `character` に `mode_def` は無いのでモードは作られない（行は保持される）。
    // その下のフィールド行は**キャラクタ直下**に入って値を失わない
    let text = "# 恵\n### A 日常\nrange: @01 ..\n";
    assert_eq!(characters_tag(text), "C[恵|@2|0..4|f2|free-|s0]");
    assert_eq!(diags_tag(text), "");
    assert_eq!(characters_md::parse(text).to_text(), text);
}

// ============================================================ relations（§5.5.6）

#[test]
fn a_relation_line_splits_the_ref_and_the_note() {
    let text = "# 恵\n## relations\n'葵あおい: 魂のパートナー。\n";
    let parsed = characters_md::parse(text);
    let section = &parsed.characters[0].sections[0];
    assert_eq!(section.relations.len(), 1);
    let found = &section.relations[0];
    assert_eq!(found.name, "葵あおい");
    assert_eq!(found.note, "魂のパートナー。");
    assert_eq!(found.line, 2);
    assert_eq!(found.range, 0..13);
    // 行そのものは自由記述のまま（§5.5.1 の 4 種に無い）
    assert_eq!(section.free, vec![2]);
}

#[test]
fn a_relation_accepts_a_full_width_colon() {
    let text = "# 恵\n## relations\n'瀬戸晴香：技術の師弟関係。\n";
    let parsed = characters_md::parse(text);
    let found = &parsed.characters[0].sections[0].relations[0];
    assert_eq!(found.name, "瀬戸晴香");
    assert_eq!(found.note, "技術の師弟関係。");
}

#[test]
fn a_relation_without_a_note_keeps_an_empty_one() {
    let text = "# 恵\n## relations\n'葵あおい\n";
    let parsed = characters_md::parse(text);
    let found = &parsed.characters[0].sections[0].relations[0];
    assert_eq!(found.name, "葵あおい");
    assert_eq!(found.note, "");
}

#[test]
fn relations_are_read_only_inside_the_relations_section() {
    // §5.5.6 の形式が意味を持つのは `## relations` の中だけ
    let text = "# 恵\n## notes\n'葵あおい: 説明\n";
    let parsed = characters_md::parse(text);
    assert!(parsed.characters[0].sections[0].relations.is_empty());
    assert_eq!(parsed.characters[0].sections[0].free, vec![2]);
}

#[test]
fn a_line_without_the_ref_sigil_is_not_a_relation() {
    let text = "# 恵\n## relations\n葵あおい: 説明\n";
    let parsed = characters_md::parse(text);
    assert!(parsed.characters[0].sections[0].relations.is_empty());
}

// ============================================================ 前文と入れ子

#[test]
fn lines_before_the_first_h1_are_the_preamble() {
    let text = "// 配役表\nnote: 全体メモ\n# 恵\n";
    assert_eq!(preamble_tag(text), "P0..2|f1");
    // 末尾改行のあとにも空の 1 行があるので span はそこまでのびる
    assert_eq!(characters_md::parse(text).characters[0].span, 2..4);
}

#[test]
fn a_section_before_any_character_is_not_in_the_tree() {
    // 壊れた入力でもパニックしない（§5.3 完了条件 3）。行は前文として残る
    let text = "## modes\n### A 日常\n# 恵\n";
    let parsed = characters_md::parse(text);
    assert_eq!(parsed.preamble.span, 0..2);
    assert_eq!(parsed.characters.len(), 1);
    assert_eq!(parsed.characters[0].span, 2..4);
    assert_eq!(parsed.to_text(), text);
}

#[test]
fn a_second_character_closes_the_first_one() {
    let text = "# 恵\n## modes\n### A 日常\n# あおい\n## tone\n";
    let parsed = characters_md::parse(text);
    assert_eq!(parsed.characters[0].span, 0..3);
    assert_eq!(parsed.characters[1].span, 3..6);
    assert_eq!(parsed.characters[0].sections[0].span, 1..3);
    assert_eq!(parsed.characters[0].sections[0].modes[0].span, 2..3);
}

#[test]
fn comments_and_blank_lines_stay_out_of_the_tree() {
    let text = "# 恵\n\n// メモ\n\n## notes\n";
    let parsed = characters_md::parse(text);
    assert_eq!(parsed.characters[0].free.len(), 0);
    assert_eq!(parsed.characters[0].sections[0].free.len(), 0);
    assert_eq!(line_tag_of(text, 1), "blank[]");
    assert_eq!(line_tag_of(text, 2), "comment[ メモ@0..9]");
    assert_eq!(line_tag_of(text, 3), "blank[]");
}

#[test]
fn a_whitespace_only_line_is_blank() {
    // B-34（空白だけの行も Blank）
    assert_eq!(line_tag_of("   \n", 0), "blank[   ]");
}

#[test]
fn free_text_goes_to_the_innermost_open_container() {
    let text = "# 恵\n直下\n## notes\nセクション内\n### A 日常\n## modes\n### B 開発\nモード内\n";
    let parsed = characters_md::parse(text);
    let found = &parsed.characters[0];
    assert_eq!(found.free, vec![1]);
    assert_eq!(found.sections[0].free, vec![3]);
    assert_eq!(found.sections[1].modes[0].free, vec![7]);
}

// ============================================================ 壊れた入力（完了条件 3）

#[test]
fn broken_input_does_not_panic() {
    let cases = [
        "",
        "\n",
        "#",
        "###",
        "# \n## \n### \n",
        "'\n",
        "'参照:\n",
        "「\n",
        "「」「\n",
        "-\n",
        "- \n",
        "range: @\n",
        "# 恵\n## modes\n### A\nrange: @01 ..\nrange: @02 ..\n",
        "# 恵\n# 恵\n",
        "key:\n  \n    x\n",
        "//\n",
    ];
    for text in cases {
        let parsed = characters_md::parse(text);
        assert_eq!(parsed.to_text(), text, "round-trip: {text:?}");
        // 木を作れること（パニックしないこと）だけを確認する
        let _ = characters_of(&parsed);
        let _ = diags_tag(text);
    }
}

#[test]
fn the_same_mode_id_twice_without_a_section_does_not_panic() {
    let text = "### A\n### A\n";
    let parsed = characters_md::parse(text);
    assert!(parsed.characters.is_empty());
    assert_eq!(parsed.to_text(), text);
}

#[test]
fn a_character_with_nothing_after_it_is_kept() {
    // 末尾改行の無いファイル（§5.0.4）
    let parsed = characters_md::parse("# 恵");
    assert_eq!(parsed.lines.len(), 1);
    assert_eq!(parsed.characters.len(), 1);
    assert_eq!(parsed.characters[0].span, 0..1);
    assert_eq!(parsed.characters[0].name, "恵");
}

// ============================================================ 補助

/// `text` の `line` 行目の先頭の byte 位置。
fn byte_offset(text: &str, line: usize) -> usize {
    text.split('\n')
        .take(line)
        .map(|found| found.len() + 1)
        .sum()
}

/// 行を 1 本だけ読む（パターン行）。
fn pattern_of<'a>(parsed: &'a Characters<'a>, index: usize) -> &'a characters_md::PatternLine<'a> {
    match &parsed.lines[index] {
        Line::Pattern(found) => found,
        _ => panic!("パターン行ではない"),
    }
}
