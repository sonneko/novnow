//! 正規操作カタログ（§19.7）とディスパッチ（§19.1.4「単一の真実」）。
//!
//! 5 つのインターフェース（Rust・Python・LSP・MCP・CLI）はすべてこの表から操作を引く:
//!
//! - Rust API: `Session::run` / ドメインハンドル（§19.2.2）
//! - Python API: `OPS` から生成したメソッド（§19.3.2）
//! - LSP: `workspace/executeCommand`（§19.4.2）
//! - MCP: `tools/list` の `inputSchema` と `tools/call`（§19.5.3）
//! - CLI: `novnow <ドメイン> <動詞>`（§19.6.1）と `novnow api list`
//!
//! 表に無い操作はどのインターフェースからも呼べない（§19.1.3「完全な一覧は §19.7」）。

pub mod agent;
pub mod editing;
pub mod knowledge;
pub mod lang;
pub mod project;
pub mod returns;
pub mod session_ops;

use serde_json::{json, Map, Value};

use crate::app::error::{Error, Result};
use crate::app::session::{Interface, Session};
use crate::ui::i18n::{self, Locale};

/// 操作の実装（1 箇所。§19.1.4）。
pub type Handler = fn(&mut Session, &Args) -> Result<Value>;

/// 引数の定義（JSON Schema の 1 プロパティ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgDef {
    /// 引数名（JSON のキー。CLI では `--<名前>`）。
    pub name: &'static str,
    /// JSON Schema の `type`（`string` / `integer` / `boolean` / `object` / `array`）。
    pub kind: &'static str,
    /// 必須か。
    pub required: bool,
}

impl ArgDef {
    /// 必須の文字列引数。
    pub const fn string(name: &'static str) -> Self {
        Self {
            name,
            kind: "string",
            required: true,
        }
    }
    /// 任意の文字列引数。
    pub const fn opt_string(name: &'static str) -> Self {
        Self {
            name,
            kind: "string",
            required: false,
        }
    }
    /// 任意の整数引数。
    pub const fn opt_integer(name: &'static str) -> Self {
        Self {
            name,
            kind: "integer",
            required: false,
        }
    }
    /// 必須の整数引数。
    pub const fn integer(name: &'static str) -> Self {
        Self {
            name,
            kind: "integer",
            required: true,
        }
    }
    /// 任意の真偽引数。
    pub const fn opt_boolean(name: &'static str) -> Self {
        Self {
            name,
            kind: "boolean",
            required: false,
        }
    }
    /// 任意のオブジェクト引数。
    pub const fn opt_object(name: &'static str) -> Self {
        Self {
            name,
            kind: "object",
            required: false,
        }
    }
}

/// 正規操作 1 件（§19.7 の 1 行）。
#[derive(Debug, Clone, Copy)]
pub struct OpDef {
    /// 正規操作 ID（`section.get`）。
    pub id: &'static str,
    /// 引数。
    pub args: &'static [ArgDef],
    /// MCP で関門（`role` 引数）を要求するか（§19.1.5 の `M 門番`）。
    pub gate: bool,
    /// 関門が**書き込みのときだけ**掛かる場合、その引き金になる引数名（§19.7 の
    /// 「置換は M 門番」「適用は M 門番」「M 門番（書き込み時）」）。`None` なら無条件。
    ///
    /// `Some` のときは `role` は任意引数として schema に載る（読み取りだけの呼び出しに
    /// `role` を求めない。§19.7 の「テキストの返却のみは無関門」）。実行時の判定は
    /// ハンドラ側の [`gate`] 呼び出しの位置がそのまま表す（`REF:src/app/ops/editing.rs` の
    /// `search_file`、`REF:src/app/ops/lang.rs` の `code_action_op`、
    /// `REF:src/app/ops/session_ops.rs` の `export_text`）。
    pub gate_arg: Option<&'static str>,
    /// 例外列（`R` / `P` / `L` / `M` / `C` → 理由）。§19.7 の「例外」列。
    pub exceptions: &'static [(&'static str, &'static str)],
    /// 対応する LSP 標準メソッド（§19.7 の 3 列目。無ければ `None`）。
    pub lsp: Option<&'static str>,
    /// 対応するパレットコマンド名（§19.2.3。無ければ `None`）。
    pub command: Option<&'static str>,
    /// 状態を持つセッションが必要か（§19.7 の `M—・C—（状態性）`）。
    pub stateful: bool,
    /// 実装。
    pub handler: Handler,
}

impl OpDef {
    /// このインターフェースで提供しないなら、その列の名前（`M` など）。
    pub fn excepts(&self, interface: Interface) -> Option<&'static str> {
        self.exceptions
            .iter()
            .find(|(column, _)| *column == interface.column())
            .map(|(column, _)| *column)
    }

    /// ドメイン名（`section.get` の `section`）。
    pub fn domain(&self) -> &'static str {
        match self.id.split_once('.') {
            Some((domain, _)) => domain,
            None => self.id,
        }
    }

    /// 動詞（`section.get` の `get`）。
    pub fn verb(&self) -> &'static str {
        match self.id.split_once('.') {
            Some((_, verb)) => verb,
            None => "",
        }
    }

    /// MCP の `inputSchema`（§19.5.3。`api.list` から生成し、手書きの複製を置かない）。
    /// 引数の**宣言順**の名前（関門の `role` を含む。§19.3.2 の位置引数はこの順で束縛する）。
    ///
    /// `input_schema` の `properties` は `serde_json::Map`（＝ `BTreeMap`）なので**アルファベット
    /// 順に並ぶ**。位置引数を宣言順に束縛したい口（Python の `session.section.write(id, text)`）
    /// は、この列を使う（`REF:python/novnow-py/python/novnow/__init__.py`）。
    pub fn argument_order(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.args.iter().map(|arg| arg.name).collect();
        // 関門の `role` は引数としても宣言されることがある（`agent.task_create` など 26 件）。
        // その場合は 1 つに数える（`input_schema` も `properties` を上書きするだけ）
        if self.gate && !names.contains(&"role") {
            names.push("role");
        }
        names
    }

    pub fn input_schema(&self) -> Value {
        let mut properties = Map::new();
        let mut required = Vec::new();
        for arg in self.args {
            properties.insert(arg.name.to_owned(), json!({ "type": arg.kind }));
            if arg.required {
                required.push(Value::String(arg.name.to_owned()));
            }
        }
        let mut schema = json!({ "type": "object", "properties": Value::Object(properties) });
        if !required.is_empty() {
            schema["required"] = Value::Array(required);
        }
        if self.gate {
            schema["properties"]["role"] = json!({ "type": "string" });
        }
        // 関門が書き込みのときだけ掛かる操作は `role` を必須にしない（§19.7）
        if self.gate && self.gate_arg.is_none() {
            schema["required"] = match schema.get("required") {
                Some(Value::Array(items)) => {
                    let mut items = items.clone();
                    items.push(Value::String("role".to_owned()));
                    Value::Array(items)
                }
                _ => json!(["role"]),
            };
        }
        schema
    }

    /// 戻り値の形（§19.8.2 の `returns`。`REF:src/app/ops/returns.rs`）。
    ///
    /// キーは「ありうるキー」の一覧で、`required` は出さない（省略されうるキーがあるため）。
    /// `any` は型を書かない（どんな値も取りうる）。
    pub fn returns_schema(&self) -> Value {
        let mut properties = Map::new();
        for (key, kind) in returns::returns_of(self.id) {
            let schema = if *kind == "any" {
                json!({})
            } else if let Some((left, right)) = kind.split_once('|') {
                json!({ "type": [left, right] })
            } else {
                json!({ "type": kind })
            };
            properties.insert((*key).to_owned(), schema);
        }
        json!({ "type": "object", "properties": Value::Object(properties) })
    }

    /// `api.list` の 1 件（§19.8.2）。
    pub fn to_json(&self, locale: Locale) -> Value {
        let mut entry = Map::new();
        entry.insert("id".to_owned(), json!(self.id));
        entry.insert("domain".to_owned(), json!(self.domain()));
        entry.insert("verb".to_owned(), json!(self.verb()));
        entry.insert(
            "description".to_owned(),
            json!(description(locale, self.id)),
        );
        let mut schema = self.input_schema();
        // JSON のオブジェクトは順序を持たないので、位置引数の束縛順は明示する（§19.3.2）
        schema["order"] = json!(self.argument_order());
        entry.insert("arguments".to_owned(), schema);
        entry.insert("gate".to_owned(), json!(self.gate));
        // 関門が書き込みのときだけ掛かる操作は、その引き金になる引数名も出す（§19.7）。
        // 型スタブ（`REF:scripts/pyi-site.py`）と MCP の `inputSchema` が `role` を
        // 必須にするかどうかをここから決める
        entry.insert(
            "gate_arg".to_owned(),
            match self.gate_arg {
                Some(name) => json!(name),
                None => Value::Null,
            },
        );
        entry.insert("returns".to_owned(), self.returns_schema());
        entry.insert("stateful".to_owned(), json!(self.stateful));
        let mut exceptions = Map::new();
        for (column, reason) in self.exceptions {
            exceptions.insert((*column).to_owned(), json!(reason));
        }
        entry.insert("exceptions".to_owned(), Value::Object(exceptions));
        entry.insert(
            "lsp".to_owned(),
            match self.lsp {
                Some(method) => json!(method),
                None => Value::Null,
            },
        );
        entry.insert(
            "command".to_owned(),
            match self.command {
                Some(name) => json!(name),
                None => Value::Null,
            },
        );
        Value::Object(entry)
    }
}

/// 説明の i18n キー（`section.get` → `api.op.section.get`）。
pub fn description_key(id: &str) -> String {
    format!("api.op.{id}")
}

/// 説明（地域化）。
pub fn description(locale: Locale, id: &str) -> String {
    i18n::text(locale, &description_key(id)).to_owned()
}

/// `opdef!` の残り（書かなかった項目の既定。`handler` は必ず書くので使われない）。
const OP_DEFAULT: OpDef = OpDef {
    id: "",
    args: &[],
    gate: false,
    gate_arg: None,
    exceptions: &[],
    lsp: None,
    command: None,
    stateful: false,
    handler: never_called,
};

/// `OpDef::handler` の型を満たすだけの関数（`OP_DEFAULT` 専用。呼ばれない）。
fn never_called(_session: &mut Session, _args: &Args) -> Result<Value> {
    Err(Error::new(
        crate::app::error::Class::Usage,
        "op has no handler".to_owned(),
    ))
}

macro_rules! opdef {
    ($id:literal, $args:expr, $handler:path $(, $key:ident = $value:expr)*) => {
        OpDef {
            id: $id,
            args: $args,
            handler: $handler,
            $($key: $value,)*
            ..OP_DEFAULT
        }
    };
}

/// 操作の引数（JSON のオブジェクト。§19.7 の引数列）。
pub type Args = serde_json::Map<String, serde_json::Value>;

/// 引数なし。
const NONE: &[ArgDef] = &[];

/// 正規操作カタログ（§19.7 の全行。順序は仕様書の表の順）。
pub const OPS: &[OpDef] = &[
    // --- project ---
    opdef!("project.info", NONE, project::info),
    opdef!(
        "project.new",
        &[ArgDef::string("path"), ArgDef::opt_string("title")],
        project::new,
        // §19.7 の例外列: LSP はプロジェクトを開いた後、MCP はプロジェクトがある前提。
        exceptions = &[("L", "startup"), ("M", "project")]
    ),
    opdef!("project.validate", NONE, project::validate),
    opdef!(
        "project.reload",
        NONE,
        project::reload,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")],
        command = Some("reload")
    ),
    opdef!("project.stats", NONE, project::stats),
    opdef!(
        "project.recent_list",
        NONE,
        project::recent_list,
        exceptions = &[("M", "stateful")],
        stateful = true
    ),
    opdef!(
        "project.recent_remove",
        &[ArgDef::string("path")],
        project::recent_remove,
        exceptions = &[("M", "stateful")],
        command = Some("forget"),
        stateful = true
    ),
    opdef!(
        "project.import_zip",
        &[ArgDef::opt_string("path"), ArgDef::opt_string("bytes")],
        project::import_zip,
        exceptions = &[("M", "binary")]
    ),
    opdef!(
        "project.export_zip",
        &[ArgDef::opt_string("out")],
        project::export_zip,
        exceptions = &[("M", "binary")]
    ),
    // --- structure ---
    opdef!(
        "structure.tree",
        &[ArgDef::opt_string("level"), ArgDef::opt_integer("chapter")],
        project::tree,
        command = Some("tree")
    ),
    opdef!(
        "structure.resolve",
        &[ArgDef::string("query")],
        project::resolve,
        command = Some("jump")
    ),
    opdef!(
        "structure.breadcrumb",
        NONE,
        project::breadcrumb,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    // --- section ---
    opdef!(
        "section.list",
        &[ArgDef::opt_string("path")],
        editing::section_list,
        lsp = Some("textDocument/documentSymbol")
    ),
    opdef!(
        "section.get",
        &[ArgDef::string("scene")],
        editing::section_get
    ),
    opdef!(
        "section.write",
        &[ArgDef::string("scene"), ArgDef::string("text")],
        editing::section_write,
        gate = true
    ),
    opdef!(
        "section.insert",
        &[ArgDef::opt_string("after"), ArgDef::opt_string("text")],
        editing::section_insert,
        gate = true,
        command = Some("new-section")
    ),
    opdef!(
        "section.delete",
        &[ArgDef::opt_string("scene")],
        editing::section_delete,
        gate = true,
        command = Some("delete-section")
    ),
    opdef!(
        "section.confirm_next",
        NONE,
        editing::confirm_next,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")],
        command = Some("confirm-next")
    ),
    opdef!(
        "section.split",
        &[ArgDef::opt_string("scene"), ArgDef::integer("at")],
        editing::section_split,
        gate = true
    ),
    // 提案は本文を書き換えないので関門を置かない（§19.1.5）。既存の MCP ツール
    // `novnow.section.propose` と同じ関数を使う（§19.1.1。`via_mcp`）。
    opdef!(
        "section.propose",
        &[
            ArgDef::string("scene"),
            ArgDef::string("text"),
            ArgDef::opt_string("note")
        ],
        editing::section_propose
    ),
    // --- nav ---
    opdef!(
        "nav.file",
        &[ArgDef::string("path"), ArgDef::opt_string("scene")],
        editing::nav_file,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "nav.section",
        &[ArgDef::string("scene")],
        editing::nav_section,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "nav.jump",
        &[ArgDef::opt_integer("line"), ArgDef::opt_string("query")],
        editing::nav_jump,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "nav.prev",
        NONE,
        editing::nav_prev,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "nav.next",
        NONE,
        editing::nav_next,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "nav.back",
        NONE,
        editing::nav_back,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "nav.forward",
        NONE,
        editing::nav_forward,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "nav.up",
        NONE,
        editing::nav_up,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")],
        command = Some("up")
    ),
    opdef!(
        "nav.switcher",
        NONE,
        editing::nav_switcher,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    // --- document ---
    opdef!(
        "document.get",
        &[ArgDef::string("path")],
        editing::document_get
    ),
    opdef!(
        "document.write",
        &[ArgDef::string("path"), ArgDef::string("text")],
        editing::document_write,
        gate = true
    ),
    opdef!(
        "document.rename",
        &[ArgDef::string("from"), ArgDef::string("to")],
        editing::document_rename,
        gate = true,
        command = Some("rename-document")
    ),
    // --- plot ---
    opdef!(
        "plot.get",
        &[
            ArgDef::string("level"),
            ArgDef::opt_string("scene"),
            ArgDef::opt_integer("chapter")
        ],
        editing::plot_get
    ),
    opdef!(
        "plot.write",
        &[
            ArgDef::string("level"),
            ArgDef::string("text"),
            ArgDef::opt_string("scene"),
            ArgDef::opt_integer("chapter")
        ],
        editing::plot_write,
        gate = true
    ),
    opdef!(
        "plot.set_state",
        &[ArgDef::string("scene"), ArgDef::opt_boolean("state")],
        editing::plot_set_state,
        gate = true
    ),
    opdef!(
        "plot.insert_section",
        &[ArgDef::opt_string("scene"), ArgDef::opt_string("title")],
        editing::plot_insert_section,
        gate = true,
        command = Some("new-item")
    ),
    opdef!(
        "plot.outline_fold",
        &[ArgDef::integer("chapter"), ArgDef::opt_boolean("fold")],
        editing::outline_fold,
        lsp = Some("textDocument/foldingRange"),
        command = Some("fold")
    ),
    // --- save ---
    opdef!(
        "save.now",
        NONE,
        editing::save_now,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")],
        command = Some("save")
    ),
    opdef!(
        "save.state",
        NONE,
        editing::save_state,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "save.keep_mine",
        &[ArgDef::opt_string("path")],
        editing::keep_mine,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "save.reload_external",
        &[ArgDef::opt_string("path")],
        editing::reload_external,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "save.diff_external",
        &[ArgDef::opt_string("path")],
        editing::diff_external,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")],
        command = Some("diff")
    ),
    // --- search ---
    opdef!(
        "search.file",
        &[
            ArgDef::string("query"),
            ArgDef::opt_string("replace"),
            ArgDef::opt_boolean("regex")
        ],
        editing::search_file,
        // §19.7「置換は M 門番」: MCP でも**提供する**。`replace` を渡したときだけ `role` を
        // 求める（`REF:src/app/ops/editing.rs` の `search_file` がその位置で `gate` を呼ぶ）
        gate = true,
        gate_arg = Some("replace"),
        command = Some("find")
    ),
    opdef!(
        "search.replace_file",
        &[
            ArgDef::string("query"),
            ArgDef::string("replace"),
            ArgDef::opt_boolean("regex")
        ],
        editing::replace_file,
        gate = true,
        command = Some("replace")
    ),
    opdef!(
        "search.project",
        &[ArgDef::string("query"), ArgDef::opt_boolean("regex")],
        editing::search_project,
        lsp = Some("workspace/symbol"),
        command = Some("search")
    ),
    opdef!(
        "search.find_next",
        NONE,
        editing::find_next,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")],
        command = Some("find-next")
    ),
    opdef!(
        "search.find_prev",
        NONE,
        editing::find_prev,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")],
        command = Some("find-previous")
    ),
    // --- materials ---
    opdef!("materials.list", NONE, knowledge::materials_list),
    opdef!(
        "materials.get",
        &[ArgDef::string("id")],
        knowledge::materials_get
    ),
    opdef!(
        "materials.approve",
        &[ArgDef::string("id")],
        knowledge::materials_approve,
        gate = true
    ),
    opdef!(
        "materials.reject",
        &[ArgDef::string("id"), ArgDef::opt_string("note")],
        knowledge::materials_reject,
        gate = true
    ),
    opdef!(
        "materials.update",
        &[ArgDef::string("id"), ArgDef::opt_object("review")],
        knowledge::materials_update,
        gate = true
    ),
    opdef!(
        "materials.attach",
        &[
            ArgDef::string("id"),
            ArgDef::opt_string("path"),
            ArgDef::opt_string("text")
        ],
        knowledge::materials_attach,
        gate = true
    ),
    opdef!(
        "materials.search",
        &[
            ArgDef::string("query"),
            ArgDef::opt_string("category"),
            ArgDef::opt_string("scope")
        ],
        knowledge::materials_search
    ),
    opdef!(
        "materials.propose",
        &[
            ArgDef::string("title"),
            ArgDef::opt_string("source"),
            ArgDef::opt_string("markdown"),
            ArgDef::opt_string("category")
        ],
        knowledge::materials_propose
    ),
    // --- idea ---
    opdef!("idea.list", NONE, knowledge::idea_list),
    opdef!(
        "idea.capture",
        &[
            ArgDef::string("title"),
            ArgDef::opt_string("kind"),
            ArgDef::opt_string("keywords")
        ],
        knowledge::idea_capture,
        gate = true,
        command = Some("capture")
    ),
    opdef!(
        "idea.promote",
        &[
            ArgDef::string("id"),
            ArgDef::string("target"),
            ArgDef::opt_string("to")
        ],
        knowledge::idea_promote,
        gate = true
    ),
    opdef!(
        "idea.drop",
        &[ArgDef::string("id")],
        knowledge::idea_drop,
        gate = true,
        command = Some("drop")
    ),
    opdef!(
        "idea.write",
        &[ArgDef::string("id"), ArgDef::string("text")],
        knowledge::idea_write,
        gate = true
    ),
    // --- cast ---
    opdef!("cast.get", &[ArgDef::string("scene")], knowledge::cast_get),
    opdef!("cast.list", NONE, knowledge::cast_list),
    opdef!(
        "cast.timeline",
        &[ArgDef::opt_integer("index")],
        knowledge::cast_timeline
    ),
    opdef!(
        "cast.scan",
        NONE,
        knowledge::cast_scan,
        gate = true,
        command = Some("scan")
    ),
    // --- dictionary ---
    opdef!("dictionary.list", NONE, knowledge::dictionary_list),
    opdef!(
        "dictionary.register_term",
        &[ArgDef::string("term"), ArgDef::opt_string("definition")],
        knowledge::register_term,
        gate = true,
        command = Some("register-term")
    ),
    opdef!(
        "dictionary.register_character",
        &[ArgDef::string("name"), ArgDef::opt_string("reading")],
        knowledge::register_character,
        gate = true,
        command = Some("register-character")
    ),
    opdef!(
        "dictionary.term_full",
        &[ArgDef::string("query")],
        knowledge::term_full
    ),
    opdef!("dictionary.modes_list", NONE, knowledge::modes_list),
    // --- lang ---
    opdef!(
        "lang.tokens",
        &[ArgDef::opt_string("path"), ArgDef::opt_string("scene")],
        lang::tokens
    ),
    opdef!(
        "lang.completion",
        &[ArgDef::string("path"), ArgDef::integer("pos")],
        lang::completion_op,
        lsp = Some("textDocument/completion"),
        command = Some("complete")
    ),
    opdef!(
        "lang.hover",
        &[ArgDef::string("path"), ArgDef::integer("pos")],
        lang::hover_op,
        lsp = Some("textDocument/hover")
    ),
    opdef!(
        "lang.hints",
        &[
            ArgDef::string("path"),
            ArgDef::opt_integer("start"),
            ArgDef::opt_integer("end")
        ],
        lang::hints,
        lsp = Some("textDocument/inlayHint"),
        command = Some("inlay-hints")
    ),
    opdef!(
        "lang.code_action",
        &[
            ArgDef::string("path"),
            ArgDef::integer("start"),
            ArgDef::integer("end"),
            // `apply` と `index` はハンドラが読むのに宣言が無かった（§19.2.2 の
            // 機械可読性が壊れ、MCP の利用者は適用できることに気づけない）
            ArgDef::opt_boolean("apply"),
            ArgDef::opt_integer("index")
        ],
        lang::code_action_op,
        lsp = Some("textDocument/codeAction"),
        // §19.7「適用は M 門番」: 候補を列挙するだけなら無関門（`apply` のときだけ `role`）
        gate = true,
        gate_arg = Some("apply"),
        command = Some("quick-fix")
    ),
    opdef!(
        "lang.definition",
        &[ArgDef::string("path"), ArgDef::integer("pos")],
        lang::definition_op,
        lsp = Some("textDocument/definition"),
        command = Some("definition")
    ),
    opdef!(
        "lang.references",
        &[ArgDef::string("path"), ArgDef::integer("pos")],
        lang::references_op,
        lsp = Some("textDocument/references"),
        command = Some("references")
    ),
    opdef!(
        "lang.symbols",
        &[ArgDef::string("path")],
        lang::symbols,
        lsp = Some("textDocument/documentSymbol")
    ),
    opdef!(
        "lang.format",
        &[ArgDef::string("path"), ArgDef::opt_boolean("apply")],
        lang::format_op,
        lsp = Some("textDocument/formatting"),
        command = Some("format")
    ),
    opdef!(
        "lang.rename_preview",
        &[
            ArgDef::string("path"),
            ArgDef::integer("pos"),
            ArgDef::opt_string("to")
        ],
        lang::rename_preview,
        lsp = Some("textDocument/rename"),
        command = Some("rename")
    ),
    opdef!(
        "lang.rename_apply",
        &[
            ArgDef::string("path"),
            ArgDef::integer("pos"),
            ArgDef::string("to")
        ],
        lang::rename_apply,
        gate = true,
        lsp = Some("textDocument/rename")
    ),
    opdef!(
        "lang.folding",
        &[ArgDef::string("path")],
        lang::folding_op,
        lsp = Some("textDocument/foldingRange")
    ),
    // --- diagnostics ---
    opdef!(
        "diagnostics.run",
        &[
            ArgDef::opt_string("scope"),
            ArgDef::opt_string("path"),
            ArgDef::opt_string("scene")
        ],
        lang::diagnostics_run,
        lsp = Some("textDocument/publishDiagnostics")
    ),
    opdef!(
        "diagnostics.place",
        &[ArgDef::opt_string("path"), ArgDef::opt_integer("pos")],
        lang::place_op
    ),
    // --- review ---
    opdef!(
        "review.list",
        NONE,
        knowledge::review_list,
        command = Some("review")
    ),
    opdef!(
        "review.goto",
        &[ArgDef::integer("index")],
        knowledge::review_goto,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    // --- agent ---
    opdef!("agent.task_list", NONE, agent::task_list),
    opdef!(
        "agent.task_create",
        &[
            ArgDef::string("role"),
            ArgDef::opt_string("scope"),
            ArgDef::opt_string("intent"),
            ArgDef::opt_string("comment"),
            ArgDef::opt_string("examples")
        ],
        agent::task_create
    ),
    opdef!("agent.task_get", &[ArgDef::string("id")], agent::task_get),
    opdef!(
        "agent.task_run",
        &[ArgDef::string("id"), ArgDef::opt_string("role")],
        agent::task_run,
        gate = true
    ),
    opdef!(
        "agent.task_dry_run",
        &[ArgDef::string("id")],
        agent::task_dry_run
    ),
    opdef!(
        "agent.task_stop",
        &[ArgDef::string("id")],
        agent::task_stop,
        gate = true
    ),
    opdef!(
        "agent.stop_all",
        NONE,
        agent::stop_all,
        gate = true,
        command = Some("stop-all")
    ),
    opdef!("agent.proposal_list", NONE, agent::proposal_list),
    opdef!(
        "agent.proposal_get",
        &[ArgDef::string("id")],
        agent::proposal_get
    ),
    opdef!(
        "agent.proposal_apply",
        &[ArgDef::string("id")],
        agent::proposal_apply,
        gate = true
    ),
    opdef!(
        "agent.proposal_reject",
        &[ArgDef::string("id"), ArgDef::string("comment")],
        agent::proposal_reject,
        gate = true
    ),
    opdef!(
        "agent.delegation_get",
        NONE,
        agent::delegation_get,
        exceptions = &[("M", "safety")]
    ),
    opdef!(
        "agent.delegation_set",
        &[ArgDef::opt_string("role"), ArgDef::string("level")],
        agent::delegation_set,
        exceptions = &[("M", "safety")]
    ),
    opdef!(
        "agent.mcp_config_get",
        NONE,
        agent::mcp_config_get,
        command = Some("mcp")
    ),
    opdef!(
        "agent.mcp_config_validate",
        &[ArgDef::opt_string("text")],
        agent::mcp_config_validate
    ),
    opdef!(
        "agent.mcp_config_set",
        &[ArgDef::string("text")],
        agent::mcp_config_set,
        exceptions = &[("M", "safety")]
    ),
    opdef!("agent.thread_list", NONE, agent::thread_list),
    opdef!(
        "agent.thread_get",
        &[ArgDef::string("id")],
        agent::thread_get
    ),
    opdef!("agent.audit_list", NONE, agent::audit_list),
    // --- context / prompt / mail ---
    opdef!(
        "context.build",
        &[ArgDef::string("role"), ArgDef::string("scene")],
        agent::context_build
    ),
    opdef!("prompt.list", NONE, agent::prompt_list),
    opdef!("prompt.get", &[ArgDef::string("name")], agent::prompt_get),
    opdef!("prompt.diff", &[ArgDef::string("name")], agent::prompt_diff),
    opdef!(
        "prompt.reset",
        &[ArgDef::string("name")],
        agent::prompt_reset,
        exceptions = &[("M", "safety")]
    ),
    opdef!(
        "mail.send",
        &[
            ArgDef::string("to"),
            ArgDef::string("body"),
            ArgDef::opt_string("kind"),
            ArgDef::opt_string("scope")
        ],
        agent::mail_send
    ),
    // --- export / settings / session / help / api ---
    opdef!(
        "export.text",
        &[
            ArgDef::opt_string("range"),
            ArgDef::opt_string("state"),
            ArgDef::opt_string("format"),
            ArgDef::opt_string("out"),
            ArgDef::opt_boolean("blurb"),
            ArgDef::opt_boolean("blurb_only"),
            ArgDef::opt_boolean("keep_illustration_memo"),
            ArgDef::opt_boolean("stock")
        ],
        session_ops::export_text,
        // §19.7「M 門番（書き込み時。テキストの返却のみは無関門）」: `out` のときだけ `role`
        gate = true,
        gate_arg = Some("out"),
        command = Some("export")
    ),
    opdef!(
        "reference.pane",
        &[ArgDef::opt_string("scene")],
        knowledge::reference_pane
    ),
    opdef!(
        "settings.get",
        NONE,
        session_ops::settings_get,
        lsp = Some("workspace/didChangeConfiguration")
    ),
    opdef!(
        "settings.set",
        &[ArgDef::string("key"), ArgDef::opt_string("value")],
        session_ops::settings_set,
        exceptions = &[("M", "safety")]
    ),
    opdef!(
        "session.workspace_get",
        NONE,
        session_ops::workspace_get,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "session.workspace_switch",
        &[ArgDef::string("workspace")],
        session_ops::workspace_switch,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "session.recent_document",
        &[ArgDef::opt_boolean("back")],
        session_ops::recent_document,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")],
        command = Some("recent-next")
    ),
    opdef!(
        "session.command",
        &[ArgDef::string("name")],
        session_ops::command,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "session.poll_events",
        NONE,
        session_ops::poll_events,
        stateful = true,
        exceptions = &[("M", "stateful"), ("C", "stateful")]
    ),
    opdef!(
        "help.topic",
        &[ArgDef::opt_string("topic")],
        session_ops::help_topic,
        command = Some("help")
    ),
    opdef!("api.list", NONE, session_ops::api_list),
];

/// 構造 ID を書かれたままの形へ（`project.toml` の桁数に従う、§5.2）。
pub fn id_text(session: &Session, scene: crate::core::id::SceneId) -> String {
    scene.display(session.aggregate().digits())
}

/// カタログの版（§19.8.1。操作の追加はマイナー版を上げる）。
pub const CATALOG_VERSION: &str = "1.1";

/// 正規操作 ID で引く。
pub fn find(id: &str) -> Option<&'static OpDef> {
    OPS.iter().find(|def| def.id == id)
}

/// 必須の引数のうち呼び出しが渡していないもの（§19.2.3 の「名前で実行」の続きを返すのに使う）。
pub fn missing(id: &str, args: &Args) -> Vec<&'static str> {
    find(id)
        .map(|def| {
            def.args
                .iter()
                .filter(|arg| arg.required && !args.contains_key(arg.name))
                .map(|arg| arg.name)
                .collect()
        })
        .unwrap_or_default()
}

/// ドメイン名の一覧（CLI の第 1 位置引数。§19.6.1）。
pub fn domains() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for def in OPS {
        if !out.contains(&def.domain()) {
            out.push(def.domain());
        }
    }
    out
}

/// 1 つのドメインの全操作（CLI のヘルプ用）。
pub fn of_domain(domain: &str) -> Vec<&'static OpDef> {
    OPS.iter().filter(|def| def.domain() == domain).collect()
}

/// `api.list` の出力（§19.8.2。キー順はソート済みで決定論的、§19.1.4）。
pub fn catalog(locale: Locale) -> Value {
    let operations: Vec<Value> = OPS.iter().map(|def| def.to_json(locale)).collect();
    json!({
        "catalog_version": CATALOG_VERSION,
        "count": operations.len(),
        "interfaces": ["rust", "python", "lsp", "mcp", "cli"],
        "operations": operations,
        "version": env!("CARGO_PKG_VERSION"),
    })
}

/// MCP の `tools/list`（§19.5.1「`api.list` の出力と一致する」）。
///
/// `frozen` は §15.8.2 の既存 14 ツール（`REF:src/core/agents/server.rs` の `tools()` の出力を
/// **そのまま**先頭に置く＝名前・説明・スキーマの凍結、§19.8.3）。それ以外は `novnow.` + 正規
/// 操作 ID で、`inputSchema` は [`OpDef::input_schema`]（＝ `api.list` と同じ単一のカタログ）から
/// 生成する。「M—」の操作と人間の専権事項 4 件（§19.1.5）は載せない。
pub fn mcp_tools(locale: Locale, frozen: &Value) -> Value {
    let mut tools = frozen.as_array().cloned().unwrap_or_default();
    let frozen_names: Vec<String> = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str().map(str::to_owned))
        .collect();
    for def in OPS {
        let name = format!("novnow.{}", def.id);
        if frozen_names.contains(&name) || def.excepts(Interface::Mcp).is_some() {
            continue;
        }
        tools.push(json!({
            "name": name,
            "description": description(locale, def.id),
            "inputSchema": def.input_schema(),
        }));
    }
    Value::Array(tools)
}

/// MCP ツール名（`novnow.<正規操作 ID>`）から正規操作 ID を返す（§19.5.1）。
pub fn op_of_tool(name: &str) -> Option<&'static str> {
    let id = name.strip_prefix("novnow.")?;
    find(id).map(|def| def.id)
}

/// 関門（§19.1.5）: MCP からの書き込みは `role` の委譲レベルが `apply` / `autonomous` のときだけ。
///
/// 人間経路（Rust・Python・LSP・CLI）では常に `Ok`。拒否は E036（`policy`）。
pub fn gate(session: &Session, args: &Args) -> Result<Option<String>> {
    use crate::core::agents::level_of;
    use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
    use crate::core::model::delegation::Delegation;

    if session.interface != Interface::Mcp {
        return Ok(None);
    }
    // `role` は必須で、**無いときも E036**（§19.1.5・§15.8.3）。既存の 14 ツールと同じ判定を
    // 1 箇所で持つ（§19.1.4「単一の真実」）: `REF:src/core/agents/server.rs` の `direct_write`。
    let role = args
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let level = if crate::core::agents::is_role(role) {
        level_of(&session.aggregate().settings().agents, role)
    } else {
        Delegation::Off
    };
    if !level.can_write() {
        let shown = if role.is_empty() { "-" } else { role };
        let found = Diagnostic::new(DiagnosticCode::E036, None, vec![Arg::text(shown)]);
        return Err(Error::policy(
            session.locale,
            "E036",
            i18n::diagnostic(session.locale, &found),
        ));
    }
    Ok(Some(role.to_owned()))
}

/// 監査ログ（§15.8.4）を 1 行書く（§19.5.2「すべての新しい書き込みツールでも記録する」）。
///
/// `event` は §5.10.5 の `EVENTS` の 1 つ（`direct_write` など）。
pub fn audit(session: &mut Session, event: &str, detail: &str) {
    use crate::core::agents::files::LogEvent;
    let stamp = session.host.now();
    let entry = LogEvent {
        detail: Some(detail.to_owned()),
        ..LogEvent::new(&stamp, event)
    };
    let _ = session.host.append(&entry.path(), &entry.line());
}

// =========================================================================== 共通の実装部品

/// 状態を持たないインターフェースか（§19.1.6「呼び出しごとに読み直して操作し、結果を返す」）。
pub fn stateless(session: &Session) -> bool {
    matches!(session.interface, Interface::Mcp | Interface::Cli)
}

/// 書き込みを確定する: 状態を持たないインターフェースではその場で書き出し、状態を持つ
/// インターフェース（Rust / Python / LSP）では `save.now` まで未保存のまま保持する（§19.1.6）。
pub fn commit(session: &mut Session) -> Result<Value> {
    if stateless(session) {
        session.flush()
    } else {
        Ok(json!({ "state": session.save_state(), "written": Vec::<String>::new() }))
    }
}

/// 文書を書き換える（メモリ上の集約と編集状態の両方へ反映する）。
pub fn write_document(session: &mut Session, path: &str, text: String) {
    session.bench.replace_document(path, text, session.clock);
}

/// 既存の MCP ツール実装（`REF:src/core/agents/server.rs`）を呼ぶ。
///
/// §19.1.1「5 つのインターフェースは同一の機能核を共有し、同じ入力に対して同じ可視結果を
/// 返す」を保証するため、MCP が既に持つ 11 操作は**同じ関数**を使う（写しを置かない）。
pub fn via_mcp(session: &mut Session, tool: &str, args: &Args) -> Result<Value> {
    let now = session.host.now();
    let title = session.host.name();
    let value = Value::Object(args.clone());
    let outcome =
        crate::core::agents::server::call(session.aggregate(), tool, &value, &now, &title);
    apply_plan(session, &outcome.plan)?;
    match outcome.result {
        Ok(found) => Ok(found),
        Err(error) => Err(tool_error(session.locale, &error)),
    }
}

/// [`crate::core::agents::Plan`] をセッションへ適用する。
pub fn apply_plan(session: &mut Session, plan: &crate::core::agents::Plan) -> Result<()> {
    for change in &plan.writes {
        write_document(session, &change.path, change.text.clone());
    }
    for event in &plan.log {
        let _ = session.host.append(&event.path(), &event.line());
    }
    Ok(())
}

/// MCP の [`crate::core::agents::server::ToolError`] を §19.1.7 の分類へ写像する。
pub fn tool_error(locale: Locale, error: &crate::core::agents::server::ToolError) -> Error {
    use crate::core::agents::server::ToolError;
    match error {
        ToolError::Unknown(name) => Error::new(
            crate::app::error::Class::Usage,
            i18n::format(locale, "api.error.unknown_op", &[name]),
        ),
        ToolError::Argument(name) => Error::usage(locale, name),
        ToolError::Diagnostic(found) => {
            Error::validation(found.code.as_str(), i18n::diagnostic(locale, found))
        }
        ToolError::Missing(what) => Error::not_found(locale, what, None),
        ToolError::Gate { proposal, codes } => Error::validation(
            codes,
            i18n::format(locale, "api.error.gate", &[proposal, codes]),
        ),
    }
}

/// 重大度の名前（`error` / `warning` / `info`）。
pub fn severity_name(severity: crate::core::diagnostics::Severity) -> &'static str {
    use crate::core::diagnostics::Severity;
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// 診断を JSON へ（`diagnostics.run`・`lang.*`・LSP の `publishDiagnostics` が共有する形。
/// メッセージは §12.3 の地域化、行と列は 1 始まり）。
pub fn diagnostic_json(
    locale: Locale,
    path: &str,
    text: &str,
    found: &crate::core::diagnostics::Diagnostic,
) -> Value {
    let (line, column) = match found.range.as_ref() {
        Some(range) => {
            let line = crate::core::features::project::line_of(text, range.start);
            let starts = crate::core::features::project::line_starts_of(text);
            let start = starts.get(line).copied().unwrap_or(0);
            let column = text[start..range.start.min(text.len())].chars().count() + 1;
            (line + 1, column)
        }
        None => (0, 0),
    };
    json!({
        "code": found.code.as_str(),
        "column": column,
        "line": line,
        "message": i18n::diagnostic(locale, found),
        "path": path,
        "severity": severity_name(found.severity()),
    })
}
