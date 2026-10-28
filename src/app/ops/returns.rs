//! 正規操作ごとの**戻り値の形**（§19.8.2 の `returns`）。
//!
//! `api.list` の 1 件に JSON Schema として載る（[`OpDef::returns_schema`]）。表は
//! 「実際に叩いて観察した形」（`REF:examples/api_returns.rs` の出力と `REF:tests/golden/api_returns.txt`）
//! と実装の読み合わせで作り、`REF:tests/api_returns.rs` が挙動との一致を検査する。
//!
//! - 型は JSON Schema の `type`（`string` / `integer` / `number` / `boolean` / `object` / `array`）。
//! - 値が `null` になりうるものは `object|null` のように `|` で並べる。
//! - 形が呼び出しごとに変わる値（`settings.set` の `value` など）は `any`（型を書かない）。
//! - 省略されうるキーは載せてよい（`required` は出さない。戻り値は「ありうるキー」の一覧）。

/// 1 操作ぶんの戻り値のキーと型。
pub type Returns = &'static [(&'static str, &'static str)];

/// 正規操作 ID → 戻り値の形（ID の昇順。全 121 操作）。
pub const RETURNS: &[(&str, Returns)] = &[
    (
        "agent.audit_list",
        &[("count", "integer"), ("events", "array")],
    ),
    (
        "agent.delegation_get",
        &[
            ("enabled", "boolean"),
            ("levels", "array"),
            ("max_parallel", "integer"),
        ],
    ),
    (
        "agent.delegation_set",
        &[
            ("level", "string"),
            ("message", "string"),
            ("role", "string"),
            ("save", "object"),
        ],
    ),
    (
        "agent.mcp_config_get",
        &[("path", "string"), ("servers", "array"), ("text", "string")],
    ),
    (
        "agent.mcp_config_set",
        &[("chars", "integer"), ("path", "string"), ("save", "object")],
    ),
    (
        "agent.mcp_config_validate",
        &[
            ("count", "integer"),
            ("diagnostics", "array"),
            ("valid", "boolean"),
        ],
    ),
    (
        "agent.proposal_apply",
        &[("applied", "string"), ("save", "object")],
    ),
    (
        "agent.proposal_get",
        &[
            ("after", "string"),
            ("after_chars", "integer"),
            ("before", "string"),
            ("before_chars", "integer"),
            ("comment", "string"),
            ("created_at", "string"),
            ("drifted", "boolean"),
            ("id", "string"),
            ("kind", "string"),
            ("note", "string"),
            ("origin", "string"),
            ("partial", "boolean"),
            ("status", "string"),
            ("target", "string"),
            ("updated_at", "string"),
        ],
    ),
    (
        "agent.proposal_list",
        &[("count", "integer"), ("proposals", "array")],
    ),
    (
        "agent.proposal_reject",
        &[
            ("comment", "string"),
            ("rejected", "string"),
            ("save", "object"),
        ],
    ),
    (
        "agent.stop_all",
        &[("message", "string"), ("save", "object")],
    ),
    (
        "agent.task_create",
        &[
            ("created", "array"),
            ("role", "string"),
            ("save", "object"),
            ("scope", "string"),
        ],
    ),
    (
        "agent.task_dry_run",
        &[
            ("chars", "integer"),
            ("path", "string"),
            ("save", "object"),
            ("task", "object"),
        ],
    ),
    (
        "agent.task_get",
        &[
            ("proposal", "object|null"),
            ("result", "object|null"),
            ("task", "object"),
        ],
    ),
    (
        "agent.task_list",
        &[("count", "integer"), ("tasks", "array")],
    ),
    (
        "agent.task_run",
        &[("notices", "array"), ("queued", "boolean")],
    ),
    (
        "agent.task_stop",
        &[("save", "object"), ("stopped", "string")],
    ),
    (
        "agent.thread_get",
        &[
            ("id", "string"),
            ("messages", "array"),
            ("scope", "string"),
            ("status", "string"),
            ("title", "string"),
        ],
    ),
    (
        "agent.thread_list",
        &[("count", "integer"), ("threads", "array")],
    ),
    (
        "api.list",
        &[
            ("catalog_version", "string"),
            ("count", "integer"),
            ("interfaces", "array"),
            ("operations", "array"),
            ("version", "string"),
        ],
    ),
    ("cast.get", &[("cast", "string")]),
    (
        "cast.list",
        &[("characters", "array"), ("count", "integer")],
    ),
    ("cast.scan", &[("count", "integer"), ("unmatched", "array")]),
    (
        "cast.timeline",
        &[
            ("bands", "array"),
            ("chapters", "array"),
            ("index", "integer"),
        ],
    ),
    (
        "context.build",
        &[
            ("cap", "integer"),
            ("chars", "integer"),
            ("packet", "string"),
        ],
    ),
    (
        "diagnostics.place",
        &[("path", "string"), ("place", "object")],
    ),
    (
        "diagnostics.run",
        &[
            ("count", "integer"),
            ("diagnostics", "array"),
            ("errors", "integer"),
        ],
    ),
    (
        "dictionary.list",
        &[("count", "integer"), ("entries", "array")],
    ),
    (
        "dictionary.modes_list",
        &[("count", "integer"), ("modes", "array")],
    ),
    (
        "dictionary.register_character",
        &[
            ("registered", "string"),
            ("save", "object"),
            ("written", "array"),
        ],
    ),
    (
        "dictionary.register_term",
        &[
            ("registered", "string"),
            ("save", "object"),
            ("written", "array"),
        ],
    ),
    ("dictionary.term_full", &[("words", "array")]),
    (
        "document.get",
        &[("format", "string"), ("path", "string"), ("text", "string")],
    ),
    (
        "document.rename",
        &[("from", "string"), ("save", "object"), ("to", "string")],
    ),
    (
        "document.write",
        &[("chars", "integer"), ("path", "string"), ("save", "object")],
    ),
    (
        "export.text",
        &[
            ("chars", "integer"),
            ("name", "string"),
            ("out", "string"),
            ("range", "string"),
            ("sections", "integer"),
        ],
    ),
    (
        "help.topic",
        &[("text", "string"), ("title", "string"), ("topic", "string")],
    ),
    (
        "idea.capture",
        &[
            ("captured", "string"),
            ("index", "integer"),
            ("save", "object"),
        ],
    ),
    ("idea.drop", &[("dropped", "string"), ("save", "object")]),
    ("idea.list", &[("count", "integer"), ("ideas", "array")]),
    (
        "idea.promote",
        &[
            ("promoted", "string"),
            ("save", "object"),
            ("target", "string"),
            ("written", "array"),
        ],
    ),
    (
        "idea.write",
        &[
            ("chars", "integer"),
            ("save", "object"),
            ("written", "array"),
        ],
    ),
    (
        "lang.code_action",
        &[
            ("actions", "array"),
            ("count", "integer"),
            ("path", "string"),
        ],
    ),
    (
        "lang.completion",
        &[("count", "integer"), ("items", "array"), ("path", "string")],
    ),
    (
        "lang.definition",
        &[("definition", "object|null"), ("path", "string")],
    ),
    (
        "lang.folding",
        &[
            ("count", "integer"),
            ("path", "string"),
            ("ranges", "array"),
        ],
    ),
    (
        "lang.format",
        &[("count", "integer"), ("edits", "array"), ("path", "string")],
    ),
    (
        "lang.hints",
        &[("count", "integer"), ("hints", "array"), ("path", "string")],
    ),
    (
        "lang.hover",
        &[("hover", "object|null"), ("path", "string")],
    ),
    (
        "lang.references",
        &[
            ("count", "integer"),
            ("path", "string"),
            ("references", "array"),
        ],
    ),
    (
        "lang.rename_apply",
        &[
            ("renamed", "array"),
            ("save", "object"),
            ("written", "array"),
        ],
    ),
    (
        "lang.rename_preview",
        &[
            ("changes", "array"),
            ("edits", "array"),
            ("error", "string|null"),
            ("path", "string"),
            ("prepared", "object"),
        ],
    ),
    (
        "lang.symbols",
        &[
            ("count", "integer"),
            ("path", "string"),
            ("symbols", "array"),
        ],
    ),
    (
        "lang.tokens",
        &[
            ("count", "integer"),
            ("path", "string"),
            ("tokens", "array"),
        ],
    ),
    (
        "mail.send",
        &[
            ("approved_by", "string"),
            ("message", "integer"),
            ("thread", "string"),
        ],
    ),
    (
        "materials.approve",
        &[
            ("already", "boolean"),
            ("approved", "string"),
            ("id", "string"),
            ("renamed", "string|null"),
            ("save", "object"),
        ],
    ),
    (
        "materials.attach",
        &[
            ("attached", "string"),
            ("path", "string"),
            ("save", "object"),
        ],
    ),
    (
        "materials.get",
        &[
            ("body", "string"),
            ("material", "object"),
            ("review", "object"),
        ],
    ),
    (
        "materials.list",
        &[("count", "integer"), ("materials", "array")],
    ),
    (
        "materials.propose",
        &[
            ("body", "string"),
            ("id", "string"),
            ("path", "string"),
            ("save", "object"),
        ],
    ),
    (
        "materials.reject",
        &[
            ("note", "string"),
            ("rejected", "string"),
            ("save", "object"),
        ],
    ),
    (
        "materials.search",
        &[("count", "integer"), ("hits", "array"), ("query", "string")],
    ),
    (
        "materials.update",
        &[("save", "object"), ("updated", "string")],
    ),
    ("nav.back", &[("moved", "boolean"), ("scene", "string")]),
    ("nav.file", &[("path", "string"), ("scene", "string")]),
    ("nav.forward", &[("moved", "boolean")]),
    (
        "nav.jump",
        &[("byte", "integer"), ("line", "integer"), ("path", "string")],
    ),
    ("nav.next", &[("moved", "boolean")]),
    ("nav.prev", &[("moved", "boolean")]),
    ("nav.section", &[("path", "string"), ("scene", "string")]),
    (
        "nav.switcher",
        &[
            ("current", "string"),
            ("previous", "string|null"),
            ("workspaces", "array"),
        ],
    ),
    ("nav.up", &[("moved", "boolean"), ("workspace", "string")]),
    ("plot.get", &[("blocks", "array"), ("level", "string")]),
    (
        "plot.insert_section",
        &[("path", "string"), ("save", "object"), ("scene", "string")],
    ),
    (
        "plot.outline_fold",
        &[
            ("chapter", "integer"),
            ("fold", "boolean"),
            ("folded", "array"),
        ],
    ),
    (
        "plot.set_state",
        &[
            ("path", "string"),
            ("save", "object"),
            ("scene", "string"),
            ("state", "string"),
        ],
    ),
    (
        "plot.write",
        &[
            ("chars", "integer"),
            ("level", "string"),
            ("path", "string"),
            ("save", "object"),
        ],
    ),
    (
        "project.export_zip",
        &[
            ("bytes", "string"),
            ("name", "string"),
            ("out", "string"),
            ("size", "integer"),
        ],
    ),
    (
        "project.import_zip",
        &[("skipped", "array"), ("written", "array")],
    ),
    (
        "project.info",
        &[
            ("chapters", "integer"),
            ("chars", "integer"),
            ("episodes", "integer"),
            ("scenes", "integer"),
            ("sections", "integer"),
            ("title", "string"),
        ],
    ),
    (
        "project.new",
        &[("files", "array"), ("path", "string"), ("title", "string")],
    ),
    ("project.recent_list", &[("projects", "array")]),
    (
        "project.recent_remove",
        &[("projects", "integer"), ("removed", "string")],
    ),
    (
        "project.reload",
        &[("files", "integer"), ("reloaded", "boolean")],
    ),
    (
        "project.stats",
        &[
            ("chapters", "integer"),
            ("dialogue_chars", "integer"),
            ("dialogue_ratio", "number"),
            ("newline", "string"),
            ("sections", "integer"),
            ("speakers", "array"),
            ("title", "string"),
            ("total_chars", "integer"),
        ],
    ),
    (
        "project.validate",
        &[
            ("diagnostics", "array"),
            ("errors", "integer"),
            ("ok", "boolean"),
            ("structure", "object|null"),
        ],
    ),
    (
        "prompt.diff",
        &[
            ("count", "integer"),
            ("edited", "boolean"),
            ("lines", "array"),
            ("name", "string"),
            ("path", "string"),
        ],
    ),
    (
        "prompt.get",
        &[
            ("blockers", "array"),
            ("edited", "boolean"),
            ("name", "string"),
            ("path", "string"),
            ("shared", "string"),
            ("text", "string"),
        ],
    ),
    ("prompt.list", &[("count", "integer"), ("prompts", "array")]),
    (
        "prompt.reset",
        &[("name", "string"), ("reset", "integer"), ("save", "object")],
    ),
    (
        "reference.pane",
        &[
            ("cast", "array"),
            ("diagnostics", "array"),
            ("illustrations", "array"),
            ("materials", "array"),
            ("plot", "array"),
            ("plot_nav", "object"),
            ("scene", "string"),
            ("senses", "array"),
            ("space", "array"),
            ("upper", "array"),
            ("words", "array"),
        ],
    ),
    (
        "review.goto",
        &[
            ("byte", "integer"),
            ("code", "string"),
            ("index", "integer"),
            ("path", "string"),
        ],
    ),
    (
        "review.list",
        &[("count", "integer"), ("diagnostics", "array")],
    ),
    ("save.diff_external", &[("diffs", "array")]),
    ("save.keep_mine", &[("kept", "array"), ("save", "object")]),
    ("save.now", &[("state", "string"), ("written", "array")]),
    ("save.reload_external", &[("reloaded", "array")]),
    (
        "save.state",
        &[
            ("conflicts", "array"),
            ("state", "string"),
            ("unsaved", "array"),
        ],
    ),
    (
        "search.file",
        &[
            ("count", "integer"),
            ("path", "string"),
            ("replaced", "boolean"),
            ("save", "object"),
        ],
    ),
    (
        "search.find_next",
        &[("range", "array|null"), ("selected", "integer|null")],
    ),
    (
        "search.find_prev",
        &[("range", "array|null"), ("selected", "integer|null")],
    ),
    (
        "search.project",
        &[("count", "integer"), ("hits", "array"), ("query", "string")],
    ),
    (
        "search.replace_file",
        &[("count", "integer"), ("path", "string"), ("save", "object")],
    ),
    (
        "section.confirm_next",
        &[("confirmed", "boolean"), ("scene", "string")],
    ),
    (
        "section.delete",
        &[("deleted", "string"), ("save", "object")],
    ),
    (
        "section.get",
        &[
            ("after", "array"),
            ("before", "array"),
            ("path", "string"),
            ("scene", "string"),
            ("text", "string"),
        ],
    ),
    (
        "section.insert",
        &[("id", "string"), ("pending", "boolean"), ("save", "object")],
    ),
    (
        "section.list",
        &[("count", "integer"), ("sections", "array")],
    ),
    (
        "section.propose",
        &[("proposal", "string"), ("status", "string")],
    ),
    (
        "section.split",
        &[
            ("save", "object"),
            ("scene", "string"),
            ("split", "boolean"),
        ],
    ),
    (
        "section.write",
        &[
            ("chars", "integer"),
            ("path", "string"),
            ("save", "object"),
            ("scene", "string"),
        ],
    ),
    (
        "session.command",
        &[
            ("command", "string"),
            ("data", "any"),
            ("gui_only", "boolean"),
            ("key", "string"),
            ("label", "string"),
            ("moved", "boolean"),
            ("op", "string"),
            ("scene", "string"),
            ("value", "any"),
            ("workspace", "string"),
        ],
    ),
    (
        "session.poll_events",
        &[("count", "integer"), ("events", "array")],
    ),
    (
        "session.recent_document",
        &[("moved", "boolean"), ("recent", "array")],
    ),
    (
        "session.workspace_get",
        &[
            ("current", "string"),
            ("previous", "string|null"),
            ("workspaces", "array"),
        ],
    ),
    (
        "session.workspace_switch",
        &[("current", "string"), ("moved", "boolean")],
    ),
    (
        "settings.get",
        &[
            ("language", "string"),
            ("path", "string"),
            ("settings", "object"),
        ],
    ),
    (
        "settings.set",
        &[("key", "string"), ("language", "string"), ("value", "any")],
    ),
    (
        "structure.breadcrumb",
        &[("breadcrumb", "string"), ("scene", "string")],
    ),
    (
        "structure.resolve",
        &[
            ("id", "string"),
            ("lines", "array"),
            ("path", "string"),
            ("resolved", "boolean"),
            ("title", "string"),
        ],
    ),
    ("structure.tree", &[("level", "string"), ("nodes", "array")]),
];

/// 戻り値の形を引く（宣言が無ければ空）。
pub fn returns_of(id: &str) -> Returns {
    RETURNS
        .binary_search_by(|(name, _)| (*name).cmp(id))
        .map(|index| RETURNS[index].1)
        .unwrap_or(&[])
}
