//! エクスポートのテスト（M9）で使う「実プロジェクト」（§17.1「サンプルと実プロジェクトで検証する」）。
//!
//! 仕様書の例（`tests/world`）は 1 話 1 場面しかないので、章・話・場面をまたぎ、§17.1 の 3〜10 で
//! 消える・変わるもの（行コメント・行内コメント・行末コメント・話者注釈・明示参照・資料参照・
//! 名前の無い `'`・エスケープ・ルビの 2 つの形・空の節・URL）と、状態の 3 段（節・話・章）を
//! ひととおり含めた。辞書は `tests/world` と同じ（アルスとリリア・星屑の時計・M0137）。
//!
//! 実効の状態（`docs/history.md` B-122）:
//!
//! | 節 | 状態 | どこに書いてあるか |
//! |---|---|---|
//! | `@01-01-01-01` | fixed | 話プロットの節 |
//! | `@01-01-01-02` | review | 全体プロットの章 `@01` |
//! | `@01-01-01-03` | draft | 話プロットの節 |
//! | `@01-01-02-01` | review | 全体プロットの章 `@01` |
//! | `@01-02-01-01`〜`03` | fixed | 章プロットの話 `@01-02` |
//! | `@02-01-01-01` | draft | どこにも無い |

#![allow(dead_code)]

// 辞書は `tests/world` のもの（使う側のクレートが `mod world;` も宣言すること）
use super::world;

use novnow::core::features::Project;
use novnow::core::model::document::Document;
use novnow::core::model::project::Project as Settings;

pub const PROJECT_JSON: &str = r#"{
  "schema_version": 1,
  "title": "星屑の時計",
  "export": {
    "paragraph_break": "blank-line",
    "indent_first_line": false,
    "keep_ruby": true,
    "insert_headings": false,
    "newline": "crlf"
  }
}
"#;

pub const ALL: &str = "// 全体プロット
title: 星屑の時計
summary: 古代錬金術を追う少年と幼馴染の物語。

@01 失われた刻印
summary: 'アルス が地下遺跡に惹かれる導入章。
state: review
;

@02 補習と地図
summary: 期末試験の失敗を代償に、二人は旧校舎の鍵を手に入れる。
;
";

pub const CHAPTER_01: &str = "// 章プロット
@01-01 図書室の密談
summary: 二人が図書室で出会い直す。
;

@01-02 旧校舎へ
summary: 二人は旧校舎の扉を開ける。
state: fixed
;
";

pub const EPISODE_01_01: &str = "@01-01-01 放課後
二人が図書室で話す。
state: fixed
;
'アルス が資料を見せる。
;
'リリア が設計図を覗き込む。
state: draft
;

@01-01-02 帰り道
二人で帰る。
";

pub const CONTENTS_01_01: &str = "// 第 1 話。見出しより前の行は書き出さない
@01-01-01 放課後
\u{3000}放課後の図書室は静かだった。'アルス は｜古文書《こもんじょ》を広げている。
「やっぱり'{アルス}、ここにいたのね」@リリア:A

/* ここは後で書き直す */
'リリアは溜め息をついた。/* 仮の一文 */窓の外は夕焼けだ。 // メモ
; // ここで節を切る
「これに載ってたんだ」'M0137 @アルス:B
彼が見せたのは古い写しだった。'M0137:科学の甲子園
本文の\\@は残る。宛先は hoshi\\@example.jp、資料は https://example.jp/hoshi にある。
;
'『星屑の時計』の設計図を、'リリアは覗き込んだ。'S0042
// 行コメントだけの行
魔導具《まどうぐ》の図面には、'未登録の紋様 が描かれていた。

@01-01-02 帰り道
夕焼けの中を、二人は並んで歩いた。
「明日も来るの？」@リリア
";

pub const CONTENTS_01_02: &str = "@01-02-01 旧校舎
旧校舎の扉は、錆びついていた。
;
;
扉の向こうは暗かった。\\\\ 落書きの「\\「」が見える。
";

pub const CONTENTS_02_01: &str = "@02-01-01 補習
補習の教室で、'アルス は居眠りをしていた。
";

/// 既定の設定（`blank-line`・ルビを残す・見出しなし・CRLF）での全体の書き出し（§17.1）。
pub const EXPECTED: &str =
    "\u{3000}放課後の図書室は静かだった。アルス は｜古文書《こもんじょ》を広げている。\r
「やっぱりアルス、ここにいたのね」\r
\r
リリアは溜め息をついた。窓の外は夕焼けだ。\r
\r
「これに載ってたんだ」\r
彼が見せたのは古い写しだった。\r
本文の@は残る。宛先は hoshi@example.jp、資料は https://example.jp/hoshi にある。\r
\r
『星屑の時計』の設計図を、リリアは覗き込んだ。\r
魔導具《まどうぐ》の図面には、未登録の紋様 が描かれていた。\r
\r
夕焼けの中を、二人は並んで歩いた。\r
「明日も来るの？」\r
\r
旧校舎の扉は、錆びついていた。\r
\r
扉の向こうは暗かった。\\ 落書きの「「」が見える。\r
\r
補習の教室で、アルス は居眠りをしていた。\r
";

/// ファイル（パス・中身）。
pub fn files() -> Vec<(&'static str, &'static str)> {
    vec![
        ("project.toml", PROJECT_JSON),
        ("characters.md", world::CHARACTERS),
        ("words.md", world::WORDS),
        ("materials/index.json", world::INDEX),
        ("plots/all.cnov", ALL),
        ("plots/01.cnov", CHAPTER_01),
        ("plots/01-01.cnov", EPISODE_01_01),
        ("contents/01-01.cnov", CONTENTS_01_01),
        ("contents/01-02.cnov", CONTENTS_01_02),
        ("contents/02-01.cnov", CONTENTS_02_01),
    ]
}

/// 集約にしたもの。
pub fn novel() -> Project {
    let documents = files()
        .into_iter()
        .map(|(path, text)| Document::new(path, text));
    Project::with_documents(Settings::default(), documents)
}

/// 1 つの文書を差し替えたもの。
pub fn novel_with(path: &str, text: &str) -> Project {
    let mut project = novel();
    project.insert(Document::new(path, text));
    project
}
