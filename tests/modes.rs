//! `core::resolve::modes`（§6.4 モード解決）のテスト。
//!
//! `PLANS.md` §5.4 の作業項目 2 と完了条件 3（§5.5.7 の `### C` の 2 変種が `@01` と `@03` で
//! **別々に選ばれる**）に対応する。8 段の優先順位（§6.4.3）は、1 段ずつ「上を抑えたときに
//! 次が出る」形で固定する。
//!
//! 入力には**仕様書のコード例をそのまま**使う（§5.5.7 の恩田恵、§5.3.3 / §5.3.4 の A 形式、
//! §5.4 の B 形式、§5.2 の C 形式。`PLANS.md` §6.3「仕様が正本であることを機械的に担保する」）。
//! プロットの `cast:` / `mode:` に出てくる名前だけ、§5.5.7 の例に合わせるために差し替えてある。
//!
//! 位置の検査は**範囲の数値を書かず、その範囲で原文を切って比べる**（CJK は 1 字 3 byte で
//! 数え間違いの元になるため。切れた文字列が仕様書のトークンそのものなので読んで分かる）。

use std::ops::Range;

use novnow::core::diagnostics::{Arg, DiagnosticCode};
use novnow::core::id::{Digits, SceneId};
use novnow::core::model::role_pattern::RolePattern;
use novnow::core::resolve::dictionary::{self, Dictionary};
use novnow::core::resolve::modes::{self, Assignment, File, Input, Step, Target};
use novnow::core::syntax::characters_md::{self, Characters, SectionKey};
use novnow::core::syntax::contents::{self, Line};
use novnow::core::syntax::plot_episode::{self, EpisodePlot};
use novnow::core::syntax::plot_outline::{self, Outline};
use novnow::core::syntax::{materials_index, words_md};

// ============================================================ 入力（仕様書の例）

/// §5.5.7 の完全な例（恩田恵）。`## relations` が解決できるように 2 人を足してある。
///
/// **`### C` が 2 回現れる**のがこの例のポイント（§6.4.5 の変種選択、完了条件 3）。
const ONDA_MD: &str = "# 恩田恵
reading: おんだめぐみ
alias: めぐみ, 恵
role: メインヒロイン
default-mode: A
color: #8fb8ff

## attributes
属性: A高校サイエンス部員（高1→高2）。常にマイPCを持ち歩くテック女子。女子力は高い。

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

# 葵あおい
alias: あおい

# 瀬戸晴香
";

/// §6.4.4 / §6.4.5 の検査用の小さな `characters.md`（範囲が狭い・壊れている・無い変種）。
const EDGE_MD: &str = "# 恩田恵

## modes

### P 狭い
range: @02-03 .. @02-05
personality: 第 2 章の一部だけ。

### Q 壊れた範囲
range: @xx ..
personality: 範囲式として読めないので全範囲扱い（B-66）。

### R 範囲なし
personality: range を書かなければ全範囲（§5.5.4）。

### S 前
range: @09 .. @09

### S 後
range: @11 ..
";

/// `default-mode:` を持たない `characters.md`（§6.4.3 の段 8 → **W006**）。
const NO_DEFAULT_MD: &str = "# 恩田恵\nrole: メインヒロイン\n";

/// `default-mode:` に 3 つ以上（§6.4.4 の **E005**。この値は他の層が読まないのでここが引く）。
const DEFAULT_THREE_MD: &str =
    "# 恩田恵\ndefault-mode: A+B+C\n\n## modes\n\n### A 日常\n\n### B 開発\n\n### C 混乱\n";

/// `default-mode:` に 2 つ（B-65「`A+B` と書かれていても受け付ける」）。
const DEFAULT_TWO_MD: &str =
    "# 恩田恵\ndefault-mode: A+B\n\n## modes\n\n### A 日常\n\n### B 開発\n";

/// 2 人それぞれにモードがある `characters.md`。
///
/// **キャラの添字と行 index は違う**（[`Characters::character_of`] が取るのは行 index）ので、
/// 2 人目を正しく解決できることの検査に使う。
const TWO_MD: &str = "# 一人目\n\n## modes\n\n### A 一人目のA\n\n# 二人目\ndefault-mode: B\n\n## modes\n\n### B 二人目のB\n";

/// 役割語パターンがモード専用とキャラ共通で重なるときの和集合（§5.5.5 / §6.5.3）。
const DEDUPE_MD: &str = "# 恩田恵\ndefault-mode: A\n\n## tone\n「*で…」\n\n## modes\n\n### A 日常\n「*で…」「*だよ…」\n";

/// §5.4 の B 形式。preamble（段 4）・場面（段 3）・節（段 2）に 1 つずつ `mode:` を置く。
///
/// 見出し直下の `;` が **1 節目（空）**を作り、その次が **2 節目**になる（§5.4.1）。
const EPISODE: &str = "mode: '恩田恵:D\n@01-04-02 図書室の攻防\nmode: '恩田恵:B\n;\nmode: '恩田恵:A\n設計図を机に広げる。\n;\n";

/// preamble の `mode:` だけ（段 4 まで落ちる）。
const EPISODE_PREAMBLE: &str = "mode: '恩田恵:D\n@01-04-02 図書室の攻防\n設計図を机に広げる。\n;\n";

/// `mode:` をどこにも書かない B 形式（段 5 以降へ落ちる）。
const EPISODE_PLAIN: &str = "@01-04-02 図書室の攻防\n;\n設計図を机に広げる。\n;\n";

/// 節に 3 つ以上のモード（§6.4.4 の **E005**。波線は構文層が引く、B-67）。
const EPISODE_THREE: &str =
    "@01-04-02 図書室の攻防\n;\nmode: '恩田恵:A+B+C\n設計図を机に広げる。\n;\n";

/// 節に名前だけ（モードを書いていない）→ この段は**見つけなかった**扱い（B-65）。
const EPISODE_NAME_ONLY: &str =
    "@01-04-02 図書室の攻防\nmode: '恩田恵:B\n;\nmode: '恩田恵\n設計図を机に広げる。\n;\n";

/// `EDGE_MD` の各変種を節の `mode:` で指名する B 形式。
const EPISODE_P: &str = "@01-04-02 図書室の攻防\n;\nmode: '恩田恵:P\n本文。\n;\n";
const EPISODE_Q: &str = "@01-04-02 図書室の攻防\n;\nmode: '恩田恵:Q\n本文。\n;\n";
const EPISODE_R: &str = "@09-09-09 遠い場面\n;\nmode: '恩田恵:R\n本文。\n;\n";
const EPISODE_S: &str = "@01-04-02 図書室の攻防\n;\nmode: '恩田恵:S\n本文。\n;\n";
const EPISODE_Z: &str = "@01-04-02 図書室の攻防\n;\nmode: '恩田恵:Z\n本文。\n;\n";
const EPISODE_ZR: &str = "@01-04-02 図書室の攻防\n;\nmode: '恩田恵:Z+R\n本文。\n;\n";
const EPISODE_TWO: &str = "@01-04-02 攻防\n;\nmode: '二人目:B\n本文。\n;\n";

/// §5.3.4 の例（`plots/01.txt`）の `cast:` を恩田恵に差し替えたもの。
const CHAPTER: &str = "@01-04 図書室の密談
summary: 試験前の図書室で設計図を見せ、放課後の探索を約束させる。
goal: 読者に探索の動機とリスクを同時に渡す。
cast: '恩田恵:C
length: 5200
state: review
;
";

/// §5.3.4 の形（第 2 章）。`range: @01 .. @02` の**上端**がどちらの変種に入るかの検査用。
const CHAPTER_02: &str = "@02-01 補習と地図\nsummary: 期末試験の失敗を代償に、旧校舎の鍵を手に入れる。\ncast: '恩田恵:C\n;\n";

/// §5.3.4 の形（第 3 章）。`### C` の**2 つ目の変種**（`range: @03 ..`）が選ばれる。
const CHAPTER_03: &str =
    "@03-01 地下遺跡\nsummary: 言語化の訓練を積んだあとの探索。\ncast: '恩田恵:C\n;\n";

/// `cast:` に同じキャラを 2 回（§5.4.2 → **E005**。A 形式は他の層が見ないのでこの層が引く）。
const CHAPTER_TWICE: &str = "@01-04 図書室の密談\nsummary: 試験前の図書室で設計図を見せる。\ncast: '恩田恵:A '恩田恵:B\n;\n";

/// `cast:` の 1 トークンに 3 つ以上のモード（**E005** は辞書層の担当、B-67）。
const CHAPTER_THREE: &str =
    "@01-04 図書室の密談\nsummary: 試験前の図書室で設計図を見せる。\ncast: '恩田恵:A+B+C\n;\n";

/// §5.3.3 の例（`plots/all.txt`）の `cast:` を恩田恵に差し替えたもの。
const ALL: &str = "// 全体プロット: 章を 1 ブロック 1 章で書く。
title: 星屑の時計
summary: 古代錬金術を追う少年と幼馴染が、学院の地下遺跡の魔導具を巡って対立と和解を繰り返す。

@01 失われた刻印
summary: 地下遺跡に惹かれ、探索を始める導入章。
goal: 二人の関係性と'星屑の時計 の存在を読者に提示する。
cast: '恩田恵:B
state: fixed
;
";

/// §5.2 の C 形式。話者注釈（段 1）は**その行だけ**に効く（§6.4.2）。
const BODY: &str = "@01-04-02 図書室の攻防
「……あの、その…」@恩田恵:C
「動かしてみせます」@恩田恵:B
「静粛に」@葵あおい:X
地の文には注釈が無い。
「……あの、その…」@恩田恵
";

/// `materials/index.json`（§5.8.2 の形。中身は空でよい）。
const INDEX_JSON: &str = "{\n  \"schema_version\": 1,\n  \"next_id\": 138,\n  \"items\": []\n}\n";

// ============================================================ 小道具

/// 8 段の入力（§6.4.3）をまとめて持つ。
struct World {
    characters: Characters<'static>,
    episode: Option<EpisodePlot<'static>>,
    chapter: Option<Outline<'static>>,
    all: Option<Outline<'static>>,
    dictionary: Dictionary,
}

impl World {
    /// `characters.md` と 3 つのプロットから作る（`None` の段は飛ばされる、§6.4.3）。
    fn new(
        characters: &'static str,
        episode: Option<&'static str>,
        chapter: Option<&'static str>,
        all: Option<&'static str>,
    ) -> Self {
        let parsed = characters_md::parse(characters);
        let words = words_md::parse("");
        let index = materials_index::parse(INDEX_JSON);
        let dictionary = Dictionary::from_sources(&parsed, &words, &index).dictionary;
        Self {
            characters: parsed,
            episode: episode.map(plot_episode::parse),
            chapter: chapter.map(plot_outline::parse),
            all: all.map(plot_outline::parse),
            dictionary,
        }
    }

    /// §5.5.7 の恩田恵だけを登場させた世界（全体プロットは無し）。
    fn onda(episode: Option<&'static str>, chapter: Option<&'static str>) -> Self {
        Self::new(ONDA_MD, episode, chapter, None)
    }

    fn input(&self) -> Input<'_> {
        Input {
            characters: &self.characters,
            episode: self.episode.as_ref(),
            chapter: self.chapter.as_ref(),
            all: self.all.as_ref(),
            dictionary: &self.dictionary,
        }
    }

    /// 対象の節とキャラ（`Characters::characters` の添字）を指定して解決する。
    fn resolve(&self, scene: &str, character: usize) -> Assignment<'_> {
        let input = self.input();
        let target = Target {
            scene: scene_id(scene),
            character,
            speaker: None,
            line_start: 0,
        };
        modes::resolve(&input, &target)
    }
}

fn scene_id(text: &str) -> SceneId {
    SceneId::parse(text).unwrap_or_else(|error| panic!("{text}: {error:?}"))
}

fn file_name(file: File) -> &'static str {
    match file {
        File::Contents => "contents",
        File::Episode => "episode",
        File::Chapter => "chapter",
        File::All => "all",
        File::Characters => "characters",
    }
}

fn range_text(range: &Option<Range<usize>>) -> String {
    match range {
        Some(found) => format!("{}..{}", found.start, found.end),
        None => "-".to_string(),
    }
}

/// 解決結果を 1 行のタグにする（段・ID・採用した定義・診断）。
fn tag(assignment: &Assignment<'_>) -> String {
    let definitions = assignment
        .definitions
        .iter()
        .map(|found| format!("{}={}", found.id(), found.label()))
        .collect::<Vec<_>>()
        .join(",");
    let diagnostics = assignment
        .diagnostics
        .iter()
        .map(|found| {
            format!(
                "{}@{}",
                found.diagnostic.code.as_str(),
                file_name(found.file)
            )
        })
        .collect::<Vec<_>>()
        .join(" / ");
    format!(
        "step={} ids=[{}] defs=[{}] diags=[{}]",
        assignment.step.order(),
        assignment.ids.join("+"),
        definitions,
        diagnostics
    )
}

fn resolved(world: &World, scene: &str) -> String {
    tag(&world.resolve(scene, 0))
}

/// 本文（C 形式）の `line` 行目を対象にして解決する（話者注釈があれば段 1 になる）。
fn resolved_in_body(world: &World, line: usize, scene: &str, character: usize) -> String {
    let body = contents::parse(BODY);
    let Line::Text(text) = &body.lines[line] else {
        panic!("{line} 行目は本文行ではない");
    };
    let line_start = body.lines[..line]
        .iter()
        .map(|found| found.raw().len() + 1)
        .sum();
    let input = world.input();
    let target = Target {
        scene: scene_id(scene),
        character,
        speaker: text.speaker.as_ref(),
        line_start,
    };
    tag(&modes::resolve(&input, &target))
}

/// 診断の引数を `CODE[引数|引数]` の形にする。
fn args_tag(assignment: &Assignment<'_>) -> String {
    assignment
        .diagnostics
        .iter()
        .map(|found| {
            let args = found
                .diagnostic
                .args
                .iter()
                .map(|arg| match arg {
                    Arg::Text(text) => text.clone(),
                    Arg::Number(value) => value.to_string(),
                })
                .collect::<Vec<_>>()
                .join("|");
            format!("{}[{}]", found.diagnostic.code.as_str(), args)
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

/// 1 つ目の診断の範囲で `text` を切る（範囲が正しいトークンを指しているかの検査）。
fn slice_of<'a>(text: &'a str, assignment: &Assignment<'_>) -> &'a str {
    let range = assignment.diagnostics[0]
        .diagnostic
        .range
        .clone()
        .expect("範囲が無い");
    &text[range]
}

fn codes(assignment: &Assignment<'_>) -> Vec<DiagnosticCode> {
    assignment
        .diagnostics
        .iter()
        .map(|found| found.diagnostic.code)
        .collect()
}

fn patterns(list: &[RolePattern]) -> String {
    list.iter()
        .map(|found| format!("「{}」", found.source))
        .collect::<Vec<_>>()
        .join("")
}

// ============================================================ §6.4.3 の 8 段（優先順位）

#[test]
fn a_speaker_annotation_wins_over_every_plot() {
    let world = World::onda(Some(EPISODE), Some(CHAPTER));
    // 段 1（話者注釈）が段 2（節の `mode:`）に勝つ（§6.4.2「その台詞だけの一時上書き」）
    assert_eq!(
        resolved_in_body(&world, 1, "01-04-02-02", 0),
        "step=1 ids=[C] defs=[C=混乱・言語化障害] diags=[]"
    );
}

#[test]
fn a_speaker_annotation_reaches_only_its_own_line() {
    let world = World::onda(Some(EPISODE), Some(CHAPTER));
    // §6.4.3「その行のみ」。同じ節の別の行は別のモードを指す
    assert_eq!(
        resolved_in_body(&world, 2, "01-04-02-02", 0),
        "step=1 ids=[B] defs=[B=開発・実装] diags=[]"
    );
}

#[test]
fn a_line_without_an_annotation_uses_the_plot() {
    let world = World::onda(Some(EPISODE), Some(CHAPTER));
    assert_eq!(
        resolved_in_body(&world, 4, "01-04-02-02", 0),
        "step=2 ids=[A] defs=[A=日常] diags=[]"
    );
}

#[test]
fn an_annotation_of_another_character_is_skipped() {
    let world = World::onda(Some(EPISODE), Some(CHAPTER));
    // `@葵あおい:X` は恩田恵（添字 0）のモードではないので段 1 では「見つけた」にしない。
    // 他人の注釈を拾うと `X` は恩田恵に定義が無く E003 が出るので、それで分かる
    assert_eq!(
        resolved_in_body(&world, 3, "01-04-02-02", 0),
        "step=2 ids=[A] defs=[A=日常] diags=[]"
    );
}

#[test]
fn an_annotation_without_a_mode_falls_through() {
    let world = World::onda(Some(EPISODE), Some(CHAPTER));
    // `@恩田恵`（修飾子なし）は段 1 では「見つけなかった」扱い（B-65）
    assert_eq!(
        resolved_in_body(&world, 5, "01-04-02-02", 0),
        "step=2 ids=[A] defs=[A=日常] diags=[]"
    );
}

#[test]
fn the_section_mode_wins_over_the_scene_mode() {
    let world = World::onda(Some(EPISODE), None);
    assert_eq!(
        resolved(&world, "01-04-02-02"),
        "step=2 ids=[A] defs=[A=日常] diags=[]"
    );
}

#[test]
fn the_scene_mode_is_used_when_the_section_has_none() {
    let world = World::onda(Some(EPISODE), None);
    // EPISODE の 1 節目は空（見出し直下の `;`）なので段 3 まで落ちる
    assert_eq!(
        resolved(&world, "01-04-02-01"),
        "step=3 ids=[B] defs=[B=開発・実装] diags=[]"
    );
}

#[test]
fn a_section_number_that_is_not_written_falls_back_to_the_scene() {
    let world = World::onda(Some(EPISODE), None);
    assert_eq!(
        resolved(&world, "01-04-02-07"),
        "step=3 ids=[B] defs=[B=開発・実装] diags=[]"
    );
}

#[test]
fn the_preamble_mode_is_used_when_the_scene_has_none() {
    let world = World::onda(Some(EPISODE_PREAMBLE), None);
    assert_eq!(
        resolved(&world, "01-04-02-01"),
        "step=4 ids=[D] defs=[D=あおい依存・感傷] diags=[]"
    );
}

#[test]
fn a_reference_without_a_mode_does_not_stop_the_descent() {
    let world = World::onda(Some(EPISODE_NAME_ONLY), None);
    // 節の `mode: '恩田恵` はモードを書いていないので段 2 では「見つけなかった」（B-65）
    assert_eq!(
        resolved(&world, "01-04-02-02"),
        "step=3 ids=[B] defs=[B=開発・実装] diags=[]"
    );
}

#[test]
fn the_chapter_cast_is_used_when_the_episode_plot_has_nothing() {
    let world = World::onda(Some(EPISODE_PLAIN), Some(CHAPTER));
    assert_eq!(
        resolved(&world, "01-04-02-02"),
        "step=5 ids=[C] defs=[C=混乱・言語化障害] diags=[]"
    );
}

#[test]
fn the_all_cast_is_used_when_the_chapter_plot_has_nothing() {
    let world = World::new(ONDA_MD, Some(EPISODE_PLAIN), None, Some(ALL));
    assert_eq!(
        resolved(&world, "01-04-02-02"),
        "step=6 ids=[B] defs=[B=開発・実装] diags=[]"
    );
}

#[test]
fn the_chapter_cast_wins_over_the_all_cast() {
    let world = World::new(ONDA_MD, Some(EPISODE_PLAIN), Some(CHAPTER), Some(ALL));
    assert_eq!(
        resolved(&world, "01-04-02-02"),
        "step=5 ids=[C] defs=[C=混乱・言語化障害] diags=[]"
    );
}

#[test]
fn the_default_mode_is_the_last_resort() {
    let world = World::onda(Some(EPISODE_PLAIN), None);
    assert_eq!(
        resolved(&world, "01-04-02-02"),
        "step=7 ids=[A] defs=[A=日常] diags=[]"
    );
}

#[test]
fn files_that_are_not_open_are_skipped() {
    // プロットを 1 つも開いていない（段 2〜6 は飛ばす）
    let world = World::new(ONDA_MD, None, None, None);
    assert_eq!(
        resolved(&world, "01-04-02-02"),
        "step=7 ids=[A] defs=[A=日常] diags=[]"
    );
}

#[test]
fn an_empty_world_is_undetermined() {
    let world = World::new(NO_DEFAULT_MD, None, None, None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert_eq!(assignment.step, Step::Undetermined);
    assert!(assignment.step.is_undetermined());
    assert!(!assignment.is_determined());
    assert!(assignment.ids.is_empty());
    assert!(assignment.definitions.is_empty());
    assert_eq!(codes(&assignment), vec![DiagnosticCode::W006]);
}

// ============================================================ §6.4.5 変種（完了条件 3）

#[test]
fn the_first_variant_is_chosen_in_chapter_01() {
    let world = World::onda(Some(EPISODE_PLAIN), Some(CHAPTER));
    assert_eq!(
        resolved(&world, "01-04-02-02"),
        "step=5 ids=[C] defs=[C=混乱・言語化障害] diags=[]"
    );
}

#[test]
fn the_second_variant_is_chosen_in_chapter_03() {
    let world = World::onda(Some(EPISODE_PLAIN), Some(CHAPTER_03));
    // §5.5.7「`@01..@02` の節では前者、`@03..` の節では後者の性格が参照ペインに出る」
    assert_eq!(
        resolved(&world, "03-01-01-01"),
        "step=5 ids=[C] defs=[C=混乱・言語化障害（改善後）] diags=[]"
    );
}

#[test]
fn the_upper_bound_of_a_range_belongs_to_the_first_variant() {
    let world = World::onda(Some(EPISODE_PLAIN), Some(CHAPTER_02));
    assert_eq!(
        resolved(&world, "02-01-01-01"),
        "step=5 ids=[C] defs=[C=混乱・言語化障害] diags=[]"
    );
}

#[test]
fn a_mode_outside_every_variant_range_is_e004() {
    let world = World::new(EDGE_MD, Some(EPISODE_P), None, None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert_eq!(codes(&assignment), vec![DiagnosticCode::E004]);
    // §6.4.6「有効モードのみ表示する」ので実体は入らない。ID は書かれたまま残る
    assert!(assignment.definitions.is_empty());
    assert_eq!(assignment.ids, vec!["P"]);
    // §7 E004「モード {1} は {2} では使用できません（有効範囲: {3}）」
    assert_eq!(
        args_tag(&assignment),
        "E004[恩田恵|P|01-04-02-02|@02-03 .. @02-05]"
    );
    assert_eq!(slice_of(EPISODE_P, &assignment), "'恩田恵:P");
}

#[test]
fn e004_lists_the_range_of_every_variant() {
    let world = World::new(EDGE_MD, Some(EPISODE_S), None, None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert_eq!(codes(&assignment), vec![DiagnosticCode::E004]);
    assert_eq!(
        args_tag(&assignment),
        "E004[恩田恵|S|01-04-02-02|@09 .. @09, @11 ..]"
    );
}

#[test]
fn a_broken_range_counts_as_everything() {
    // B-66: 形が壊れた `range:` は全範囲扱いにして E004 を重ねない（E002 は M2 が出している）
    let world = World::new(EDGE_MD, Some(EPISODE_Q), None, None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert_eq!(
        tag(&assignment),
        "step=2 ids=[Q] defs=[Q=壊れた範囲] diags=[]"
    );
}

#[test]
fn a_mode_without_a_range_covers_every_chapter() {
    let world = World::new(EDGE_MD, Some(EPISODE_R), None, None);
    let assignment = world.resolve("09-09-09-02", 0);
    // §5.5.4「`range` 省略時は全範囲」なので第 9 章でも使える
    assert_eq!(
        tag(&assignment),
        "step=2 ids=[R] defs=[R=範囲なし] diags=[]"
    );
}

#[test]
fn an_undefined_mode_is_e003() {
    let world = World::new(EDGE_MD, Some(EPISODE_Z), None, None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert_eq!(codes(&assignment), vec![DiagnosticCode::E003]);
    // §7 E003「{0} にモード {1} は定義されていません」
    assert_eq!(args_tag(&assignment), "E003[恩田恵|Z]");
    assert_eq!(slice_of(EPISODE_Z, &assignment), "'恩田恵:Z");
    assert!(assignment.definitions.is_empty());
}

#[test]
fn an_undefined_mode_does_not_hide_the_defined_one() {
    let world = World::new(EDGE_MD, Some(EPISODE_ZR), None, None);
    let assignment = world.resolve("01-04-02-02", 0);
    // 2 モードの片方だけが未定義（§6.4.4 の最大 2 モードの範囲内）
    assert_eq!(assignment.ids, vec!["Z", "R"]);
    assert_eq!(assignment.definitions.len(), 1);
    assert_eq!(assignment.definitions[0].id(), "R");
    assert_eq!(codes(&assignment), vec![DiagnosticCode::E003]);
}

// ============================================================ E005 の分担（B-67）

#[test]
fn three_modes_in_an_episode_field_are_left_to_the_syntax_layer() {
    let world = World::onda(Some(EPISODE_THREE), None);
    let assignment = world.resolve("01-04-02-02", 0);
    // §7 のクイックフィックス「先頭 2 つを残す」は適用するが、波線は重ねない
    assert_eq!(
        tag(&assignment),
        "step=2 ids=[A+B] defs=[A=日常,B=開発・実装] diags=[]"
    );
    assert!(assignment.is_combined());
    // 波線は M2 の構文層が既に引いている
    let episode = plot_episode::parse(EPISODE_THREE);
    assert_eq!(
        episode
            .diagnostics
            .iter()
            .map(|found| found.code)
            .collect::<Vec<_>>(),
        vec![DiagnosticCode::E005]
    );
}

#[test]
fn a_repeated_character_in_a_cast_field_is_e005_here() {
    let world = World::onda(Some(EPISODE_PLAIN), Some(CHAPTER_TWICE));
    let assignment = world.resolve("01-04-02-02", 0);
    // A 形式の `cast:` の重複は構文層が出さないので、この層が引く
    assert_eq!(assignment.diagnostics.len(), 1);
    assert_eq!(assignment.diagnostics[0].file, File::Chapter);
    assert_eq!(
        assignment.diagnostics[0].diagnostic.code,
        DiagnosticCode::E005
    );
    assert_eq!(slice_of(CHAPTER_TWICE, &assignment), "'恩田恵:A '恩田恵:B");
    assert_eq!(assignment.ids, vec!["A", "B"]);
}

#[test]
fn three_modes_in_one_cast_token_are_left_to_the_dictionary() {
    let world = World::onda(Some(EPISODE_PLAIN), Some(CHAPTER_THREE));
    let assignment = world.resolve("01-04-02-02", 0);
    // 切り詰めるが波線は引かない（1 トークンの中の 3 つ以上は辞書層の担当）
    assert_eq!(
        tag(&assignment),
        "step=5 ids=[A+B] defs=[A=日常,B=開発・実装] diags=[]"
    );
    // 同じ行を辞書層が走査すると E005 が出ることの確認（担当が空白にならない）
    let line = "cast: '恩田恵:A+B+C";
    let found = dictionary::scan_line(line, 0, &world.dictionary)
        .diagnostics
        .into_iter()
        .map(|diagnostic| {
            let range = diagnostic.range.unwrap_or(0..0);
            (diagnostic.code, line[range].to_string())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        found,
        vec![(DiagnosticCode::E005, "'恩田恵:A+B+C".to_string())]
    );
}

#[test]
fn three_modes_in_the_default_mode_are_e005_here() {
    let world = World::new(DEFAULT_THREE_MD, None, None, None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert_eq!(assignment.diagnostics.len(), 1);
    assert_eq!(assignment.diagnostics[0].file, File::Characters);
    assert_eq!(
        assignment.diagnostics[0].diagnostic.code,
        DiagnosticCode::E005
    );
    assert_eq!(slice_of(DEFAULT_THREE_MD, &assignment), "A+B+C");
    assert_eq!(assignment.ids, vec!["A", "B"]);
}

#[test]
fn two_modes_in_the_default_mode_are_accepted() {
    let world = World::new(DEFAULT_TWO_MD, None, None, None);
    let assignment = world.resolve("01-04-02-02", 0);
    // §5.5.3 の値は `mode_id` 1 つだが、`A+B` も受け付ける（B-65）
    assert_eq!(
        tag(&assignment),
        "step=7 ids=[A+B] defs=[A=日常,B=開発] diags=[]"
    );
    assert!(assignment.is_combined());
}

// ============================================================ W006（§6.4.3 の段 8）

#[test]
fn w006_points_at_the_section_of_the_episode_plot() {
    let world = World::new(NO_DEFAULT_MD, Some(EPISODE_PLAIN), None, None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert_eq!(assignment.diagnostics.len(), 1);
    assert_eq!(assignment.diagnostics[0].file, File::Episode);
    assert_eq!(
        assignment.diagnostics[0].diagnostic.code,
        DiagnosticCode::W006
    );
    // §7 W006 のクイックフィックスは「プロットに `mode:` を追加」なので、追加する場所を指す
    assert_eq!(slice_of(EPISODE_PLAIN, &assignment), "設計図を机に広げる。");
}

#[test]
fn w006_without_an_episode_plot_has_no_range() {
    let world = World::new(NO_DEFAULT_MD, None, None, None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert_eq!(codes(&assignment), vec![DiagnosticCode::W006]);
    assert_eq!(range_text(&assignment.diagnostics[0].diagnostic.range), "-");
}

#[test]
fn w006_is_not_reported_when_a_mode_is_found() {
    let world = World::onda(Some(EPISODE), None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert!(!codes(&assignment).contains(&DiagnosticCode::W006));
}

// ============================================================ 添字（キャラの選択）

#[test]
fn definitions_uses_the_character_index_not_a_line_index() {
    let characters = characters_md::parse(TWO_MD);
    // 2 人目（添字 1）。行 index と混同すると 1 人目の `### A` が返る
    let found = modes::definitions(&characters, 1);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id(), "B");
    assert_eq!(found[0].label(), "二人目のB");
    assert_eq!(modes::definitions(&characters, 0)[0].id(), "A");
}

#[test]
fn the_second_character_resolves_its_own_default_mode() {
    let world = World::new(TWO_MD, None, None, None);
    assert_eq!(
        tag(&world.resolve("01-04-02-02", 1)),
        "step=7 ids=[B] defs=[B=二人目のB] diags=[]"
    );
    // 1 人目は `default-mode:` を持たないので未決定
    assert_eq!(world.resolve("01-04-02-02", 0).step, Step::Undetermined);
}

#[test]
fn a_reference_is_matched_by_character_index() {
    let world = World::new(TWO_MD, Some(EPISODE_TWO), None, None);
    // 対象が 1 人目（添字 0）なら `'二人目:B` は拾わない → 段 8
    assert_eq!(world.resolve("01-04-02-02", 0).step, Step::Undetermined);
    // 対象が 2 人目（添字 1）なら拾う
    assert_eq!(
        tag(&world.resolve("01-04-02-02", 1)),
        "step=2 ids=[B] defs=[B=二人目のB] diags=[]"
    );
}

#[test]
fn an_alias_reaches_the_same_character() {
    let world = World::onda(None, None);
    // `めぐみ` は §5.5.3 の `alias:`（辞書経由で同じキャラに当たる）
    let entry = world
        .dictionary
        .lookup("めぐみ", None)
        .expect("alias が無い");
    let index = match &world.dictionary.entry(entry).expect("entry").target {
        dictionary::Target::Character { index, .. } => *index,
        other => panic!("{other:?}"),
    };
    assert_eq!(index, 0);
    assert_eq!(
        tag(&world.resolve("01-04-02-02", index)),
        "step=7 ids=[A] defs=[A=日常] diags=[]"
    );
}

// ============================================================ §6.4.1 / §6.4.6 の実体

#[test]
fn definitions_lists_every_mode_in_written_order() {
    let characters = characters_md::parse(ONDA_MD);
    let found = modes::definitions(&characters, 0);
    assert_eq!(
        found
            .iter()
            .map(|definition| format!("{}={}", definition.id(), definition.label()))
            .collect::<Vec<_>>()
            .join(","),
        "A=日常,B=開発・実装,C=混乱・言語化障害,C=混乱・言語化障害（改善後）,D=あおい依存・感傷"
    );
}

#[test]
fn definitions_with_id_ignores_ascii_case() {
    let characters = characters_md::parse(ONDA_MD);
    // B-64: §6.1.3 が名前を小文字化して比べるのと同じ流儀（`:c` を `### C` に当てられる）
    assert_eq!(modes::definitions_with_id(&characters, 0, "C").len(), 2);
    assert_eq!(modes::definitions_with_id(&characters, 0, "c").len(), 2);
    assert_eq!(modes::definitions_with_id(&characters, 0, "Z").len(), 0);
}

#[test]
fn definitions_of_a_character_without_modes_is_empty() {
    let characters = characters_md::parse(ONDA_MD);
    // 葵あおい（添字 1）は `## modes` を持たない
    assert!(modes::definitions(&characters, 1).is_empty());
    // 居ないキャラは空（パニックしない）
    assert!(modes::definitions(&characters, 99).is_empty());
}

#[test]
fn to_model_reads_the_spec_example() {
    let characters = characters_md::parse(ONDA_MD);
    let definition = modes::definitions_with_id(&characters, 0, "A")[0];
    let mode = modes::to_model(&definition, &characters);
    assert_eq!(mode.id, "A");
    assert_eq!(mode.label, "日常");
    assert_eq!(mode.range.as_deref(), Some("@01 .."));
    assert_eq!(
        mode.personality.as_deref(),
        Some("おとなしく控えめ。'葵あおい の後ろに隠れていることが多い。")
    );
    assert_eq!(
        mode.emotion.as_deref(),
        Some("平穏を好み、あおいと一緒にいられる時間を何よりの幸福と感じる。")
    );
    assert_eq!(
        mode.role.as_deref(),
        Some("主人公の傍らに寄り添う可憐な「嫁」キャラ。")
    );
    assert_eq!(mode.note, None);
    assert_eq!(patterns(&mode.patterns), "「*だよ…」「えへへ、*」");
}

#[test]
fn to_model_of_the_second_variant_differs() {
    let characters = characters_md::parse(ONDA_MD);
    let variants = modes::definitions_with_id(&characters, 0, "C");
    let first = modes::to_model(&variants[0], &characters);
    let second = modes::to_model(&variants[1], &characters);
    // §6.4.1「`(mode_id, range)` の組で一意」= ID は同じで中身が違う
    assert_eq!(first.id, second.id);
    assert_eq!(first.range.as_deref(), Some("@01 .. @02"));
    assert_eq!(second.range.as_deref(), Some("@03 .."));
    assert_eq!(second.label, "混乱・言語化障害（改善後）");
    assert_eq!(patterns(&second.patterns), "「*あの…でも、つまり*」");
}

#[test]
fn to_model_without_a_range_keeps_it_none() {
    let characters = characters_md::parse(EDGE_MD);
    let definition = modes::definitions_with_id(&characters, 0, "R")[0];
    let mode = modes::to_model(&definition, &characters);
    // §5.5.4「`range` 省略時は全範囲」= model では `None`、解決では全範囲
    assert_eq!(mode.range, None);
    assert!(mode.range_expr().is_some());
    assert!(definition.contains(scene_id("09-09-09-09")));
}

#[test]
fn a_definition_range_decides_what_it_contains() {
    let characters = characters_md::parse(ONDA_MD);
    let variants = modes::definitions_with_id(&characters, 0, "C");
    assert!(variants[0].contains(scene_id("01-04-02-02")));
    assert!(variants[0].contains(scene_id("02-01-01-01")));
    assert!(!variants[0].contains(scene_id("03-01-01-01")));
    assert!(variants[1].contains(scene_id("03-01-01-01")));
    assert!(!variants[1].contains(scene_id("01-04-02-02")));
    assert_eq!(variants[0].range_text(), "@01 .. @02");
    assert_eq!(variants[1].range_text(), "@03 ..");
}

// ============================================================ §5.5.5 / §6.5.3 適用パターン集合

#[test]
fn applicable_patterns_is_the_union_of_mode_and_tone() {
    let characters = characters_md::parse(ONDA_MD);
    let world = World::onda(Some(EPISODE), None);
    let assignment = world.resolve("01-04-02-02", 0);
    // モード専用 → キャラ共通の順（§5.5.5「モード専用 ∪ キャラ共通」。上書きではない）
    assert_eq!(
        patterns(&modes::applicable_patterns(&characters, &assignment, 0)),
        "「*だよ…」「えへへ、*」「*で…」「*です…」「…あの…*」"
    );
}

#[test]
fn applicable_patterns_of_two_modes_sums_them() {
    let characters = characters_md::parse(ONDA_MD);
    let world = World::onda(Some(EPISODE_THREE), None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert_eq!(assignment.ids, vec!["A", "B"]);
    // §6.5.3「有効モードが 2 つならそれぞれの和」
    assert_eq!(
        patterns(&modes::applicable_patterns(&characters, &assignment, 0)),
        "「*だよ…」「えへへ、*」「*ないと…！」「*ないんです」「*で…」「*です…」「…あの…*」"
    );
}

#[test]
fn applicable_patterns_dedupes_the_same_pattern() {
    let characters = characters_md::parse(DEDUPE_MD);
    let world = World::new(DEDUPE_MD, None, None, None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert_eq!(assignment.ids, vec!["A"]);
    // §6.5.4「二重線にしない」= 同じパターンは 1 本
    assert_eq!(
        patterns(&modes::applicable_patterns(&characters, &assignment, 0)),
        "「*で…」「*だよ…」"
    );
}

#[test]
fn applicable_patterns_without_a_mode_is_tone_only() {
    let characters = characters_md::parse(ONDA_MD);
    let undetermined = Assignment {
        step: Step::Undetermined,
        ids: Vec::new(),
        definitions: Vec::new(),
        diagnostics: Vec::new(),
    };
    // §6.4.3 の段 8 でもキャラ共通のパターンは適用される
    assert_eq!(
        patterns(&modes::applicable_patterns(&characters, &undetermined, 0)),
        "「*で…」「*です…」「…あの…*」"
    );
    // `## tone` が無ければ空
    let plain = characters_md::parse(NO_DEFAULT_MD);
    assert!(modes::applicable_patterns(&plain, &undetermined, 0).is_empty());
}

#[test]
fn patterns_of_reads_every_pattern_on_a_line() {
    let characters = characters_md::parse(ONDA_MD);
    let tone = characters.characters[0]
        .section(SectionKey::Tone)
        .expect("## tone が無い");
    assert_eq!(
        patterns(&modes::patterns_of(&characters, &tone.patterns)),
        "「*で…」「*です…」「…あの…*」"
    );
}

// ============================================================ その他

#[test]
fn scene_text_uses_the_default_digits() {
    // §6.4.4 の E004 の引数 `{2}` は既定の桁数（`project.json.structure.id_digits`）
    assert_eq!(modes::scene_text(&scene_id("01-04-02-02")), "01-04-02-02");
    assert_eq!(modes::scene_text(&scene_id("1-4-2-0")), "01-04-02");
    assert_eq!(
        scene_id("1-4-2-2").display(Digits {
            chapter: 3,
            episode: 3,
            scene: 3,
            section: 3
        }),
        "001-004-002-002"
    );
}

#[test]
fn one_mode_is_not_combined() {
    let world = World::onda(Some(EPISODE), None);
    let assignment = world.resolve("01-04-02-02", 0);
    assert!(assignment.is_determined());
    assert!(!assignment.is_combined());
}

#[test]
fn an_unknown_character_index_resolves_to_nothing() {
    let world = World::onda(Some(EPISODE), None);
    let assignment = world.resolve("01-04-02-02", 99);
    assert_eq!(assignment.step, Step::Undetermined);
    assert_eq!(codes(&assignment), vec![DiagnosticCode::W006]);
}

#[test]
fn broken_input_does_not_panic() {
    // §4.3-4「パース失敗を起動失敗にしない」。壊れた入力でも 8 段を走り切る
    let broken = [
        "",
        "@\n;\n",
        "mode: '\n@01-04-02\n;",
        "@01-04-02-03-04 攻防\n;\n",
    ];
    for episode in broken {
        let world = World::new(EDGE_MD, Some(episode), Some("cast: '恩田恵:A+B+C\n"), None);
        for scene in ["01-04-02-02", "00-00-00-00", "9999-9999-9999-9999"] {
            let assignment = world.resolve(scene, 0);
            assert!(assignment.step.order() >= 1);
            assert!(assignment.step.order() <= 8);
            assert!(assignment.ids.len() <= 2, "{:?}", assignment.ids);
        }
    }
}
