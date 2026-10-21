//! 診断コードの表（`SPECIFICATIONS.md` §7 / §7.1 / §7.2）。
//!
//! **実装規則（MUST、§7.2）**: `core::diagnostics` は **コードと引数だけ**を返す。
//! 文言（`辞書にない参照です: {0}` のような文字列）は `ui/i18n/*.toml` が担当する（§12.2）。
//! キーは `diag.<コード>`（例 `diag.E004`、§12.3）で、**コード文字列そのものが正準の識別子**。
//! だから core は文言を持たないが、コードを文字列で返す口（[`DiagnosticCode::as_str`]）は持つ。
//!
//! **全件を先に定義する**（`PLANS.md` §3 I5「診断は先に表を実装」）。① が出さないコード
//! （エージェント系・MCP 系・ローカル LLM 系）も enum に入れてある。後から足すと番号が
//! 散らかるためで、未使用 variant の警告は I5 が明示的に許容している。
//!
//! 中身は §7 の 3 つの表を写したもので、**仕様書が正本**。`tests/diagnostics_catalog.rs` が
//! `SPECIFICATIONS.md` を `include_str!` で読み直して、件数・コード・重大度・引数の数・
//! どの表に載っているか・文書位置の有無・既定の有効/無効・実行コスト表を突き合わせる。
//! 表を変えるときは **先に仕様書**を直すこと（`PLANS.md` §6.3 の「仕様書 → ゴールデン → 実装」）。
//!
//! 仕様書に明記の無い属性の決め方は `PLANS.md` §9.2 B-26。

use serde::Serialize;
use std::ops::Range;

use crate::core::features::project;
use crate::core::features::Project;
use crate::core::id::SceneId;
use crate::core::model::document::{Document, DocumentFormat};
use crate::core::resolve::dictionary::{self, SourceFile};
use crate::core::resolve::{context_rules, materials, modes};
use crate::core::syntax::{
    characters_md, contents, ideas_md, materials_index, plot_episode, plot_outline, project_json,
    prompt_md, words_md,
};
use crate::core::text::lexer::{self, FieldKey, LineKind};

/// 重大度（§7 の `Sev` 列）。
///
/// 並びは「重い順」で、`sort_by_key(|d| d.severity())` がエラーを先頭に置く。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// エラー（赤波線）。
    Error,
    /// 警告（黄波線）。
    Warning,
    /// 情報（下線なし、レビュー一覧のみ）。
    Info,
}

/// §7 のどの表に載っているか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    /// §7 診断カタログ。本文・プロット・資料・辞書の検査で、① の中心。
    Core,
    /// §7.1 ストレージ・移行・エージェント系。**文書位置を持たない通知**。
    Storage,
    /// §7.2 執筆支援系（v1 追加仕様）。文章の良し悪しを言うものは W / I で、設定・接続・
    /// 権限の誤り（E032〜E036・E064）だけが E（§7.2 の冒頭文、第 2.8 版。`PLANS.md` §9.1 A-8）。
    Writing,
}

/// 診断が文書内の位置を持つか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Anchor {
    /// 文書内の byte range を持つ。波線と「該当箇所を開く」が使える。
    Document,
    /// 文書位置を持たない（§7.1）。ステータスバーの通知とレビュー一覧だけに出す。
    Status,
}

/// 既定の有効 / 無効（§7.2「既定の有効 / 無効」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Toggle {
    /// トグルの対象外（常に有効）。仕様にトグルの記述が無いコード。
    Always,
    /// 既定で有効。UI のトグルで無効にできる（セッション内の一時設定）。
    OnByDefault,
    /// 既定で無効。文体ワークスペース（§10.9）を開いたときだけ計算する。
    OffByDefault,
}

/// 計算のタイミング（§7.2「実行コストの制御（MUST）」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trigger {
    /// [`COST`] に載っていない。タイミングは M4 で決める。
    Unspecified,
    /// 編集中（アイドル 300ms）。
    Editing,
    /// 明示操作またはアイドル 5 秒。
    Idle,
    /// 起動時と明示操作。
    Startup,
    /// イベント発生時（書き込みツール呼び出し / 上限到達）。
    Event,
}

/// 計算の範囲（§7.2「実行コストの制御（MUST）」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// [`COST`] に載っていない。
    Unspecified,
    /// 編集中の節のみ。
    Section,
    /// 編集中の場面（直前の節までを文脈に含む）。
    Scene,
    /// プロジェクト全体。
    Project,
    /// セッション。
    Session,
}

/// 診断コード（§7 / §7.1 / §7.2 の全 76 件）。
///
/// **variant 名はコード文字列そのもの**（`E001` → `"E001"`）。i18n のキーは
/// `diag.` を前に付けた形（`diag.E001`、§12.3「キー命名」）で、core は文言を持たない。
///
/// 並びは重大度（E → W → I）、その中で番号順。§7.2 の表は W が先に来るが、
/// enum はコードで引ける順に揃えた（`Ord` もこの順になる）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum DiagnosticCode {
    /// `'` の後が辞書に一致しない（§7）
    E001,
    /// SceneId・明示形の構文エラー（§7）
    E002,
    /// 未定義のモード ID（§7）
    E003,
    /// モードの `range` 外での割り当て（§7）
    E004,
    /// 1 キャラに 3 つ以上のモード、または 1 節の `mode` に同じキャラを 2 回（§7）
    E005,
    /// 資料タイトル不一致（§7）
    E006,
    /// 未知の資料 ID（§7）
    E007,
    /// 同一ファイル内の ID 重複（§7）
    E008,
    /// 話者注釈のキャラが未定義（§7）
    E012,
    /// `relations` 等の参照不能（§7）
    E013,
    /// 範囲式の左端 > 右端（§7）
    E016,
    /// 同一モード ID の range 重複（§7）
    E017,
    /// A 形式ブロックに `summary` 無し（§7）
    E018,
    /// 見出し階層の不一致（§7）
    E020,
    /// `words.md` に `definition` 無し（§7）
    E021,
    /// 予約番号の見出し（§5.1.2）（§7）
    E022,
    /// `materials` 外を指す `path`（§7）
    E024,
    /// JSON ファイルがパースできない（§4.3-4）（§7）
    E025,
    /// 修飾子を付けられない参照に修飾子（§6.1.1）（§7）
    E026,
    /// `project.json` の `lexicon.word_classes` の正規表現が不正（§6.5.1）（§7）
    E027,
    /// 辞書キーが 96 バイト超（§7）
    E028,
    /// プロンプトに必須のセクション（`## system` / `## task`）が無い（§15.5.4 (2)）（§7）
    E029,
    /// MCP サーバの環境変数未解決（§7）
    E030,
    /// プロンプトの未知変数（§7）
    E031,
    /// カスタムロール名が不正（§5.9.2）（§7.2）
    E032,
    /// カスタムロール名が既定ロールと衝突（§7.2）
    E033,
    /// カスタムロールが 16 件を超えた（§7.2）
    E034,
    /// 同一 role × 同一 scene のタスク重複（§15.7.5）（§7.2）
    E035,
    /// 委譲レベル不足の書き込みツールを呼んだ（§15.8.3）（§7.2）
    E036,
    /// プロジェクト外への書き込みを拒否した（§13.1.2）（§7.1）
    E060,
    /// IndexedDB が利用できない（§7.1）
    E062,
    /// MCP サーバに接続できない（§7.1）
    E063,
    /// ローカル LLM に接続できない（§15.8.4）（§7.2）
    E064,
    /// 本文中の裸の `@`（§7）
    W002,
    /// モード未指定（§7）
    W006,
    /// 未知のフィールドキー（§7）
    W011,
    /// 空の節（§7）
    W012,
    /// `summary` が 120 字超（§7）
    W019,
    /// 用語の `range` 外での使用（§7）
    W022,
    /// 資料 MD の H1 と index の不一致（§7）
    W025,
    /// 辞書キーの種別衝突（§7）
    W027,
    /// より長い辞書項目にも一致しうる（§7）
    W029,
    /// プロットと本文の節数不一致（§7）
    W030,
    /// 本文中の資料参照（§7）
    W031,
    /// `mcp.json` に秘密情報が直書き（§7）
    W033,
    /// 書き込み不可のプロジェクト（§7）
    W041,
    /// 残容量が 10MB 未満（§7.1）
    W062,
    /// index に未登録の資料 MD がある（§7.1）
    W064,
    /// 出典のない資料（§7.1）
    W065,
    /// アイコンの形式が PNG/JPEG でない（§5.5.3）（§7.1）
    W068,
    /// アイコンが大きすぎる（長辺 256px 超 / 256KB 超）（§7.1）
    W069,
    /// アイコンのファイルが存在しない（§7.1）
    W070,
    /// 節順に対し `when` が巻き戻る（§5.3.5）（§7.2）
    W080,
    /// 1 文に読点が 1 つも無く `long_sentence_chars` を超える（§7.2）
    W081,
    /// 同一話者の台詞行が `max_consecutive_speeches` を超えて連続（§7.2）
    W082,
    /// 連続 5 節以上で `senses` が視覚系のみ（§7.2）
    W083,
    /// 台詞内の呼称が `address:` に反する（§5.5.6）（§7.2）
    W084,
    /// 呼称参照 `'先輩` の相手が一意に決まらない（§7.2）
    W085,
    /// 章あらすじ（`B=00`）の本文が `blurb` と大きく食い違う長さ（§7.2）
    W086,
    /// 同一段落内で同じ自立語が 3 回以上（§9.4.4）（§7.2）
    W087,
    /// 文頭の接続詞が 3 文連続（§9.4.4）（§7.2）
    W088,
    /// 1 文が `long_sentence_chars` の 2 倍を超える（§7.2）
    W089,
    /// 1 節の台詞比率が `dialogue_ratio_target` を大きく外れる（§7.2）
    W090,
    /// 節の長さが話内平均の T スコア 35 未満 / 65 超（§10.3.4）（§7.2）
    W091,
    /// `icon` / `space` の参照先ファイルが無い（§7.2）
    W092,
    /// 提案の `before` が現在の本文と食い違う（§8.7.4）（§7.2）
    W093,
    /// 正規表現検索が 1 秒で打ち切られた（§9.7.4）（§7.2）
    W094,
    /// 自律タスクがセッション上限（`agents.limits`）に達して停止した（§15.1.1 A8）（§7.2）
    W095,
    /// 既定プロンプトを生成した（§7）
    I050,
    /// 未保存の変更がある（§7）
    I051,
    /// 外部変更を取り込んだ（§7.1）
    I052,
    /// どの節からも引用されていない採用済み資料（§5.2.4）（§7.2）
    I054,
    /// ストック章（`00`）に本文が残っている（§5.1.2）（§7.2）
    I055,
    /// `prompts/custom/` に未登録のロールがある（§5.9.2）（§7.2）
    I056,
    /// ローカル LLM の設定を書き込んだ（§15.8.4）（§7.2）
    I057,
    /// 検索結果を資料化した（§16.3.1）（§7.2）
    I058,
}

impl DiagnosticCode {
    /// コード文字列（`"E001"`）。
    ///
    /// §12.3 の i18n キーはこれに `diag.` を前に付ける。**これが正準の識別子**で、
    /// レビュー一覧のフィルタ・ログ・`.expected.json` もこの文字列を使う。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::E001 => "E001",
            Self::E002 => "E002",
            Self::E003 => "E003",
            Self::E004 => "E004",
            Self::E005 => "E005",
            Self::E006 => "E006",
            Self::E007 => "E007",
            Self::E008 => "E008",
            Self::E012 => "E012",
            Self::E013 => "E013",
            Self::E016 => "E016",
            Self::E017 => "E017",
            Self::E018 => "E018",
            Self::E020 => "E020",
            Self::E021 => "E021",
            Self::E022 => "E022",
            Self::E024 => "E024",
            Self::E025 => "E025",
            Self::E026 => "E026",
            Self::E027 => "E027",
            Self::E028 => "E028",
            Self::E029 => "E029",
            Self::E030 => "E030",
            Self::E031 => "E031",
            Self::E032 => "E032",
            Self::E033 => "E033",
            Self::E034 => "E034",
            Self::E035 => "E035",
            Self::E036 => "E036",
            Self::E060 => "E060",
            Self::E062 => "E062",
            Self::E063 => "E063",
            Self::E064 => "E064",
            Self::W002 => "W002",
            Self::W006 => "W006",
            Self::W011 => "W011",
            Self::W012 => "W012",
            Self::W019 => "W019",
            Self::W022 => "W022",
            Self::W025 => "W025",
            Self::W027 => "W027",
            Self::W029 => "W029",
            Self::W030 => "W030",
            Self::W031 => "W031",
            Self::W033 => "W033",
            Self::W041 => "W041",
            Self::W062 => "W062",
            Self::W064 => "W064",
            Self::W065 => "W065",
            Self::W068 => "W068",
            Self::W069 => "W069",
            Self::W070 => "W070",
            Self::W080 => "W080",
            Self::W081 => "W081",
            Self::W082 => "W082",
            Self::W083 => "W083",
            Self::W084 => "W084",
            Self::W085 => "W085",
            Self::W086 => "W086",
            Self::W087 => "W087",
            Self::W088 => "W088",
            Self::W089 => "W089",
            Self::W090 => "W090",
            Self::W091 => "W091",
            Self::W092 => "W092",
            Self::W093 => "W093",
            Self::W094 => "W094",
            Self::W095 => "W095",
            Self::I050 => "I050",
            Self::I051 => "I051",
            Self::I052 => "I052",
            Self::I054 => "I054",
            Self::I055 => "I055",
            Self::I056 => "I056",
            Self::I057 => "I057",
            Self::I058 => "I058",
        }
    }

    /// コード文字列から引く（レビュー一覧のフィルタ・テスト用）。
    pub fn parse(text: &str) -> Option<Self> {
        CATALOG
            .iter()
            .find(|row| row.0.as_str() == text)
            .map(|row| row.0)
    }

    /// 3 桁の番号（`E001` → 1）。
    pub fn number(self) -> u16 {
        let mut n = 0;
        for byte in self.as_str()[1..].bytes() {
            n = n * 10 + u16::from(byte - b'0');
        }
        n
    }

    /// 重大度。§7 の `Sev` 列はコードの 1 文字目と常に一致する（テストで固定）。
    pub fn severity(self) -> Severity {
        match self.as_str().as_bytes()[0] {
            b'E' => Severity::Error,
            b'W' => Severity::Warning,
            _ => Severity::Info,
        }
    }

    /// §7 のどの表に載っているか。番号の帯で決まる（未使用の番号は §7.1 / §7.2 の帯）。
    pub fn group(self) -> Group {
        let n = self.number();
        match self.severity() {
            Severity::Error => match n {
                1..=31 => Group::Core,
                32..=36 | 64 => Group::Writing,
                _ => Group::Storage,
            },
            Severity::Warning => match n {
                2..=41 => Group::Core,
                62..=70 => Group::Storage,
                _ => Group::Writing,
            },
            Severity::Info => match n {
                50..=51 => Group::Core,
                52 => Group::Storage,
                _ => Group::Writing,
            },
        }
    }

    /// [`CATALOG`] の行。
    pub fn row(self) -> Option<&'static CatalogRow> {
        CATALOG.iter().find(|row| row.0 == self)
    }

    /// 文言のプレースホルダの数（`{0}` の最大添字 + 1）。
    pub fn params(self) -> u8 {
        self.row().map_or(0, |row| row.1)
    }

    /// 文書位置を持つか。
    pub fn anchor(self) -> Anchor {
        if STATUS_ANCHOR.contains(&self) {
            Anchor::Status
        } else {
            Anchor::Document
        }
    }

    /// 既定の有効 / 無効。
    pub fn toggle(self) -> Toggle {
        if TOGGLE_OFF.contains(&self) {
            Toggle::OffByDefault
        } else if TOGGLE_ON.contains(&self) {
            Toggle::OnByDefault
        } else {
            Toggle::Always
        }
    }

    /// 実行タイミング（[`COST`] に無ければ [`Trigger::Unspecified`]）。
    pub fn trigger(self) -> Trigger {
        self.cost().map_or(Trigger::Unspecified, |row| row.1)
    }

    /// 計算の範囲（[`COST`] に無ければ [`Scope::Unspecified`]）。
    pub fn scope(self) -> Scope {
        self.cost().map_or(Scope::Unspecified, |row| row.2)
    }

    /// [`COST`] の行。
    pub fn cost(self) -> Option<&'static CostRow> {
        COST.iter().find(|row| row.0 == self)
    }

    /// 既定の状態で計算・表示する診断か（§7.2「既定の有効 / 無効」）。
    pub fn enabled_by_default(self) -> bool {
        self.toggle() != Toggle::OffByDefault
    }
}

/// [`CATALOG`] の 1 行 = `(コード, 引数の数)`。
pub type CatalogRow = (DiagnosticCode, u8);

/// 全コードの表。**並びは [`DiagnosticCode`] と同じ**（重大度 → 番号）。
///
/// 引数の数は §7 の **ja 文言**の `{0}` 形式プレースホルダの**最大添字 + 1**。
/// en 文言のプレースホルダは全件で ja と一致することを `tests/diagnostics_catalog.rs`
/// が仕様書を読み直して確認している。
pub const CATALOG: &[CatalogRow] = &[
    (DiagnosticCode::E001, 1),
    (DiagnosticCode::E002, 0),
    (DiagnosticCode::E003, 2),
    (DiagnosticCode::E004, 4),
    (DiagnosticCode::E005, 0),
    (DiagnosticCode::E006, 1),
    (DiagnosticCode::E007, 1),
    (DiagnosticCode::E008, 1),
    (DiagnosticCode::E012, 1),
    (DiagnosticCode::E013, 1),
    (DiagnosticCode::E016, 0),
    (DiagnosticCode::E017, 1),
    (DiagnosticCode::E018, 0),
    (DiagnosticCode::E020, 1),
    (DiagnosticCode::E021, 0),
    (DiagnosticCode::E022, 1),
    (DiagnosticCode::E024, 0),
    (DiagnosticCode::E025, 1),
    (DiagnosticCode::E026, 0),
    (DiagnosticCode::E027, 1),
    (DiagnosticCode::E028, 0),
    (DiagnosticCode::E029, 1),
    (DiagnosticCode::E030, 1),
    (DiagnosticCode::E031, 1),
    (DiagnosticCode::E032, 1),
    (DiagnosticCode::E033, 1),
    (DiagnosticCode::E034, 0),
    (DiagnosticCode::E035, 2),
    (DiagnosticCode::E036, 1),
    (DiagnosticCode::E060, 1),
    (DiagnosticCode::E062, 0),
    (DiagnosticCode::E063, 1),
    (DiagnosticCode::E064, 1),
    (DiagnosticCode::W002, 0),
    (DiagnosticCode::W006, 0),
    (DiagnosticCode::W011, 1),
    (DiagnosticCode::W012, 0),
    (DiagnosticCode::W019, 1),
    (DiagnosticCode::W022, 1),
    (DiagnosticCode::W025, 0),
    (DiagnosticCode::W027, 1),
    (DiagnosticCode::W029, 1),
    (DiagnosticCode::W030, 2),
    (DiagnosticCode::W031, 0),
    (DiagnosticCode::W033, 0),
    (DiagnosticCode::W041, 0),
    (DiagnosticCode::W062, 0),
    (DiagnosticCode::W064, 1),
    (DiagnosticCode::W065, 1),
    (DiagnosticCode::W068, 1),
    (DiagnosticCode::W069, 1),
    (DiagnosticCode::W070, 1),
    (DiagnosticCode::W080, 2),
    (DiagnosticCode::W081, 1),
    (DiagnosticCode::W082, 2),
    (DiagnosticCode::W083, 1),
    (DiagnosticCode::W084, 3),
    (DiagnosticCode::W085, 1),
    (DiagnosticCode::W086, 1),
    (DiagnosticCode::W087, 2),
    (DiagnosticCode::W088, 0),
    (DiagnosticCode::W089, 1),
    (DiagnosticCode::W090, 2),
    (DiagnosticCode::W091, 2),
    (DiagnosticCode::W092, 1),
    (DiagnosticCode::W093, 1),
    (DiagnosticCode::W094, 0),
    (DiagnosticCode::W095, 1),
    (DiagnosticCode::I050, 0),
    (DiagnosticCode::I051, 0),
    (DiagnosticCode::I052, 1),
    (DiagnosticCode::I054, 1),
    (DiagnosticCode::I055, 1),
    (DiagnosticCode::I056, 1),
    (DiagnosticCode::I057, 0),
    (DiagnosticCode::I058, 1),
];

/// 文書位置を持たない診断（[`Anchor::Status`]）。
///
/// 内訳は 3 つ。
///
/// 1. **§7.1 の全 10 件**。表の見出しが「文書位置を持たない通知」で、波線ではなく
///    ステータスバーの通知とレビュー一覧に出す（仕様書の明記）。
/// 2. **§7.2 の E032〜E036**。§7.2 の注「エラーだが文書位置を持たないため、
///    §7.1 と同じくステータスバー通知として扱う」（仕様書の明記）。
/// 3. **位置を指しようがない通知**（接続・保存・検索・件数・プロジェクト全体のもの）。
///    仕様書の明記は無いので、判断は `PLANS.md` §9.2 B-26 に記録した。
///
/// 1 と 2 は `tests/diagnostics_catalog.rs` が仕様書から導いて一致を確認している。
pub const STATUS_ANCHOR: &[DiagnosticCode] = &[
    DiagnosticCode::E032,
    DiagnosticCode::E033,
    DiagnosticCode::E034,
    DiagnosticCode::E035,
    DiagnosticCode::E036,
    DiagnosticCode::E060,
    DiagnosticCode::E062,
    DiagnosticCode::E063,
    DiagnosticCode::E064,
    DiagnosticCode::W041,
    DiagnosticCode::W062,
    DiagnosticCode::W064,
    DiagnosticCode::W065,
    DiagnosticCode::W068,
    DiagnosticCode::W069,
    DiagnosticCode::W070,
    DiagnosticCode::W094,
    DiagnosticCode::W095,
    DiagnosticCode::I050,
    DiagnosticCode::I051,
    DiagnosticCode::I052,
    DiagnosticCode::I054,
    DiagnosticCode::I055,
    DiagnosticCode::I056,
    DiagnosticCode::I057,
    DiagnosticCode::I058,
];

/// 既定で**無効**の診断（§7.2「既定の有効 / 無効」）。
///
/// 文体ワークスペース（§10.9）を開いたときだけ計算・表示する。理由は仕様書どおり
/// 「書いている最中に言われたくない種類の指摘」で、P3（ノイズを増やさない）に抵触するため。
/// 有効 / 無効は設定ファイルに書かず **UI のトグルのみ**（セッション内の一時設定）。
pub const TOGGLE_OFF: &[DiagnosticCode] = &[
    DiagnosticCode::W081,
    DiagnosticCode::W083,
    DiagnosticCode::W087,
    DiagnosticCode::W088,
    DiagnosticCode::W089,
    DiagnosticCode::W090,
    DiagnosticCode::W091,
];

/// 既定で**有効**だがトグルできる診断（§7.2「既定の有効 / 無効」）。
///
/// W085・W086 は仕様書が名指ししていない（有効 9 件・無効 7 件のどちらにも入っていない）。
/// W084 と同じタイミング・範囲で計算する同族の lint なので有効側に寄せた。
/// 判断は `PLANS.md` §9.2 B-26。
pub const TOGGLE_ON: &[DiagnosticCode] = &[
    DiagnosticCode::E064,
    DiagnosticCode::W080,
    DiagnosticCode::W082,
    DiagnosticCode::W084,
    DiagnosticCode::W085,
    DiagnosticCode::W086,
    DiagnosticCode::W092,
    DiagnosticCode::W095,
    DiagnosticCode::I054,
    DiagnosticCode::I055,
    DiagnosticCode::I056,
];

/// [`COST`] の 1 行 = `(コード, 実行タイミング, 範囲)`。
pub type CostRow = (DiagnosticCode, Trigger, Scope);

/// 実行コストの制御表（§7.2「実行コストの制御（MUST）」）。
///
/// 載っているのは §7.2 の 16 件のみ。それ以外は [`Trigger::Unspecified`] で、
/// 実行タイミングは M4（診断）で決める。**範囲は MUST** なので、
/// `Scope::Section` の診断をプロジェクト全体で計算してはいけない。
pub const COST: &[CostRow] = &[
    (DiagnosticCode::W081, Trigger::Editing, Scope::Section),
    (DiagnosticCode::W087, Trigger::Editing, Scope::Section),
    (DiagnosticCode::W088, Trigger::Editing, Scope::Section),
    (DiagnosticCode::W089, Trigger::Editing, Scope::Section),
    (DiagnosticCode::W090, Trigger::Editing, Scope::Section),
    (DiagnosticCode::W082, Trigger::Editing, Scope::Scene),
    (DiagnosticCode::W084, Trigger::Editing, Scope::Scene),
    (DiagnosticCode::W085, Trigger::Editing, Scope::Scene),
    (DiagnosticCode::W080, Trigger::Idle, Scope::Project),
    (DiagnosticCode::W083, Trigger::Idle, Scope::Project),
    (DiagnosticCode::W091, Trigger::Idle, Scope::Project),
    (DiagnosticCode::I054, Trigger::Startup, Scope::Project),
    (DiagnosticCode::I055, Trigger::Startup, Scope::Project),
    (DiagnosticCode::I056, Trigger::Startup, Scope::Project),
    (DiagnosticCode::E036, Trigger::Event, Scope::Session),
    (DiagnosticCode::W095, Trigger::Event, Scope::Session),
];

/// 文言のプレースホルダ（`{0}` 等）に埋める値。
///
/// 表示用の文字列化は `ui/i18n/*` が行う（§12.2）。core は**値の意味だけ**を渡す。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Arg {
    /// 名前・ID・パス・範囲など、そのまま挿入する文字列。
    Text(String),
    /// 字数・件数・比率・偏差値など。
    Number(i64),
}

impl Arg {
    /// 文字列の引数。
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    /// 数値の引数。
    pub fn number(value: i64) -> Self {
        Self::Number(value)
    }
}

/// 1 件の診断。**コードと引数だけ**を持つ（§7.2「診断の実装規則（MUST）」）。
///
/// 文言もアイコンも波線の色も持たない。それらは §7 の表と §11（テーマ）から
/// UI 側が引く。
///
/// **JSON 化（M2 で決定）**: ゴールデンテストの `.expected.json` に出す形は
/// `{"code": "E020", "range": {"start": 3, "end": 9}, "args": [{"Number": 3}]}`。
/// `code` はユニットバリアントなので §7.2 のコード文字列そのもの、`range` は
/// `Option<Range<usize>>`（無いときは `null`）、`args` は `Arg` の newtype バリアント。
/// **文言は入れない**（§12.2: 表示用の文字列化は `ui/i18n/*` が行う）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// コード。
    pub code: DiagnosticCode,
    /// プレースホルダに埋める値。個数は [`DiagnosticCode::params`] と揃える。
    pub args: Vec<Arg>,
    /// 文書内の byte range（§3.4「core は内部で byte offset」、`PLANS.md` §9.2 B-1）。
    /// [`Anchor::Status`] の診断は `None`。
    pub range: Option<Range<usize>>,
}

impl Diagnostic {
    /// 引数を取る診断を作る。
    ///
    /// `args.len()` と [`DiagnosticCode::params`] の不一致はデバッグビルドで落とす。
    /// 文言を組み立てるのは UI なので、個数がずれると `{1}` が空のまま表示される。
    pub fn new(code: DiagnosticCode, range: Option<Range<usize>>, args: Vec<Arg>) -> Self {
        debug_assert_eq!(args.len(), usize::from(code.params()), "{}", code.as_str());
        Self { code, args, range }
    }

    /// 引数を取らない診断を作る。
    pub fn simple(code: DiagnosticCode, range: Option<Range<usize>>) -> Self {
        debug_assert_eq!(code.params(), 0, "{}", code.as_str());
        Self {
            code,
            args: Vec::new(),
            range,
        }
    }

    /// 重大度。
    pub fn severity(&self) -> Severity {
        self.code.severity()
    }

    /// 文書位置を持つか。
    pub fn anchor(&self) -> Anchor {
        self.code.anchor()
    }

    /// 波線として表示できる診断か（§7「I=情報（下線なし、レビュー一覧のみ）」）。
    pub fn is_underlined(&self) -> bool {
        self.code.anchor() == Anchor::Document && self.code.severity() != Severity::Info
    }
}

// ============================================================ 文書単位の診断（§3.4）

/// 1 文書の診断を全部集める（§3.4 `diagnostics::document(doc, project)` →
/// `textDocument/publishDiagnostics`）。範囲は**この文書の中の byte**。
///
/// 下の層が出す診断を**1 か所で組む**まとめ役（`PLANS.md` §5.5 作業項目 1）。どの層が
/// どのコードを出すかは次の表で、**同じ形に 2 層が波線を引かない**（B-67 / B-71）。
/// 最後に同一の診断（コード・範囲・引数が同じ）を 1 つにまとめ、§9.5.2 の 9 の順
/// （`E` → `W` → `I` → 位置 → コード、`context_rules::sort_diagnostics`）に並べる。
///
/// | 出す層 | コード | 対象の形式 |
/// |---|---|---|
/// | `syntax/*` のパーサ（M2。W065 は M8） | E002 / E005（B 形式の `mode:`） / E008 / E016 / E017 / E018 / E020 / E021 / E022 / E024 / E025 / E031〜E033 / W002 / W011 / W012 / W019 / W065 | 各形式（W065 は `materials/index.json`） |
/// | [`project_json::parse`]（M5、B-88） | E025 / E027 | `project.json` |
/// | [`materials::scan_line`](crate::core::resolve::materials::scan_line)（M3、B-71） | E001 / E002 / E005（1 トークン） / E006 / E007 / E026 / W029 / W031 | 本文部のある全形式（走査範囲は B-77） |
/// | [`dictionary::speaker_diagnostic`](crate::core::resolve::dictionary::speaker_diagnostic) | E012 | C |
/// | 辞書の構築（[`Project::dictionary_diagnostics`]） | E028 / W027 | `characters.md` / `words.md` / `materials/index.json` |
/// | [`dictionary::relation_diagnostics`](crate::core::resolve::dictionary::relation_diagnostics) | E013 | `characters.md` |
/// | [`modes::resolve`](crate::core::resolve::modes::resolve)（M3、B-67） | E003 / E004 / E005（`cast:` の重複・`default-mode:`） / W006 | C（話者注釈）・B・A・`characters.md` |
/// | **この関数**（M4。W064 は M8） | W022 / W025 / W030 / W064 | C・B / 資料 MD / B / 資料 MD |
///
/// モードの検査は「**その文書に書かれた割り当て**」だけをその文書に載せる（B-79）。
/// 本文を開いて話プロットの `mode:` の E003 が出ることはない（話プロットを開けば出る）。
pub fn document(document: &Document, project: &Project) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let text = document.text.as_str();
    match document.format {
        DocumentFormat::Contents => {
            let parsed = contents::parse_with(text, &project.contents_options());
            out.extend(parsed.diagnostics.iter().cloned());
            reference_diagnostics(document, project, true, &mut out);
            contents_speakers(document, project, &parsed, &mut out);
            let scenes = project::contents_scenes(&parsed);
            ref_modes(document, project, &scenes, &mut out);
            word_ranges(document, project, &scenes, &mut out);
        }
        DocumentFormat::PlotEpisode => {
            let parsed = plot_episode::parse_with(text, &project.contents_options());
            out.extend(parsed.diagnostics.iter().cloned());
            reference_diagnostics(document, project, false, &mut out);
            episode_modes(document, project, &parsed, &mut out);
            section_counts(document, project, &parsed, &mut out);
            let scenes = project::episode_scenes(&parsed);
            ref_modes(document, project, &scenes, &mut out);
            word_ranges(document, project, &scenes, &mut out);
        }
        DocumentFormat::PlotOutline => {
            let options = project.outline_options(&document.path);
            let parsed = plot_outline::parse_with(text, &options);
            out.extend(parsed.diagnostics.iter().cloned());
            reference_diagnostics(document, project, false, &mut out);
            outline_modes(document, project, &parsed, &mut out);
            ref_modes(document, project, &[], &mut out);
        }
        DocumentFormat::Characters => {
            let parsed = characters_md::parse(text);
            out.extend(parsed.diagnostics.iter().cloned());
            out.extend(project.dictionary_diagnostics(SourceFile::Characters));
            let relations = dictionary::relation_diagnostics(&parsed, project.dictionary());
            out.extend(relations.into_iter().map(|found| found.diagnostic));
            reference_diagnostics(document, project, false, &mut out);
            ref_modes(document, project, &[], &mut out);
            default_modes(project, &parsed, &mut out);
        }
        DocumentFormat::Words => {
            out.extend(words_md::parse(text).diagnostics);
            out.extend(project.dictionary_diagnostics(SourceFile::Words));
            reference_diagnostics(document, project, false, &mut out);
        }
        DocumentFormat::Ideas => {
            out.extend(ideas_md::parse(text).diagnostics);
            reference_diagnostics(document, project, false, &mut out);
        }
        DocumentFormat::MaterialsIndex => {
            out.extend(materials_index::parse(text).diagnostics);
            out.extend(project.dictionary_diagnostics(SourceFile::MaterialsIndex));
        }
        DocumentFormat::MaterialDoc => {
            material_heading(document, project, &mut out);
            reference_diagnostics(document, project, false, &mut out);
        }
        DocumentFormat::Prompt => {
            out.extend(prompt_md::parse(text).diagnostics);
            out.extend(crate::core::agents::prompts::diagnostics(text));
        }
        DocumentFormat::SharedPrompt => {
            out.extend(prompt_md::parse(text).diagnostics);
        }
        DocumentFormat::McpConfig => {
            out.extend(crate::core::agents::mcp_config::diagnostics(text));
        }
        DocumentFormat::CustomPrompt => {
            let options = prompt_md::Options { custom: true };
            out.extend(prompt_md::parse_with(text, &options).diagnostics);
        }
        DocumentFormat::ProjectFile => {
            out.extend(project_json::parse(text).diagnostics);
        }
        _ => {}
    }
    out.extend(crate::core::stage_two::diagnose::document(
        document, project, false,
    ));
    finish(out)
}

/// 重複を落として §9.5.2 の 9 の順に並べる。
fn finish(found: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::with_capacity(found.len());
    for diagnostic in found {
        if !out.contains(&diagnostic) {
            out.push(diagnostic);
        }
    }
    // E013（relations の参照不能）と同じ場所の E001 は重ねない（同じ名前の同じ不在を 2 回言う）
    let relations: Vec<Range<usize>> = out
        .iter()
        .filter(|found| found.code == DiagnosticCode::E013)
        .filter_map(|found| found.range.clone())
        .collect();
    out.retain(|found| {
        found.code != DiagnosticCode::E001
            || !found.range.as_ref().is_some_and(|range| {
                relations
                    .iter()
                    .any(|other| range.start < other.end && other.start < range.end)
            })
    });
    context_rules::sort_diagnostics(&mut out);
    out
}

/// 本文部の `'` 参照の診断（[`materials::scan_line`] を入口にする、B-71）。
fn reference_diagnostics(
    document: &Document,
    project: &Project,
    in_body: bool,
    out: &mut Vec<Diagnostic>,
) {
    let Some(format) = document.format.text_format() else {
        return;
    };
    let index = project.materials();
    for line in project::lines_of(&document.text) {
        let keep = project::scan_ranges(line.text, format);
        if keep.is_empty() {
            continue;
        }
        let masked = project::mask(line.text, &keep);
        let scan = materials::scan_line(&masked, line.start, project.dictionary(), &index, in_body);
        out.extend(scan.diagnostics);
    }
}

/// 本文の話者注釈: **E012**（未定義のキャラ）と、モード付きの注釈の E003 / E004（段 1、§6.4.3）。
fn contents_speakers(
    document: &Document,
    project: &Project,
    parsed: &contents::Contents<'_>,
    out: &mut Vec<Diagnostic>,
) {
    let characters = project.characters();
    let starts = project::line_starts_of(&document.text);
    let scenes = project::contents_scenes(parsed);
    for (index, line) in parsed.lines.iter().enumerate() {
        let contents::Line::Text(text) = line else {
            continue;
        };
        let Some(speaker) = &text.speaker else {
            continue;
        };
        let start = starts.get(index).copied().unwrap_or(0);
        if let Some(found) = dictionary::speaker_diagnostic(speaker, project.dictionary()) {
            out.push(shifted(found, start));
            continue;
        }
        if speaker.modes.is_empty() {
            continue;
        }
        let (Some(scene), Some(character)) = (
            scenes.get(index).copied().flatten(),
            character_index(project, speaker.name),
        ) else {
            continue;
        };
        let input = modes::Input {
            characters: &characters,
            episode: None,
            chapter: None,
            all: None,
            dictionary: project.dictionary(),
        };
        let target = modes::Target {
            scene,
            character,
            speaker: Some(speaker),
            line_start: start,
        };
        let assignment = modes::resolve(&input, &target);
        keep_file(assignment.diagnostics, modes::File::Contents, out);
    }
}

/// 話プロットの `mode:` の E003 / E004 と、本文で話しているのにモードが決まらない **W006**。
///
/// 対象は (節, キャラ) の組で、キャラは「その場面・節・前文の `mode:` に書かれた人」と
/// 「対応する本文の同じ節で話している人」（B-79）。後者は話プロットに**同じ場面がある**ときだけ
/// （節が無ければ W006 の波線は場面全体、`modes::resolve` の段 8）。モードを 1 つも定義して
/// いないキャラは W006 の対象にしない（端役の台詞ごとに警告が並ぶだけになる）。
fn episode_modes(
    document: &Document,
    project: &Project,
    parsed: &plot_episode::EpisodePlot<'_>,
    out: &mut Vec<Diagnostic>,
) {
    let characters = project.characters();
    let episode = project::episode_of_path(&document.path);
    let chapter_plot = episode.and_then(|(chapter, _)| project.chapter_plot(chapter));
    let all_plot = project.all_plot();
    let body = episode.and_then(|(chapter, number)| project.episode_contents(chapter, number));
    let preamble = mode_characters(
        project,
        parsed
            .preamble
            .fields
            .iter()
            .filter_map(|line| parsed.field(*line))
            .filter(|field| field.key == FieldKey::Mode),
    );
    // (節, キャラ, 本文で話しているか)
    let mut targets: Vec<(SceneId, usize, bool)> = Vec::new();
    let mut push = |scene: SceneId, character: usize, spoken: bool| match targets
        .iter_mut()
        .find(|(seen, who, _)| *seen == scene && *who == character)
    {
        Some(found) => found.2 |= spoken,
        None => targets.push((scene, character, spoken)),
    };
    for (scene_index, scene) in parsed.scenes.iter().enumerate() {
        let Some(id) = scene.id else {
            continue;
        };
        let mut written = preamble.clone();
        written.extend(mode_characters(
            project,
            parsed.scene_fields(scene_index, FieldKey::Mode).into_iter(),
        ));
        for (section_index, section) in scene.sections.iter().enumerate() {
            let at = SceneId {
                section: section.number,
                ..id
            };
            let mut here = written.clone();
            here.extend(mode_characters(
                project,
                parsed
                    .section_fields(scene_index, section_index, FieldKey::Mode)
                    .into_iter(),
            ));
            for character in here {
                push(at, character, false);
            }
        }
    }
    if let Some(body) = &body {
        for (at, character) in speakers_by_section(project, body) {
            let planned = parsed
                .scenes
                .iter()
                .any(|scene| scene.id.is_some_and(|id| same_scene(&id, &at)));
            if planned && !modes::definitions(&characters, character).is_empty() {
                push(at, character, true);
            }
        }
    }
    let input = modes::Input {
        characters: &characters,
        episode: Some(parsed),
        chapter: chapter_plot.as_ref(),
        all: all_plot.as_ref(),
        dictionary: project.dictionary(),
    };
    for (scene, character, spoken) in targets {
        let target = modes::Target {
            scene,
            character,
            speaker: None,
            line_start: 0,
        };
        let mut found = modes::resolve(&input, &target).diagnostics;
        if !spoken {
            // 書かれた割り当ての検査だけ。話していない人の W006 は出さない
            found.retain(|item| item.diagnostic.code != DiagnosticCode::W006);
        }
        keep_file(found, modes::File::Episode, out);
    }
}

/// `mode:` / `cast:` 以外に書かれた `'名前:モード`（本文中の参照など）の E003 / E004。
///
/// `mode:` と `cast:` は 8 段の解決（[`episode_modes`] / [`outline_modes`]）が同じトークンを
/// 検査するので飛ばす。節の分からない行（A 形式・`*.md`）では E004 は出せないので E003 だけ。
fn ref_modes(
    document: &Document,
    project: &Project,
    scenes: &[Option<SceneId>],
    out: &mut Vec<Diagnostic>,
) {
    let Some(format) = document.format.text_format() else {
        return;
    };
    let characters = project.characters();
    let input = modes::Input {
        characters: &characters,
        episode: None,
        chapter: None,
        all: None,
        dictionary: project.dictionary(),
    };
    let lines: Vec<&str> = document.text.split('\n').collect();
    for site in project.ref_sites(document) {
        let Some(ids) = site.token.modifier.as_ref().and_then(|found| found.modes()) else {
            continue;
        };
        let Some(entry) = site.entry().and_then(|id| project.dictionary().entry(id)) else {
            continue;
        };
        let dictionary::Target::Character { index, .. } = &entry.target else {
            continue;
        };
        let raw = lines.get(site.line).copied().unwrap_or_default();
        if let LineKind::Field(field) = lexer::classify(raw, format) {
            if matches!(field.key, FieldKey::Mode | FieldKey::Cast) {
                continue;
            }
        }
        let scene = scenes.get(site.line).copied().flatten();
        for id in ids {
            let target = modes::Target {
                scene: scene.unwrap_or_default(),
                character: *index,
                speaker: None,
                line_start: 0,
            };
            let Some(found) = modes::check_id(&input, &target, id, site.range()) else {
                continue;
            };
            if found.code == DiagnosticCode::E004 && scene.is_none() {
                continue;
            }
            out.push(found);
        }
    }
}

/// 章プロット・全体プロットの `cast:` の E003 / E004 / E005（段 5 / 6、§6.4.3）。
fn outline_modes(
    document: &Document,
    project: &Project,
    parsed: &plot_outline::Outline<'_>,
    out: &mut Vec<Diagnostic>,
) {
    let characters = project.characters();
    let is_all = document.path == project::ALL_PLOT_PATH;
    let file = if is_all {
        modes::File::All
    } else {
        modes::File::Chapter
    };
    let input = modes::Input {
        characters: &characters,
        episode: None,
        chapter: (!is_all).then_some(parsed),
        all: is_all.then_some(parsed),
        dictionary: project.dictionary(),
    };
    for (index, block) in parsed.blocks.iter().enumerate() {
        let Some(id) = block.id else {
            continue;
        };
        let cast = mode_characters(project, parsed.fields_of(index, FieldKey::Cast).into_iter());
        for character in cast {
            let target = modes::Target {
                scene: id,
                character,
                speaker: None,
                line_start: 0,
            };
            keep_file(modes::resolve(&input, &target).diagnostics, file, out);
        }
    }
}

/// `characters.md` の `default-mode:` の E003 / E005（段 7、§6.4.3）。
///
/// 既定モードはどの節にも効くので、特定の節を前提にする E004（範囲外）は出さない。
fn default_modes(
    project: &Project,
    parsed: &characters_md::Characters<'_>,
    out: &mut Vec<Diagnostic>,
) {
    let input = modes::Input {
        characters: parsed,
        episode: None,
        chapter: None,
        all: None,
        dictionary: project.dictionary(),
    };
    for (index, _) in parsed.characters.iter().enumerate() {
        if parsed
            .character_fields(index, FieldKey::DefaultMode)
            .is_empty()
        {
            continue;
        }
        let target = modes::Target {
            scene: SceneId::default(),
            character: index,
            speaker: None,
            line_start: 0,
        };
        let mut found = modes::resolve(&input, &target).diagnostics;
        found.retain(|item| item.diagnostic.code != DiagnosticCode::E004);
        keep_file(found, modes::File::Characters, out);
    }
}

/// **W030**（§5.4.2「節数が対応する本文の節数と食い違う場合」）。
///
/// 比べるのは**本文に同じ場面があるときだけ**（プロットだけ先に書くのは正常、§5.4.2）。
/// 波線は話プロットの場面見出しの ID。
fn section_counts(
    document: &Document,
    project: &Project,
    parsed: &plot_episode::EpisodePlot<'_>,
    out: &mut Vec<Diagnostic>,
) {
    let Some((chapter, episode)) = project::episode_of_path(&document.path) else {
        return;
    };
    let Some(body) = project.episode_contents(chapter, episode) else {
        return;
    };
    let starts = project::line_starts_of(&document.text);
    for scene in &parsed.scenes {
        let Some(id) = scene.id else {
            continue;
        };
        let written = body
            .sections
            .iter()
            .filter(|section| section.scene.is_some_and(|found| same_scene(&found, &id)))
            .count();
        let planned = scene.sections.len();
        if written == 0 || written == planned {
            continue;
        }
        let Some(plot_episode::Line::Scene(header)) = parsed.lines.get(scene.header) else {
            continue;
        };
        let start = starts.get(scene.header).copied().unwrap_or(0) + header.indent;
        let range = start..start + 1 + header.id_text.len();
        out.push(Diagnostic::new(
            DiagnosticCode::W030,
            Some(range),
            vec![Arg::number(planned as i64), Arg::number(written as i64)],
        ));
    }
}

/// **W022**（§5.6「`range` を書くと、その範囲外の節で参照された場合に W022」）。
///
/// `scenes[line]` はその行が属する節（C 形式・B 形式の木から作る）。節の分からない行
/// （見出しより前など）は検査しない。
fn word_ranges(
    document: &Document,
    project: &Project,
    scenes: &[Option<SceneId>],
    out: &mut Vec<Diagnostic>,
) {
    let words = project.words();
    for site in project.ref_sites(document) {
        let Some(entry) = site.entry().and_then(|id| project.dictionary().entry(id)) else {
            continue;
        };
        let dictionary::Target::Word { index, .. } = &entry.target else {
            continue;
        };
        let (Some(scene), Some(range)) = (
            scenes.get(site.line).copied().flatten(),
            words.range(*index),
        ) else {
            continue;
        };
        if range.contains(scene) {
            continue;
        }
        let key = document
            .text
            .get(site.key_range())
            .unwrap_or_default()
            .to_string();
        out.push(Diagnostic::new(
            DiagnosticCode::W022,
            Some(site.range()),
            vec![Arg::text(key)],
        ));
    }
}

/// **W025**（§5.8.3「1 行目の H1 は `index.json` の `title` と一致していること」）。
///
/// どの資料のファイルかは `index.json` の `path` で引く。比較は §6.1.3 の正規化
/// （[`materials::same_title`](crate::core::resolve::materials::same_title)）。
fn material_heading(document: &Document, project: &Project, out: &mut Vec<Diagnostic>) {
    let index = project.materials();
    // 索引が読めない（E025）ときは、登録されているかどうかも分からないので何も言わない
    let Some(parsed) = index.index.as_ref() else {
        return;
    };
    let Some(material) = parsed
        .items
        .iter()
        .find(|item| item.path.trim_start_matches("./") == document.path)
    else {
        // §16.2「MD ファイルを手で足しただけでは資料として認識しない（W064 で通知し、ワンクリックで
        // 登録できる）」。位置は 1 行目（M8、`PLANS.md` §9.2 B-134）
        let first = project::lines_of(&document.text)
            .next()
            .map(|line| line.start..line.start + line.text.len());
        out.push(Diagnostic::new(
            DiagnosticCode::W064,
            first,
            vec![Arg::text(document.path.as_str())],
        ));
        return;
    };
    let Some(first) = project::lines_of(&document.text).next() else {
        return;
    };
    let Some(title) = first.text.strip_prefix("# ") else {
        return;
    };
    if materials::same_title(title, &material.title) {
        return;
    }
    let start = first.start + 2;
    out.push(Diagnostic::simple(
        DiagnosticCode::W025,
        Some(start..start + title.len()),
    ));
}

/// 本文の (節, 話者のキャラの添字)。
fn speakers_by_section(project: &Project, body: &contents::Contents<'_>) -> Vec<(SceneId, usize)> {
    let scenes = project::contents_scenes(body);
    let mut out = Vec::new();
    for (index, line) in body.lines.iter().enumerate() {
        let contents::Line::Text(text) = line else {
            continue;
        };
        let (Some(speaker), Some(scene)) = (&text.speaker, scenes.get(index).copied().flatten())
        else {
            continue;
        };
        if let Some(character) = character_index(project, speaker.name) {
            if !out.contains(&(scene, character)) {
                out.push((scene, character));
            }
        }
    }
    out
}

/// フィールドの `'名前` に書かれたキャラの添字（重複なし、書かれた順）。
fn mode_characters<'f, 'a: 'f>(
    project: &Project,
    fields: impl Iterator<Item = &'f plot_outline::Field<'a>>,
) -> Vec<usize> {
    let mut out = Vec::new();
    for field in fields {
        for found in field.refs() {
            if let Some(index) = character_index(project, found.name) {
                if !out.contains(&index) {
                    out.push(index);
                }
            }
        }
    }
    out
}

/// キャラ名（表示名・別名）→ `characters.md` の添字。
fn character_index(project: &Project, name: &str) -> Option<usize> {
    let id = project
        .dictionary()
        .lookup(name, Some(&[dictionary::EntryKind::Character]))?;
    match &project.dictionary().entry(id)?.target {
        dictionary::Target::Character { index, .. } => Some(*index),
        _ => None,
    }
}

/// `modes` の診断のうち `file` に載るものだけを足す。
fn keep_file(found: Vec<modes::FileDiagnostic>, file: modes::File, out: &mut Vec<Diagnostic>) {
    out.extend(
        found
            .into_iter()
            .filter(|item| item.file == file)
            .map(|item| item.diagnostic),
    );
}

/// 場面まで（章・話・場面）が同じか。
fn same_scene(left: &SceneId, right: &SceneId) -> bool {
    left.chapter == right.chapter && left.episode == right.episode && left.scene == right.scene
}

/// 行内相対の診断をファイル単位にする。
fn shifted(found: Diagnostic, by: usize) -> Diagnostic {
    Diagnostic {
        range: found.range.map(|range| range.start + by..range.end + by),
        ..found
    }
}
