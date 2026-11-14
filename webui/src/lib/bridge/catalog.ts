// このファイルは **生成物**。手で編集しない（`scripts/api-ts.py` が書く）。
//
// 正本は `REF:tests/golden/api_catalog.json`（§19.7）。第 1 の画面実装・LSP・MCP・Python・
// CLI と同じカタログから作るので、操作 ID の綴りと引数の形が 2 系統でずれない
// （`REF:docs/plans.md#webui-w3`・`REF:docs/designs/webui.md` §21.5）。
//
// 変更するときは `python3 scripts/api-ts.py` を実行する（`scripts/verify.sh` の
// `webui-api` が `--check` で一致を強制する）。

export const CATALOG_VERSION = "1.1";
export const PRODUCT_VERSION = "0.1.1";
/** カタログの操作 ID の全部（件数は `OP_IDS.length` で数える）。 */
export const OP_IDS = [
  "project.info", // プロジェクト基本情報（タイトル・作者・言語・版）
  "project.new", // 新規プロジェクト作成
  "project.validate", // 構造検証
  "project.reload", // 再読込
  "project.stats", // 執筆統計
  "project.recent_list", // 最近開いたプロジェクト一覧
  "project.recent_remove", // 最近開いた一覧からの削除
  "project.import_zip", // Zip 導入
  "project.export_zip", // Zip 書き出し
  "structure.tree", // 構成ツリー
  "structure.resolve", // 位置・ID の解決
  "structure.breadcrumb", // パンくず
  "section.list", // セクション一覧
  "section.get", // セクション取得
  "section.write", // セクション保存
  "section.insert", // 節の新設
  "section.delete", // 節の削除
  "section.confirm_next", // 次の未確認節の確認
  "section.split", // 節の分割
  "section.propose", // 節の提案（本文は変えない）
  "nav.file", // 文書・ファイルの直接指定
  "nav.section", // セクションへの移動
  "nav.jump", // 行への移動
  "nav.prev", // 前の位置
  "nav.next", // 次の位置
  "nav.back", // 親ディレクトリへ
  "nav.forward", // 子の先頭へ
  "nav.up", // 1 つ上の階層へ
  "nav.switcher", // 切替パネルの一覧
  "document.get", // ドキュメント全文の取得
  "document.write", // ドキュメント全文の書き込み
  "document.rename", // 文書のリネーム
  "plot.get", // 全体／章プロットの取得
  "plot.write", // 全体／章／話プロットの書き込み
  "plot.set_state", // プロットの確認状態の切替
  "plot.insert_section", // 話プロットへの節挿入
  "plot.outline_fold", // 構成ツリーの章→話の展開・折り畳み
  "save.now", // 保存
  "save.state", // 保存状態の取得
  "save.keep_mine", // 衝突時に自分の版を維持
  "save.reload_external", // 外部変更の読み込み
  "save.diff_external", // 自分の版と外部変更の差分
  "search.file", // 検索／置換
  "search.replace_file", // 全置換
  "search.project", // 全体検索
  "search.find_next", // 次の候補
  "search.find_prev", // 前の候補
  "materials.list", // 素材インボックス一覧
  "materials.get", // 素材カードの取得
  "materials.approve", // 素材の採用
  "materials.reject", // 素材の却下
  "materials.update", // 素材カードの編集
  "materials.attach", // 素材の添付
  "materials.search", // 素材の検索
  "materials.propose", // 素材の提案
  "idea.list", // 着想一覧
  "idea.capture", // 着想の記録
  "idea.promote", // 着想の昇格
  "idea.drop", // 着想の削除
  "idea.write", // 着想メモの書き込み
  "cast.get", // キャラクター情報・有効なモード
  "cast.list", // キャラクター一覧
  "cast.timeline", // 呼称タイムライン
  "cast.scan", // 本文スキャンで候補抽出
  "dictionary.list", // 辞書の一覧
  "dictionary.register_term", // 用語の辞書登録
  "dictionary.register_character", // キャラクターの辞書登録
  "dictionary.term_full", // 用語の完全情報
  "dictionary.modes_list", // 全モード一覧
  "lang.tokens", // 字句解析
  "lang.completion", // 補完候補
  "lang.hover", // カーソル位置の情報
  "lang.hints", // インラインヒント
  "lang.code_action", // クイックフィックス候補
  "lang.definition", // 定義へジャンプ
  "lang.references", // 参照検索
  "lang.symbols", // シンボル一覧
  "lang.format", // 整形
  "lang.rename_preview", // リネームの影響範囲
  "lang.rename_apply", // リネームの実行
  "lang.folding", // 折り畳み範囲
  "diagnostics.run", // 診断
  "diagnostics.place", // 地名解決
  "review.list", // 見直し項目一覧
  "review.goto", // 見直し項目への移動
  "agent.task_list", // タスク一覧
  "agent.task_create", // タスクの起票
  "agent.task_get", // タスク 1 件の詳細
  "agent.task_run", // タスクの実行
  "agent.task_dry_run", // ドライラン
  "agent.task_stop", // タスクの中止
  "agent.stop_all", // 全実行の中止
  "agent.proposal_list", // 提案一覧
  "agent.proposal_get", // 提案 1 件の内容
  "agent.proposal_apply", // 提案の適用
  "agent.proposal_reject", // 提案の却下
  "agent.delegation_get", // 委譲レベルの参照
  "agent.delegation_set", // 委譲レベルの変更
  "agent.mcp_config_get", // MCP 接続設定の参照
  "agent.mcp_config_validate", // MCP 接続設定の検証
  "agent.mcp_config_set", // MCP 接続設定の書き込み
  "agent.thread_list", // メールスレッド一覧
  "agent.thread_get", // スレッド 1 件の内容
  "agent.audit_list", // 監査ログ一覧
  "context.build", // コンテキストパケットの構築
  "prompt.list", // プロンプトファイル一覧
  "prompt.get", // プロンプト内容の取得
  "prompt.diff", // 初期版との差分
  "prompt.reset", // プロンプトの初期化
  "mail.send", // エージェント間のメール
  "export.text", // テキストのエクスポート
  "reference.pane", // 参照ペインの全セクション
  "settings.get", // 設定の取得
  "settings.set", // 設定の変更
  "session.workspace_get", // 現在のワークスペース
  "session.workspace_switch", // ワークスペースの切替
  "session.recent_document", // 最近開いた文書の循環
  "session.command", // パレットコマンドの実行
  "session.poll_events", // イベントキューの取得
  "help.topic", // ヘルプ本文
  "api.list", // 正規操作カタログの機械可読出力
] as const;

/** 操作 ID の型（`run(op, args)` の第 1 引数）。 */
export type OpId = (typeof OP_IDS)[number];

/** 操作 1 つの引数の形（`run` の第 2 引数）。 */
export interface OpArgs {
  "project.info": Record<string, never>;
  "project.new": {
    "path": string;
    "title"?: string;
  };
  "project.validate": Record<string, never>;
  "project.reload": Record<string, never>;
  "project.stats": Record<string, never>;
  "project.recent_list": Record<string, never>;
  "project.recent_remove": {
    "path": string;
  };
  "project.import_zip": {
    "bytes"?: string;
    "path"?: string;
  };
  "project.export_zip": {
    "out"?: string;
  };
  "structure.tree": {
    "chapter"?: number;
    "level"?: string;
  };
  "structure.resolve": {
    "query": string;
  };
  "structure.breadcrumb": Record<string, never>;
  "section.list": {
    "path"?: string;
  };
  "section.get": {
    "scene": string;
  };
  "section.write": {
    "role": string;
    "scene": string;
    "text": string;
  };
  "section.insert": {
    "after"?: string;
    "role": string;
    "text"?: string;
  };
  "section.delete": {
    "role": string;
    "scene"?: string;
  };
  "section.confirm_next": Record<string, never>;
  "section.split": {
    "at": number;
    "role": string;
    "scene"?: string;
  };
  "section.propose": {
    "note"?: string;
    "scene": string;
    "text": string;
  };
  "nav.file": {
    "path": string;
    "scene"?: string;
  };
  "nav.section": {
    "scene": string;
  };
  "nav.jump": {
    "line"?: number;
    "query"?: string;
  };
  "nav.prev": Record<string, never>;
  "nav.next": Record<string, never>;
  "nav.back": Record<string, never>;
  "nav.forward": Record<string, never>;
  "nav.up": Record<string, never>;
  "nav.switcher": Record<string, never>;
  "document.get": {
    "path": string;
  };
  "document.write": {
    "path": string;
    "role": string;
    "text": string;
  };
  "document.rename": {
    "from": string;
    "role": string;
    "to": string;
  };
  "plot.get": {
    "chapter"?: number;
    "level": string;
    "scene"?: string;
  };
  "plot.write": {
    "chapter"?: number;
    "level": string;
    "role": string;
    "scene"?: string;
    "text": string;
  };
  "plot.set_state": {
    "role": string;
    "scene": string;
    "state"?: boolean;
  };
  "plot.insert_section": {
    "role": string;
    "scene"?: string;
    "title"?: string;
  };
  "plot.outline_fold": {
    "chapter": number;
    "fold"?: boolean;
  };
  "save.now": Record<string, never>;
  "save.state": Record<string, never>;
  "save.keep_mine": {
    "path"?: string;
  };
  "save.reload_external": {
    "path"?: string;
  };
  "save.diff_external": {
    "path"?: string;
  };
  "search.file": {
    "query": string;
    "regex"?: boolean;
    "replace"?: string;
    "role"?: string;
  };
  "search.replace_file": {
    "query": string;
    "regex"?: boolean;
    "replace": string;
    "role": string;
  };
  "search.project": {
    "query": string;
    "regex"?: boolean;
  };
  "search.find_next": Record<string, never>;
  "search.find_prev": Record<string, never>;
  "materials.list": Record<string, never>;
  "materials.get": {
    "id": string;
  };
  "materials.approve": {
    "id": string;
    "role": string;
  };
  "materials.reject": {
    "id": string;
    "note"?: string;
    "role": string;
  };
  "materials.update": {
    "id": string;
    "review"?: Record<string, unknown>;
    "role": string;
  };
  "materials.attach": {
    "id": string;
    "path"?: string;
    "role": string;
    "text"?: string;
  };
  "materials.search": {
    "category"?: string;
    "query": string;
    "scope"?: string;
  };
  "materials.propose": {
    "category"?: string;
    "markdown"?: string;
    "source"?: string;
    "title": string;
  };
  "idea.list": Record<string, never>;
  "idea.capture": {
    "keywords"?: string;
    "kind"?: string;
    "role": string;
    "title": string;
  };
  "idea.promote": {
    "id": string;
    "role": string;
    "target": string;
    "to"?: string;
  };
  "idea.drop": {
    "id": string;
    "role": string;
  };
  "idea.write": {
    "id": string;
    "role": string;
    "text": string;
  };
  "cast.get": {
    "scene": string;
  };
  "cast.list": Record<string, never>;
  "cast.timeline": {
    "index"?: number;
  };
  "cast.scan": {
    "role": string;
  };
  "dictionary.list": Record<string, never>;
  "dictionary.register_term": {
    "definition"?: string;
    "role": string;
    "term": string;
  };
  "dictionary.register_character": {
    "name": string;
    "reading"?: string;
    "role": string;
  };
  "dictionary.term_full": {
    "query": string;
  };
  "dictionary.modes_list": Record<string, never>;
  "lang.tokens": {
    "path"?: string;
    "scene"?: string;
  };
  "lang.completion": {
    "path": string;
    "pos": number;
  };
  "lang.hover": {
    "path": string;
    "pos": number;
  };
  "lang.hints": {
    "end"?: number;
    "path": string;
    "start"?: number;
  };
  "lang.code_action": {
    "apply"?: boolean;
    "end": number;
    "index"?: number;
    "path": string;
    "role"?: string;
    "start": number;
  };
  "lang.definition": {
    "path": string;
    "pos": number;
  };
  "lang.references": {
    "path": string;
    "pos": number;
  };
  "lang.symbols": {
    "path": string;
  };
  "lang.format": {
    "apply"?: boolean;
    "path": string;
  };
  "lang.rename_preview": {
    "path": string;
    "pos": number;
    "to"?: string;
  };
  "lang.rename_apply": {
    "path": string;
    "pos": number;
    "role": string;
    "to": string;
  };
  "lang.folding": {
    "path": string;
  };
  "diagnostics.run": {
    "path"?: string;
    "scene"?: string;
    "scope"?: string;
  };
  "diagnostics.place": {
    "path"?: string;
    "pos"?: number;
  };
  "review.list": Record<string, never>;
  "review.goto": {
    "index": number;
  };
  "agent.task_list": Record<string, never>;
  "agent.task_create": {
    "comment"?: string;
    "examples"?: string;
    "intent"?: string;
    "role": string;
    "scope"?: string;
  };
  "agent.task_get": {
    "id": string;
  };
  "agent.task_run": {
    "id": string;
    "role": string;
  };
  "agent.task_dry_run": {
    "id": string;
  };
  "agent.task_stop": {
    "id": string;
    "role": string;
  };
  "agent.stop_all": {
    "role": string;
  };
  "agent.proposal_list": Record<string, never>;
  "agent.proposal_get": {
    "id": string;
  };
  "agent.proposal_apply": {
    "id": string;
    "role": string;
  };
  "agent.proposal_reject": {
    "comment": string;
    "id": string;
    "role": string;
  };
  "agent.delegation_get": Record<string, never>;
  "agent.delegation_set": {
    "level": string;
    "role"?: string;
  };
  "agent.mcp_config_get": Record<string, never>;
  "agent.mcp_config_validate": {
    "text"?: string;
  };
  "agent.mcp_config_set": {
    "text": string;
  };
  "agent.thread_list": Record<string, never>;
  "agent.thread_get": {
    "id": string;
  };
  "agent.audit_list": Record<string, never>;
  "context.build": {
    "role": string;
    "scene": string;
  };
  "prompt.list": Record<string, never>;
  "prompt.get": {
    "name": string;
  };
  "prompt.diff": {
    "name": string;
  };
  "prompt.reset": {
    "name": string;
  };
  "mail.send": {
    "body": string;
    "kind"?: string;
    "scope"?: string;
    "to": string;
  };
  "export.text": {
    "blurb"?: boolean;
    "blurb_only"?: boolean;
    "format"?: string;
    "keep_illustration_memo"?: boolean;
    "out"?: string;
    "range"?: string;
    "role"?: string;
    "state"?: string;
    "stock"?: boolean;
  };
  "reference.pane": {
    "scene"?: string;
  };
  "settings.get": Record<string, never>;
  "settings.set": {
    "key": string;
    "value"?: string;
  };
  "session.workspace_get": Record<string, never>;
  "session.workspace_switch": {
    "workspace": string;
  };
  "session.recent_document": {
    "back"?: boolean;
  };
  "session.command": {
    "name": string;
  };
  "session.poll_events": Record<string, never>;
  "help.topic": {
    "topic"?: string;
  };
  "api.list": Record<string, never>;
}

/** 操作 1 つの戻り値の形（`run` の `ok: true` のときの `result`）。 */
export interface OpResult {
  "project.info": {
    "chapters": number;
    "chars": number;
    "episodes": number;
    "scenes": number;
    "sections": number;
    "title": string;
  };
  "project.new": {
    "files": unknown[];
    "path": string;
    "title": string;
  };
  "project.validate": {
    "diagnostics": unknown[];
    "errors": number;
    "ok": boolean;
    "structure": Record<string, unknown> | null;
  };
  "project.reload": {
    "files": number;
    "reloaded": boolean;
  };
  "project.stats": {
    "chapters": number;
    "dialogue_chars": number;
    "dialogue_ratio": number;
    "newline": string;
    "sections": number;
    "speakers": unknown[];
    "title": string;
    "total_chars": number;
  };
  "project.recent_list": {
    "projects": unknown[];
  };
  "project.recent_remove": {
    "projects": number;
    "removed": string;
  };
  "project.import_zip": {
    "skipped": unknown[];
    "written": unknown[];
  };
  "project.export_zip": {
    "bytes": string;
    "name": string;
    "out": string;
    "size": number;
  };
  "structure.tree": {
    "level": string;
    "nodes": unknown[];
  };
  "structure.resolve": {
    "id": string;
    "lines": unknown[];
    "path": string;
    "resolved": boolean;
    "title": string;
  };
  "structure.breadcrumb": {
    "breadcrumb": string;
    "scene": string;
  };
  "section.list": {
    "count": number;
    "sections": unknown[];
  };
  "section.get": {
    "after": unknown[];
    "before": unknown[];
    "path": string;
    "scene": string;
    "text": string;
  };
  "section.write": {
    "chars": number;
    "path": string;
    "save": Record<string, unknown>;
    "scene": string;
  };
  "section.insert": {
    "id": string;
    "pending": boolean;
    "save": Record<string, unknown>;
  };
  "section.delete": {
    "deleted": string;
    "save": Record<string, unknown>;
  };
  "section.confirm_next": {
    "confirmed": boolean;
    "scene": string;
  };
  "section.split": {
    "save": Record<string, unknown>;
    "scene": string;
    "split": boolean;
  };
  "section.propose": {
    "proposal": string;
    "status": string;
  };
  "nav.file": {
    "path": string;
    "scene": string;
  };
  "nav.section": {
    "path": string;
    "scene": string;
  };
  "nav.jump": {
    "byte": number;
    "line": number;
    "path": string;
  };
  "nav.prev": {
    "moved": boolean;
  };
  "nav.next": {
    "moved": boolean;
  };
  "nav.back": {
    "moved": boolean;
    "scene": string;
  };
  "nav.forward": {
    "moved": boolean;
  };
  "nav.up": {
    "moved": boolean;
    "workspace": string;
  };
  "nav.switcher": {
    "current": string;
    "previous": string | null;
    "workspaces": unknown[];
  };
  "document.get": {
    "format": string;
    "path": string;
    "text": string;
  };
  "document.write": {
    "chars": number;
    "path": string;
    "save": Record<string, unknown>;
  };
  "document.rename": {
    "from": string;
    "save": Record<string, unknown>;
    "to": string;
  };
  "plot.get": {
    "blocks": unknown[];
    "level": string;
  };
  "plot.write": {
    "chars": number;
    "level": string;
    "path": string;
    "save": Record<string, unknown>;
  };
  "plot.set_state": {
    "path": string;
    "save": Record<string, unknown>;
    "scene": string;
    "state": string;
  };
  "plot.insert_section": {
    "path": string;
    "save": Record<string, unknown>;
    "scene": string;
  };
  "plot.outline_fold": {
    "chapter": number;
    "fold": boolean;
    "folded": unknown[];
  };
  "save.now": {
    "state": string;
    "written": unknown[];
  };
  "save.state": {
    "conflicts": unknown[];
    "state": string;
    "unsaved": unknown[];
  };
  "save.keep_mine": {
    "kept": unknown[];
    "save": Record<string, unknown>;
  };
  "save.reload_external": {
    "reloaded": unknown[];
  };
  "save.diff_external": {
    "diffs": unknown[];
  };
  "search.file": {
    "count": number;
    "path": string;
    "replaced": boolean;
    "save": Record<string, unknown>;
  };
  "search.replace_file": {
    "count": number;
    "path": string;
    "save": Record<string, unknown>;
  };
  "search.project": {
    "count": number;
    "hits": unknown[];
    "query": string;
  };
  "search.find_next": {
    "range": unknown[] | null;
    "selected": number | null;
  };
  "search.find_prev": {
    "range": unknown[] | null;
    "selected": number | null;
  };
  "materials.list": {
    "count": number;
    "materials": unknown[];
  };
  "materials.get": {
    "body": string;
    "material": Record<string, unknown>;
    "review": Record<string, unknown>;
  };
  "materials.approve": {
    "already": boolean;
    "approved": string;
    "id": string;
    "renamed": string | null;
    "save": Record<string, unknown>;
  };
  "materials.reject": {
    "note": string;
    "rejected": string;
    "save": Record<string, unknown>;
  };
  "materials.update": {
    "save": Record<string, unknown>;
    "updated": string;
  };
  "materials.attach": {
    "attached": string;
    "path": string;
    "save": Record<string, unknown>;
  };
  "materials.search": {
    "count": number;
    "hits": unknown[];
    "query": string;
  };
  "materials.propose": {
    "body": string;
    "id": string;
    "path": string;
    "save": Record<string, unknown>;
  };
  "idea.list": {
    "count": number;
    "ideas": unknown[];
  };
  "idea.capture": {
    "captured": string;
    "index": number;
    "save": Record<string, unknown>;
  };
  "idea.promote": {
    "promoted": string;
    "save": Record<string, unknown>;
    "target": string;
    "written": unknown[];
  };
  "idea.drop": {
    "dropped": string;
    "save": Record<string, unknown>;
  };
  "idea.write": {
    "chars": number;
    "save": Record<string, unknown>;
    "written": unknown[];
  };
  "cast.get": {
    "cast": string;
  };
  "cast.list": {
    "characters": unknown[];
    "count": number;
  };
  "cast.timeline": {
    "bands": unknown[];
    "chapters": unknown[];
    "index": number;
  };
  "cast.scan": {
    "count": number;
    "unmatched": unknown[];
  };
  "dictionary.list": {
    "count": number;
    "entries": unknown[];
  };
  "dictionary.register_term": {
    "registered": string;
    "save": Record<string, unknown>;
    "written": unknown[];
  };
  "dictionary.register_character": {
    "registered": string;
    "save": Record<string, unknown>;
    "written": unknown[];
  };
  "dictionary.term_full": {
    "words": unknown[];
  };
  "dictionary.modes_list": {
    "count": number;
    "modes": unknown[];
  };
  "lang.tokens": {
    "count": number;
    "path": string;
    "tokens": unknown[];
  };
  "lang.completion": {
    "count": number;
    "items": unknown[];
    "path": string;
  };
  "lang.hover": {
    "hover": Record<string, unknown> | null;
    "path": string;
  };
  "lang.hints": {
    "count": number;
    "hints": unknown[];
    "path": string;
  };
  "lang.code_action": {
    "actions": unknown[];
    "count": number;
    "path": string;
  };
  "lang.definition": {
    "definition": Record<string, unknown> | null;
    "path": string;
  };
  "lang.references": {
    "count": number;
    "path": string;
    "references": unknown[];
  };
  "lang.symbols": {
    "count": number;
    "path": string;
    "symbols": unknown[];
  };
  "lang.format": {
    "count": number;
    "edits": unknown[];
    "path": string;
  };
  "lang.rename_preview": {
    "changes": unknown[];
    "edits": unknown[];
    "error": string | null;
    "path": string;
    "prepared": Record<string, unknown>;
  };
  "lang.rename_apply": {
    "renamed": string;
    "save": Record<string, unknown>;
    "written": unknown[];
  };
  "lang.folding": {
    "count": number;
    "path": string;
    "ranges": unknown[];
  };
  "diagnostics.run": {
    "count": number;
    "diagnostics": unknown[];
    "errors": number;
  };
  "diagnostics.place": {
    "path": string;
    "place": Record<string, unknown>;
  };
  "review.list": {
    "count": number;
    "diagnostics": unknown[];
  };
  "review.goto": {
    "byte": number;
    "code": string;
    "index": number;
    "path": string;
  };
  "agent.task_list": {
    "count": number;
    "tasks": unknown[];
  };
  "agent.task_create": {
    "created": unknown[];
    "role": string;
    "save": Record<string, unknown>;
    "scope": string;
  };
  "agent.task_get": {
    "proposal": Record<string, unknown> | null;
    "result": Record<string, unknown> | null;
    "task": Record<string, unknown>;
  };
  "agent.task_run": {
    "notices": unknown[];
    "queued": boolean;
  };
  "agent.task_dry_run": {
    "chars": number;
    "path": string;
    "save": Record<string, unknown>;
    "task": Record<string, unknown>;
  };
  "agent.task_stop": {
    "save": Record<string, unknown>;
    "stopped": string;
  };
  "agent.stop_all": {
    "message": string;
    "save": Record<string, unknown>;
  };
  "agent.proposal_list": {
    "count": number;
    "proposals": unknown[];
  };
  "agent.proposal_get": {
    "after": string;
    "after_chars": number;
    "before": string;
    "before_chars": number;
    "comment": string;
    "created_at": string;
    "drifted": boolean;
    "id": string;
    "kind": string;
    "note": string;
    "origin": string;
    "partial": boolean;
    "status": string;
    "target": string;
    "updated_at": string;
  };
  "agent.proposal_apply": {
    "applied": string;
    "save": Record<string, unknown>;
  };
  "agent.proposal_reject": {
    "comment": string;
    "rejected": string;
    "save": Record<string, unknown>;
  };
  "agent.delegation_get": {
    "enabled": boolean;
    "levels": unknown[];
    "max_parallel": number;
  };
  "agent.delegation_set": {
    "level": string;
    "message": string;
    "role": string;
    "save": Record<string, unknown>;
  };
  "agent.mcp_config_get": {
    "path": string;
    "servers": unknown[];
    "text": string;
  };
  "agent.mcp_config_validate": {
    "count": number;
    "diagnostics": unknown[];
    "valid": boolean;
  };
  "agent.mcp_config_set": {
    "chars": number;
    "path": string;
    "save": Record<string, unknown>;
  };
  "agent.thread_list": {
    "count": number;
    "threads": unknown[];
  };
  "agent.thread_get": {
    "id": string;
    "messages": unknown[];
    "scope": string;
    "status": string;
    "title": string;
  };
  "agent.audit_list": {
    "count": number;
    "events": unknown[];
  };
  "context.build": {
    "cap": number;
    "chars": number;
    "packet": string;
  };
  "prompt.list": {
    "count": number;
    "prompts": unknown[];
  };
  "prompt.get": {
    "blockers": unknown[];
    "edited": boolean;
    "name": string;
    "path": string;
    "shared": string;
    "text": string;
  };
  "prompt.diff": {
    "count": number;
    "edited": boolean;
    "lines": unknown[];
    "name": string;
    "path": string;
  };
  "prompt.reset": {
    "name": string;
    "reset": number;
    "save": Record<string, unknown>;
  };
  "mail.send": {
    "approved_by": string;
    "message": number;
    "thread": string;
  };
  "export.text": {
    "chars": number;
    "name": string;
    "out": string;
    "range": string;
    "sections": number;
  };
  "reference.pane": {
    "cast": unknown[];
    "diagnostics": unknown[];
    "illustrations": unknown[];
    "materials": unknown[];
    "plot": unknown[];
    "plot_nav": Record<string, unknown>;
    "scene": string;
    "senses": unknown[];
    "space": unknown[];
    "upper": unknown[];
    "words": unknown[];
  };
  "settings.get": {
    "language": string;
    "path": string;
    "settings": Record<string, unknown>;
  };
  "settings.set": {
    "key": string;
    "language": string;
    "value": unknown;
  };
  "session.workspace_get": {
    "current": string;
    "previous": string | null;
    "workspaces": unknown[];
  };
  "session.workspace_switch": {
    "current": string;
    "moved": boolean;
  };
  "session.recent_document": {
    "moved": boolean;
    "recent": unknown[];
  };
  "session.command": {
    "command": string;
    "data": unknown;
    "gui_only": boolean;
    "key": string;
    "label": string;
    "moved": boolean;
    "op": string;
    "scene": string;
    "value": unknown;
    "workspace": string;
  };
  "session.poll_events": {
    "count": number;
    "events": unknown[];
  };
  "help.topic": {
    "text": string;
    "title": string;
    "topic": string;
  };
  "api.list": {
    "catalog_version": string;
    "count": number;
    "interfaces": unknown[];
    "operations": unknown[];
    "version": string;
  };
}

/** 画面から呼べる操作か（関門付きでも `Interface::Rust` なので `role` は要らない）。 */
export function isOp(id: string): id is OpId {
  return (OP_IDS as readonly string[]).includes(id);
}
