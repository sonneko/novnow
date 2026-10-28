"""novnow の型スタブ（§19.3.1。`py.typed` と一緒に wheel へ同梱する）。

**生成物**: `python3 scripts/pyi-site.py` が `tests/golden/api_catalog.json`
（§19.7 の 121 操作・25 ドメイン）から作る。手で編集しないこと。
位置引数の型と並びは `arguments.order`（宣言順）どおり。
"""

from typing import Any, Callable, Iterator, Protocol

__version__: str


class Record(dict):
    """JSON と同じ構造の `dict`。キーは属性としても読める（§19.3.2）。"""

    def __getattr__(self, name: str) -> Any: ...
    def __dir__(self) -> list[str]: ...


class NovnowError(Exception):
    """すべての novnow エラーの親（§19.3.3）。"""

    class_: str
    code: str
    message: str
    path: str | None
    exit_code: int


class UsageError(NovnowError):
    """引数の形・型の誤り（§19.1.7）。"""


class ProjectError(NovnowError):
    """プロジェクト構造の不正（§19.1.7）。"""


class IOError(NovnowError):
    """ファイルシステムのエラー（§19.1.7）。"""


class ValidationError(NovnowError):
    """内容の検証エラー（§19.1.7）。"""


class PolicyError(NovnowError):
    """権限・関門の拒否（E036 など）（§19.1.7）。"""


class NotFoundError(NovnowError):
    """対象 ID の不存在（§19.1.7）。"""


class ConflictError(NovnowError):
    """保存状態機の衝突（§19.1.7）。"""


class TimeoutError(NovnowError):
    """LLM 応答・処理の時間切れ（§19.1.7）。"""


class CancelledError(NovnowError):
    """利用者が操作を中断した（§19.1.7）。"""


class agent(Protocol):
    """`session.agent` のドメイン（§19.3.2）。"""

    def task_list(self) -> Any:
        """タスク一覧（`agent.task_list`）"""

    def task_create(self, role: str, scope: str = ..., intent: str = ..., comment: str = ..., examples: str = ...) -> Any:
        """タスクの起票（`agent.task_create`）"""

    def task_get(self, id: str) -> Any:
        """タスク 1 件の詳細（`agent.task_get`）"""

    def task_run(self, id: str, role: str) -> Any:
        """タスクの実行（`agent.task_run`）　[MCP からは `role` が必要]"""

    def task_dry_run(self, id: str) -> Any:
        """ドライラン（`agent.task_dry_run`）"""

    def task_stop(self, id: str, role: str) -> Any:
        """タスクの中止（`agent.task_stop`）　[MCP からは `role` が必要]"""

    def stop_all(self, role: str) -> Any:
        """全実行の中止（`agent.stop_all`）　[MCP からは `role` が必要]"""

    def proposal_list(self) -> Any:
        """提案一覧（`agent.proposal_list`）"""

    def proposal_get(self, id: str) -> Any:
        """提案 1 件の内容（`agent.proposal_get`）"""

    def proposal_apply(self, id: str, role: str) -> Any:
        """提案の適用（`agent.proposal_apply`）　[MCP からは `role` が必要]"""

    def proposal_reject(self, id: str, comment: str, role: str) -> Any:
        """提案の却下（`agent.proposal_reject`）　[MCP からは `role` が必要]"""

    def delegation_get(self) -> Any:
        """委譲レベルの参照（`agent.delegation_get`）　[M—（safety）]"""

    def delegation_set(self, role: str = ..., *, level: str) -> Any:
        """委譲レベルの変更（`agent.delegation_set`）　[M—（safety）]"""

    def mcp_config_get(self) -> Any:
        """MCP 接続設定の参照（`agent.mcp_config_get`）"""

    def mcp_config_validate(self, text: str = ...) -> Any:
        """MCP 接続設定の検証（`agent.mcp_config_validate`）"""

    def mcp_config_set(self, text: str) -> Any:
        """MCP 接続設定の書き込み（`agent.mcp_config_set`）　[M—（safety）]"""

    def thread_list(self) -> Any:
        """メールスレッド一覧（`agent.thread_list`）"""

    def thread_get(self, id: str) -> Any:
        """スレッド 1 件の内容（`agent.thread_get`）"""

    def audit_list(self) -> Any:
        """監査ログ一覧（`agent.audit_list`）"""


class api(Protocol):
    """`session.api` のドメイン（§19.3.2）。"""

    def list(self) -> Any:
        """正規操作カタログの機械可読出力（`api.list`）"""


class cast(Protocol):
    """`session.cast` のドメイン（§19.3.2）。"""

    def get(self, scene: str) -> Any:
        """キャラクター情報・有効なモード（`cast.get`）"""

    def list(self) -> Any:
        """キャラクター一覧（`cast.list`）"""

    def timeline(self, index: int = ...) -> Any:
        """呼称タイムライン（`cast.timeline`）"""

    def scan(self, role: str) -> Any:
        """本文スキャンで候補抽出（`cast.scan`）　[MCP からは `role` が必要]"""


class context(Protocol):
    """`session.context` のドメイン（§19.3.2）。"""

    def build(self, role: str, scene: str) -> Any:
        """コンテキストパケットの構築（`context.build`）"""


class diagnostics(Protocol):
    """`session.diagnostics` のドメイン（§19.3.2）。"""

    def run(self, scope: str = ..., path: str = ..., scene: str = ...) -> Any:
        """診断（`diagnostics.run`）"""

    def place(self, path: str = ..., pos: int = ...) -> Any:
        """地名解決（`diagnostics.place`）"""


class dictionary(Protocol):
    """`session.dictionary` のドメイン（§19.3.2）。"""

    def list(self) -> Any:
        """辞書の一覧（`dictionary.list`）"""

    def register_term(self, term: str, definition: str = ..., *, role: str) -> Any:
        """用語の辞書登録（`dictionary.register_term`）　[MCP からは `role` が必要]"""

    def register_character(self, name: str, reading: str = ..., *, role: str) -> Any:
        """キャラクターの辞書登録（`dictionary.register_character`）　[MCP からは `role` が必要]"""

    def term_full(self, query: str) -> Any:
        """用語の完全情報（`dictionary.term_full`）"""

    def modes_list(self) -> Any:
        """全モード一覧（`dictionary.modes_list`）"""


class document(Protocol):
    """`session.document` のドメイン（§19.3.2）。"""

    def get(self, path: str) -> Any:
        """ドキュメント全文の取得（`document.get`）"""

    def write(self, path: str, text: str, role: str) -> Any:
        """ドキュメント全文の書き込み（`document.write`）　[MCP からは `role` が必要]"""

    def rename(self, from_: str, to: str, role: str) -> Any:
        """文書のリネーム（`document.rename`）　[MCP からは `role` が必要; 実引数名は `from`（Python の予約語なので型スタブでは `_` を付ける）]"""


class export(Protocol):
    """`session.export` のドメイン（§19.3.2）。"""

    def text(self, range: str = ..., state: str = ..., format: str = ..., out: str = ..., blurb: bool = ..., blurb_only: bool = ..., keep_illustration_memo: bool = ..., stock: bool = ..., role: str = ...) -> Any:
        """テキストのエクスポート（`export.text`）　[書き込みのときだけ `role` が必要]"""


class help(Protocol):
    """`session.help` のドメイン（§19.3.2）。"""

    def topic(self, topic: str = ...) -> Any:
        """ヘルプ本文（`help.topic`）"""


class idea(Protocol):
    """`session.idea` のドメイン（§19.3.2）。"""

    def list(self) -> Any:
        """着想一覧（`idea.list`）"""

    def capture(self, title: str, kind: str = ..., keywords: str = ..., *, role: str) -> Any:
        """着想の記録（`idea.capture`）　[MCP からは `role` が必要]"""

    def promote(self, id: str, target: str, to: str = ..., *, role: str) -> Any:
        """着想の昇格（`idea.promote`）　[MCP からは `role` が必要]"""

    def drop(self, id: str, role: str) -> Any:
        """着想の削除（`idea.drop`）　[MCP からは `role` が必要]"""

    def write(self, id: str, text: str, role: str) -> Any:
        """着想メモの書き込み（`idea.write`）　[MCP からは `role` が必要]"""


class lang(Protocol):
    """`session.lang` のドメイン（§19.3.2）。"""

    def tokens(self, path: str = ..., scene: str = ...) -> Any:
        """字句解析（`lang.tokens`）"""

    def completion(self, path: str, pos: int) -> Any:
        """補完候補（`lang.completion`）"""

    def hover(self, path: str, pos: int) -> Any:
        """カーソル位置の情報（`lang.hover`）"""

    def hints(self, path: str, start: int = ..., end: int = ...) -> Any:
        """インラインヒント（`lang.hints`）"""

    def code_action(self, path: str, start: int, end: int, apply: bool = ..., index: int = ..., role: str = ...) -> Any:
        """クイックフィックス候補（`lang.code_action`）　[書き込みのときだけ `role` が必要]"""

    def definition(self, path: str, pos: int) -> Any:
        """定義へジャンプ（`lang.definition`）"""

    def references(self, path: str, pos: int) -> Any:
        """参照検索（`lang.references`）"""

    def symbols(self, path: str) -> Any:
        """シンボル一覧（`lang.symbols`）"""

    def format(self, path: str, apply: bool = ...) -> Any:
        """整形（`lang.format`）"""

    def rename_preview(self, path: str, pos: int, to: str = ...) -> Any:
        """リネームの影響範囲（`lang.rename_preview`）"""

    def rename_apply(self, path: str, pos: int, to: str, role: str) -> Any:
        """リネームの実行（`lang.rename_apply`）　[MCP からは `role` が必要]"""

    def folding(self, path: str) -> Any:
        """折り畳み範囲（`lang.folding`）"""


class mail(Protocol):
    """`session.mail` のドメイン（§19.3.2）。"""

    def send(self, to: str, body: str, kind: str = ..., scope: str = ...) -> Any:
        """エージェント間のメール（`mail.send`）"""


class materials(Protocol):
    """`session.materials` のドメイン（§19.3.2）。"""

    def list(self) -> Any:
        """素材インボックス一覧（`materials.list`）"""

    def get(self, id: str) -> Any:
        """素材カードの取得（`materials.get`）"""

    def approve(self, id: str, role: str) -> Any:
        """素材の採用（`materials.approve`）　[MCP からは `role` が必要]"""

    def reject(self, id: str, note: str = ..., *, role: str) -> Any:
        """素材の却下（`materials.reject`）　[MCP からは `role` が必要]"""

    def update(self, id: str, review: dict[str, Any] = ..., *, role: str) -> Any:
        """素材カードの編集（`materials.update`）　[MCP からは `role` が必要]"""

    def attach(self, id: str, path: str = ..., text: str = ..., *, role: str) -> Any:
        """素材の添付（`materials.attach`）　[MCP からは `role` が必要]"""

    def search(self, query: str, category: str = ..., scope: str = ...) -> Any:
        """素材の検索（`materials.search`）"""

    def propose(self, title: str, source: str = ..., markdown: str = ..., category: str = ...) -> Any:
        """素材の提案（`materials.propose`）"""


class nav(Protocol):
    """`session.nav` のドメイン（§19.3.2）。"""

    def file(self, path: str, scene: str = ...) -> Any:
        """文書・ファイルの直接指定（`nav.file`）　[C—（stateful）; M—（stateful）]"""

    def section(self, scene: str) -> Any:
        """セクションへの移動（`nav.section`）　[C—（stateful）; M—（stateful）]"""

    def jump(self, line: int = ..., query: str = ...) -> Any:
        """行への移動（`nav.jump`）　[C—（stateful）; M—（stateful）]"""

    def prev(self) -> Any:
        """前の位置（`nav.prev`）　[C—（stateful）; M—（stateful）]"""

    def next(self) -> Any:
        """次の位置（`nav.next`）　[C—（stateful）; M—（stateful）]"""

    def back(self) -> Any:
        """親ディレクトリへ（`nav.back`）　[C—（stateful）; M—（stateful）]"""

    def forward(self) -> Any:
        """子の先頭へ（`nav.forward`）　[C—（stateful）; M—（stateful）]"""

    def up(self) -> Any:
        """1 つ上の階層へ（`nav.up`）　[C—（stateful）; M—（stateful）]"""

    def switcher(self) -> Any:
        """切替パネルの一覧（`nav.switcher`）　[C—（stateful）; M—（stateful）]"""


class plot(Protocol):
    """`session.plot` のドメイン（§19.3.2）。"""

    def get(self, level: str, scene: str = ..., chapter: int = ...) -> Any:
        """全体／章プロットの取得（`plot.get`）"""

    def write(self, level: str, text: str, scene: str = ..., chapter: int = ..., *, role: str) -> Any:
        """全体／章／話プロットの書き込み（`plot.write`）　[MCP からは `role` が必要]"""

    def set_state(self, scene: str, state: bool = ..., *, role: str) -> Any:
        """プロットの確認状態の切替（`plot.set_state`）　[MCP からは `role` が必要]"""

    def insert_section(self, scene: str = ..., title: str = ..., *, role: str) -> Any:
        """話プロットへの節挿入（`plot.insert_section`）　[MCP からは `role` が必要]"""

    def outline_fold(self, chapter: int, fold: bool = ...) -> Any:
        """構成ツリーの章→話の展開・折り畳み（`plot.outline_fold`）"""


class project(Protocol):
    """`session.project` のドメイン（§19.3.2）。"""

    def info(self) -> Any:
        """プロジェクト基本情報ワークスペース見出し（`project.info`）"""

    def new(self, path: str, title: str = ...) -> Any:
        """新規プロジェクト作成（`project.new`）　[L—（startup）; M—（project）]"""

    def validate(self) -> Any:
        """構造検証（`project.validate`）"""

    def reload(self) -> Any:
        """再読込（`project.reload`）　[C—（stateful）; M—（stateful）]"""

    def stats(self) -> Any:
        """執筆統計（`project.stats`）"""

    def recent_list(self) -> Any:
        """最近開いたプロジェクト一覧（`project.recent_list`）　[M—（stateful）]"""

    def recent_remove(self, path: str) -> Any:
        """最近開いた一覧からの削除（`project.recent_remove`）　[M—（stateful）]"""

    def import_zip(self, path: str = ..., bytes: bytes | str = ...) -> Any:
        """Zip 導入（`project.import_zip`）　[M—（binary）]"""

    def export_zip(self, out: str = ...) -> Any:
        """Zip 書き出し（`project.export_zip`）　[M—（binary）]"""


class prompt(Protocol):
    """`session.prompt` のドメイン（§19.3.2）。"""

    def list(self) -> Any:
        """プロンプトファイル一覧（`prompt.list`）"""

    def get(self, name: str) -> Any:
        """プロンプト内容の取得（`prompt.get`）"""

    def diff(self, name: str) -> Any:
        """初期版との差分（`prompt.diff`）"""

    def reset(self, name: str) -> Any:
        """プロンプトの初期化（`prompt.reset`）　[M—（safety）]"""


class reference(Protocol):
    """`session.reference` のドメイン（§19.3.2）。"""

    def pane(self, scene: str = ...) -> Any:
        """参照ペインの全セクション（`reference.pane`）"""


class review(Protocol):
    """`session.review` のドメイン（§19.3.2）。"""

    def list(self) -> Any:
        """見直し項目一覧（`review.list`）"""

    def goto(self, index: int) -> Any:
        """見直し項目への移動（`review.goto`）　[C—（stateful）; M—（stateful）]"""


class save(Protocol):
    """`session.save` のドメイン（§19.3.2）。"""

    def now(self) -> Any:
        """保存（`save.now`）　[C—（stateful）; M—（stateful）]"""

    def state(self) -> Any:
        """保存状態の取得（`save.state`）　[C—（stateful）; M—（stateful）]"""

    def keep_mine(self, path: str = ...) -> Any:
        """衝突時に自分の版を維持（`save.keep_mine`）　[C—（stateful）; M—（stateful）]"""

    def reload_external(self, path: str = ...) -> Any:
        """外部変更の読み込み（`save.reload_external`）　[C—（stateful）; M—（stateful）]"""

    def diff_external(self, path: str = ...) -> Any:
        """自分の版と外部変更の差分（`save.diff_external`）　[C—（stateful）; M—（stateful）]"""


class search(Protocol):
    """`session.search` のドメイン（§19.3.2）。"""

    def file(self, query: str, replace: str = ..., regex: bool = ..., role: str = ...) -> Any:
        """検索／置換（`search.file`）　[書き込みのときだけ `role` が必要]"""

    def replace_file(self, query: str, replace: str, regex: bool = ..., *, role: str) -> Any:
        """全置換（`search.replace_file`）　[MCP からは `role` が必要]"""

    def project(self, query: str, regex: bool = ...) -> Any:
        """全体検索（`search.project`）"""

    def find_next(self) -> Any:
        """次の候補（`search.find_next`）　[C—（stateful）; M—（stateful）]"""

    def find_prev(self) -> Any:
        """前の候補（`search.find_prev`）　[C—（stateful）; M—（stateful）]"""


class section(Protocol):
    """`session.section` のドメイン（§19.3.2）。"""

    def list(self, path: str = ...) -> Any:
        """セクション一覧（`section.list`）"""

    def get(self, scene: str) -> Any:
        """セクション取得（`section.get`）"""

    def write(self, scene: str, text: str, role: str) -> Any:
        """セクション保存（`section.write`）　[MCP からは `role` が必要]"""

    def insert(self, after: str = ..., text: str = ..., *, role: str) -> Any:
        """節の新設（`section.insert`）　[MCP からは `role` が必要]"""

    def delete(self, scene: str = ..., *, role: str) -> Any:
        """節の削除（`section.delete`）　[MCP からは `role` が必要]"""

    def confirm_next(self) -> Any:
        """次の未確認節の確認（`section.confirm_next`）　[C—（stateful）; M—（stateful）]"""

    def split(self, scene: str = ..., *, at: int, role: str) -> Any:
        """節の分割（`section.split`）　[MCP からは `role` が必要]"""

    def propose(self, scene: str, text: str, note: str = ...) -> Any:
        """節の提案（本文は変えない）（`section.propose`）"""


class session(Protocol):
    """`session.session` のドメイン（§19.3.2）。"""

    def workspace_get(self) -> Any:
        """現在のワークスペース（`session.workspace_get`）　[C—（stateful）; M—（stateful）]"""

    def workspace_switch(self, workspace: str) -> Any:
        """ワークスペースの切替（`session.workspace_switch`）　[C—（stateful）; M—（stateful）]"""

    def recent_document(self, back: bool = ...) -> Any:
        """最近開いた文書の循環（`session.recent_document`）　[C—（stateful）; M—（stateful）]"""

    def command(self, name: str) -> Any:
        """パレットコマンドの実行（`session.command`）　[C—（stateful）; M—（stateful）]"""

    def poll_events(self) -> Any:
        """イベントキューの取得（`session.poll_events`）　[C—（stateful）; M—（stateful）]"""


class settings(Protocol):
    """`session.settings` のドメイン（§19.3.2）。"""

    def get(self) -> Any:
        """設定の取得（`settings.get`）"""

    def set(self, key: str, value: str = ...) -> Any:
        """設定の変更（`settings.set`）　[M—（safety）]"""


class structure(Protocol):
    """`session.structure` のドメイン（§19.3.2）。"""

    def tree(self, level: str = ..., chapter: int = ...) -> Any:
        """構成ツリー（`structure.tree`）"""

    def resolve(self, query: str) -> Any:
        """位置・ID の解決（`structure.resolve`）"""

    def breadcrumb(self) -> Any:
        """パンくず（`structure.breadcrumb`）　[C—（stateful）; M—（stateful）]"""


class Session:
    """開いたプロジェクト（§19.3.2）。コンテキストマネージャ。"""

    def __init__(self, path: str) -> None: ...
    def run(self, op: str, *args: Any, **kwargs: Any) -> Any:
        """正規操作を 1 つ実行する。位置引数は `arguments.order` の順。"""
    def command(self, name: str) -> Any: ...
    def poll_events(self) -> list[dict[str, Any]]: ...
    def on_event(self, callback: Callable[[dict[str, Any]], None]) -> None: ...
    def cancel(self, handle: str) -> Any: ...
    def set_locale(self, locale: str) -> None: ...
    @property
    def root(self) -> str | None: ...
    def close(self) -> None: ...
    def __getattr__(self, domain: str) -> Any: ...
    def __enter__(self) -> Session: ...
    def __exit__(self, *exc_info: Any) -> bool: ...
    def __iter__(self) -> Iterator[str]: ...
    @property
    def agent(self) -> agent: ...
    @property
    def api(self) -> api: ...
    @property
    def cast(self) -> cast: ...
    @property
    def context(self) -> context: ...
    @property
    def diagnostics(self) -> diagnostics: ...
    @property
    def dictionary(self) -> dictionary: ...
    @property
    def document(self) -> document: ...
    @property
    def export(self) -> export: ...
    @property
    def help(self) -> help: ...
    @property
    def idea(self) -> idea: ...
    @property
    def lang(self) -> lang: ...
    @property
    def mail(self) -> mail: ...
    @property
    def materials(self) -> materials: ...
    @property
    def nav(self) -> nav: ...
    @property
    def plot(self) -> plot: ...
    @property
    def project(self) -> project: ...
    @property
    def prompt(self) -> prompt: ...
    @property
    def reference(self) -> reference: ...
    @property
    def review(self) -> review: ...
    @property
    def save(self) -> save: ...
    @property
    def search(self) -> search: ...
    @property
    def section(self) -> section: ...
    @property
    def session(self) -> session: ...
    @property
    def settings(self) -> settings: ...
    @property
    def structure(self) -> structure: ...


def open(path: str) -> Session:
    """既存のプロジェクトを開く（§19.3.2）。"""
def create(path: str, title: str | None = ...) -> Session:
    """新しいプロジェクトを作って開く（`project.new`）。"""
def operations() -> list[dict[str, Any]]:
    """正規操作カタログ（§19.7。`api.list` と同じ形）。"""
