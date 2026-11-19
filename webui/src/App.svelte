<script lang="ts">
  // 画面の組み立て（§9.1）と、正規操作の呼び出し（§21.1.3）。W2 はここまで。
  //
  // 流れ:
  //
  // 1. `open` でプロジェクトを開き、`section.list` などで画面の材料を集める。
  // 2. 打鍵は `SectionEditor` が受け、`section.write` で本文を書く（**節単位**。§8.1）。
  // 3. 自動保存は `save.now`（`Session::flush` と同じ 1 回分）を、最後の打鍵から
  //    `autosave_ms` のあとに 1 度だけ呼ぶ（§8.2。第 1 の `save_if_due` と同じ規則）。
  // 4. 保存領域（IndexedDB）へ流し終えるのは `bridge.flush()`。失敗したら保存状態機を
  //    `error` にして、ステータスバーと通知で見せる（§21.2.3）。
  //
  // **第 1 の判定を写さない**: 何を保存するか・どの状態へ移るかは `src/app/**` が決める。
  // 画面は「いつ呼ぶか」だけを持つ。

  import AppShell from './lib/shell/AppShell.svelte';
  import Overlay from './lib/shell/Overlay.svelte';
  import ProjectPicker from './lib/project/ProjectPicker.svelte';
  import Switcher from './lib/nav/Switcher.svelte';
  import RenameDialog from './lib/nav/RenameDialog.svelte';
  import { didMove, movedScene, type SwitcherRow } from './lib/nav/moves';
  import ProposalDialog from './lib/sections/ProposalDialog.svelte';
  import ConflictDialog from './lib/save/ConflictDialog.svelte';
  import LookupDialog from './lib/reference/LookupDialog.svelte';
  import { attachArgs, attachResult, placeArgs, type LookupMode } from './lib/reference/lookups';
  import LangDialog from './lib/lang/LangDialog.svelte';
  import {
    LANG_PALETTE,
    definitionRow,
    foldRows,
    hoverRows,
    isLangOp,
    renamePlanRow,
    referenceRows,
    symbolRows,
    tokenRows,
    type LangRow,
  } from './lib/lang/langOps';
  import { byteOffset } from './lib/editor/text';
  import {
    conflictDiffs,
    conflictPaths,
    proposalArgs,
    resolveArgs,
    setStateArgs,
    splitArgs,
    stateOf,
    type ConflictDiff,
  } from './lib/sections/sectionOps';
  import type { Runtime, ZipExport } from './lib/project/picker';
  import { Ime } from './lib/keyboard/ime';
  import { command as commandOf, isLocal, keepPayload, strokeOf } from './lib/keyboard/keys';
  import { texts, type Locale, type Texts } from './lib/i18n';
  import type { Session } from './lib/store/session';
  import { Session as SessionStore } from './lib/store/session';
  import type { Bridge, OpId, SaveState } from './lib/bridge/types';

  /** 橋渡し（wasm）。 */
  export let bridge: Bridge;

  /** 画面の状態（Svelte 5 の rune。`$state` を使えない箇所でも動くようストアで持つ）。 */
  let store = new SessionStore(bridge);
  let screen = store.state;
  let t: Texts['t'] = texts('ja').t;

  /** 変換中（IME）。 */
  let composing = false;

  /**
   * ワークスペースへ渡す読み直しの合図（切り替えるたびに 1 つ進める）。
   *
   * `revision` を受け取る画面（資料・着想・エージェント・点検）は、この値が変わると
   * 材料を取り直す。ワークスペースを離れて戻ったときに古い一覧を出さないための口。
   */
  let revision = 0;

  /** 重ね物（`?` のヘルプと `Ctrl+P` のパレット。§9.7.5: 重ね物は 1 つずつ）。 */
  let overlay: 'help' | 'palette' | 'project' | 'switcher' | null = null;

  /**
   * 参照の面（`cast.get` / `dictionary.list` / `dictionary.term_full`。§9.5.5・§9.7）。
   *
   * 重ね物は 1 つずつ（§9.7.5）なので `overlay` とは別に持つ（同時には開かない。
   * 開くときに `overlay` を閉じる）。
   */
  let lookup: LookupMode | null = null;
  /** 参照の面の見出しに添える対象（場面 ID・語）。 */
  let lookupSubject = '';
  /** `cast.get` の結果。 */
  let lookupCast: Record<string, unknown> | null = null;
  /** `dictionary.list` の結果。 */
  let lookupDictionary: Record<string, unknown> | null = null;
  /** `dictionary.term_full` の結果。 */
  let lookupTerms: Record<string, unknown> | null = null;
  /** 参照の面の失敗の文言。 */
  let lookupFailure = '';
  /** 参照の面の読み込み中。 */
  let lookupBusy = false;

  /**
   * `lang.*` の結果の面（§9.7.1）。重ね物は 1 つずつ（§9.7.5）なので `lookup` とは別に持つ。
   *
   * `mode` は操作 ID（`lang.tokens` など）。`lang.format` は実行して通知に出すので这里へ来ない。
   */
  let langMode: string | null = null;
  /** `lang.*` の面の見出しに添える対象（文書のパス）。 */
  let langSubject = '';
  /** `lang.*` の結果の行。 */
  let langRows: LangRow[] = [];
  /** `lang.*` の結果の件数（ある操作だけ）。 */
  let langCount: number | null = null;
  /** `lang.*` の面の失敗の文言。 */
  let langFailure = '';
  /** `lang.*` の面の読み込み中。 */
  let langBusy = false;

  /**
   * `>` パレットに出すコマンド（§9.7.1）。`lang.*`（LSP 相当の操作群）の 9 件。
   *
   * `name` は**操作 ID**（`lang.tokens` など）。`runPalette` が `lang.` で始まる名前を検出して
   * `runLang` へ渡し、直接 `run('<操作 ID>')` で呼ぶ。`session.command` には
   * `tokens` / `hover` / `symbols` / `rename_apply` / `folding` の名前が無いため、直接呼びが
   * 唯一の口（`definition` / `references` / `format` / `rename_preview` はカタログの
   * `command` 列にも載るが、キャレット位置を渡すため直接呼びを使う）。
   *
   * `label` は生成物 `i18n/catalog.ts` の `api.op.lang.*`（既存）。
   */
  const PALETTE_ROWS: readonly { name: string; label: string }[] = LANG_PALETTE;

  /** 切替パネルの一覧（`nav.switcher` の戻り値。開くたびに取り直す）。 */
  let switcher: Record<string, unknown> | null = null;

  /** 文書のリネームの入力欄を出しているか（`document.rename`）。 */
  let renaming = false;

  /** 提案の面を出しているか（`section.propose`。§8.7.1）。 */
  let proposing = false;

  /** 衝突の面を出しているか（`save.keep_mine` / `save.reload_external`。§8.5）。 */
  let conflicting = false;
  /** 衝突しているパス（`save.state` の `conflicts`）。 */
  let conflicts: string[] = [];

  /** いまのキャレット（UTF-16。節の中の位置）。`Ctrl+Enter` の分割に使う。 */
  let caret = 0;
  /** リネームの対象（`document.rename` の `from`）。 */
  let renamingFrom = '';
  /**
   * 動いている環境。Tauri は `window.__TAURI__` を置く（`tauri.conf.json` の
   * `withGlobalTauri`）。ブラウザの wasm には無い。
   */
  const runtime: Runtime =
    typeof window !== 'undefined' && '__TAURI__' in window ? 'native' : 'browser';
  const ime = new Ime((found) => {
    composing = found;
  });

  /** 前後の節（`section.get`）。 */
  let before: { scene: string; text: string }[] = [];
  let after: { scene: string; text: string }[] = [];

  /** 台詞比率と有効モード（§9.4.2）。W2 では本文から数える簡易版。 */
  let ratio = 0;
  let modes = '';

  /** 自動保存の待ち（§8.2 の 400ms）と、いま走っている保存。 */
  let timer: ReturnType<typeof setTimeout> | null = null;
  let saving = false;
  /** 保存の待ち（ミリ秒。`ui.json` の `autosave_ms`）。 */
  const idle = 400;

  /** 画面の状態を購読する（`open` の前から）。 */
  $: unsubscribe = store.subscribe((next) => {
    screen = next;
  });

  /** 言語が変わったら文言を差し替える。 */
  $: t = texts(screen.locale as Locale).t;

  /** 起動（`open` → 材料を集める）。 */
  export async function start(root: string | null = null): Promise<void> {
    try {
      await bridge.open(root);
      store.update((next) => {
        next.projectId = bridge.projectId();
        next.version = bridge.version();
        next.locale = bridge.locale();
      });
      await refresh();
      store.update((next) => {
        next.ready = true;
      });
    } catch (error) {
      // 保存領域を使えない環境（IndexedDB の無い iframe・プライベートモードなど）では、
      // メモリだけで開いて「保存されない」ことを明示する（§21.1.2: 黙って別の結果にしない）
      const message = error instanceof Error ? error.message : String(error);
      const fallback = (bridge as unknown as { openFallback?: () => void }).openFallback;
      if (typeof fallback === 'function') {
        fallback();
        await refresh();
        store.update((next) => {
          next.ready = true;
          next.projectId = bridge.projectId();
          next.version = bridge.version();
          next.locale = bridge.locale();
        });
        store.notice(message, 'error', 0);
        return;
      }
      store.update((next) => {
        next.failure = message;
      });
    }
  }

  /** 画面の材料を集め直す（正規操作だけを通す）。 */
  async function refresh(): Promise<void> {
    const info = await store.run('project.info');
    if (info.ok) {
      store.update((next) => {
        next.title = info.result.title;
        next.projectChars = info.result.chars;
        next.diagnostics = 0;
      });
    }
    const crumb = await store.run('structure.breadcrumb');
    if (crumb.ok) {
      store.update((next) => {
        next.breadcrumb = crumb.result.breadcrumb;
        next.scene = crumb.result.scene ?? '';
      });
    }
    const valid = await store.run('project.validate');
    if (valid.ok) {
      store.update((next) => {
        next.diagnostics = valid.result.diagnostics.filter(
          (found) => typeof found === 'object' && found !== null,
        ).length;
      });
    }
    await openSection(screen.scene);
  }

  /** 節を開く（一覧・前後・本文を取る）。 */
  async function openSection(scene: string): Promise<void> {
    if (!scene) {
      return;
    }
    const list = await store.run('section.list');
    if (list.ok) {
      store.update((next) => {
        next.sections = (list.result.sections as unknown as typeof next.sections) ?? [];
      });
    }
    const found = await store.run('section.get', { scene });
    if (found.ok) {
      store.update((next) => {
        next.scene = found.result.scene;
        next.path = found.result.path;
        next.text = found.result.text;
        next.sectionChars = countChars(found.result.text);
      });
      before = (found.result.before as { scene: string; text: string }[]) ?? [];
      after = (found.result.after as { scene: string; text: string }[]) ?? [];
      measure();
    }
    const pane = await store.run('reference.pane', { scene });
    store.update((next) => {
      next.reference = pane.ok ? (pane.result as unknown as Record<string, unknown>) : null;
    });
    const state = await store.run('save.state');
    if (state.ok) {
      store.update((next) => {
        next.unsaved = state.result.unsaved.filter((path): path is string => typeof path === 'string');
        next.save = state.result.state as SaveState;
      });
      // 衝突しているパス（§8.5）。面はここで一覧を持つ（押したときに取り直さない）。
      conflicts = conflictPaths(state.result as unknown as Record<string, unknown>);
    }
    const words = await store.run('project.stats');
    if (words.ok) {
      store.update((next) => {
        next.projectChars = Number((words.result as { chars?: number }).chars ?? next.projectChars);
      });
    }
  }

  /** 本文から台詞比率と有効モードを数える（§9.4.2。W2 の簡易版）。 */
  function measure(): void {
    const text = screen.text;
    const total = countChars(text);
    let dialogue = 0;
    for (const match of text.matchAll(/[「『]([^」』]*)[」』]/gu)) {
      dialogue += countChars(match[1] ?? '');
    }
    ratio = total === 0 ? 0 : dialogue / total;
    // `'名前` の参照を集める（§6.4.6 の有効モード。W2 では名前だけ）
    const names = new Set<string>();
    for (const match of text.matchAll(/'([^\s'、。「」『』]+)/gu)) {
      if (match[1]) {
        names.add(match[1]);
      }
    }
    modes = [...names].join(' ');
  }

  /** 文字数（空白を除く。§9.6.1）。 */
  function countChars(text: string): number {
    let found = 0;
    for (const ch of text) {
      if (!/\s/u.test(ch)) {
        found += 1;
      }
    }
    return found;
  }

  /** 本文が変わった（`section.write`。400ms のデバウンスは自動保存側で行う）。 */
  async function oninput(text: string): Promise<void> {
    store.update((next) => {
      next.text = text;
      next.sectionChars = countChars(text);
    });
    measure();
    await writeSection(text);
    schedule();
  }

  /** 節の本文を書く（`section.write`。**節単位**の操作。§8.1）。 */
  async function writeSection(text: string): Promise<void> {
    const found = await store.run('section.write', { scene: screen.scene, text, role: '' });
    if (!found.ok) {
      store.notice(found.message, 'error', 0);
      return;
    }
    store.update((next) => {
      next.path = found.result.path;
    });
  }

  /** 自動保存を予約する（最後の打鍵から `idle` ミリ秒）。§8.2。 */
  function schedule(): void {
    if (timer !== null) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      timer = null;
      void saveNow();
    }, idle);
  }

  /** 保存 1 回分（`save.now` → `flush` で保存領域へ流す）。 */
  async function saveNow(): Promise<void> {
    if (saving) {
      return;
    }
    saving = true;
    store.update((next) => {
      next.save = 'saving';
    });
    const found = await store.run('save.now');
    if (!found.ok) {
      saving = false;
      store.update((next) => {
        next.save = 'error';
        next.saveMessage = found.message;
      });
      return;
    }
    // IndexedDB へ流し終えるまで待つ（失敗は `pending` と `error` で返る）
    const flushed = await bridge.flush();
    await drain();
    saving = false;
    store.update((next) => {
      if (flushed.ok) {
        next.save = found.result.state as SaveState;
        next.pending = 0;
      } else {
        next.pending = bridge.pendingWrites();
        next.save = 'error';
        next.saveMessage = flushed.error.message;
      }
    });
  }

  /** 溜まったイベントを状態へ反映する（`save_state_changed` など）。 */
  async function drain(): Promise<void> {
    for (const event of await bridge.pollEvents()) {
      if (event.kind === 'save_state_changed') {
        store.update((next) => {
          next.save = event.state;
          next.saveMessage = event.message;
        });
      } else if (event.kind === 'write_failed') {
        store.update((next) => {
          next.save = 'error';
          next.saveMessage = event.message;
        });
        store.notice(event.message, 'error', 0);
      } else if (event.kind === 'document_changed') {
        // 本文は `section.get` で取り直す（保存の後に 1 度だけ）
      }
    }
    const state = await store.run('save.state');
    if (state.ok) {
      store.update((next) => {
        next.unsaved = state.result.unsaved.filter((path): path is string => typeof path === 'string');
      });
    }
  }

  /** キー（画面で 1 か所だけ受ける。§9.8）。 */
  async function onkey(event: KeyboardEvent): Promise<void> {
    if (composing) {
      // 変換中は 1 つも横取りしない（§9.8.5 の 2）
      return;
    }
    const found = commandOf(strokeOf(event), { mac: isMac() });
    if (!found) {
      return;
    }
    event.preventDefault();
    if (isLocal(found)) {
      local(found);
      return;
    }
    // `Ctrl+Shift+1..9`（節を持ち越す切替）は表に無いので、正規操作を直接呼ぶ（§10.0）
    const keep = keepPayload(found);
    if (keep) {
      const switched = await store.run('session.workspace_switch', {
        workspace: String(keep.workspace),
      });
      if (switched.ok) {
        store.update((next) => {
          next.workspace = switched.result.current;
        });
      }
      return;
    }
    // `Command` の名前は `session.command` が受ける（実行は機能核が決める）
    const result = await store.run('session.command', { name: found });
    if (!result.ok) {
      store.notice(result.message, 'error');
      return;
    }
    await afterCommand(result.result as Record<string, unknown>);
  }

  /** 画面側だけで完結する操作（`session.command` に無いもの）。 */
  function local(found: string): void {
    switch (found) {
      case 'ui:toggle-reference':
        store.update((next) => {
          next.referenceVisible = !next.referenceVisible;
        });
        return;
      case 'ui:help':
        // `?` は重ね物を開く（§9.8.1 の「入力欄外」）。開いていれば閉じる。
        closeLookup();
        overlay = overlay === 'help' ? null : 'help';
        return;
      case 'ui:palette':
        closeLookup();
        overlay = overlay === 'palette' ? null : 'palette';
        return;
      case 'ui:project':
        // `Ctrl+O` と同じ規則（§9.7・§9.8）: 変更を保存してから入口の面を開く。
        // 保存に失敗している間は開かない（仕様 §9.7 の `Ctrl+O` の統一）。
        closeLookup();
        void openProjectPicker();
        return;
      case 'ui:switcher':
        // 切替パネル（§10.0）。開いている間にもう一度押したら閉じる（§9.7.5: 重ね物は 1 つ）。
        closeLookup();
        void openSwitcher();
        return;
      case 'ui:nav-prev':
        void move('nav.prev');
        return;
      case 'ui:nav-next':
        void move('nav.next');
        return;
      case 'ui:close':
        // `Esc` は重ね物を閉じる（参照の面・`lang.*` の面も同じ。§9.7.5）。無ければ節エディタへ戻す。
        if (langMode) {
          closeLang();
          return;
        }
        if (lookup) {
          closeLookup();
          return;
        }
        if (overlay) {
          overlay = null;
        }
        return;
      default:
        return;
    }
  }

  /** コマンドの結果を画面へ反映する。 */
  async function afterCommand(data: Record<string, unknown>): Promise<void> {
    // 構造移動（`Alt+↑` など）のあとは、指していた節を開き直す
    if (typeof data.scene === 'string' && data.scene && data.scene !== screen.scene) {
      await openSection(data.scene);
      return;
    }
    if (data.moved === true) {
      await afterCommand({ scene: await currentScene() });
      return;
    }
    // 保存・再読込は状態を取り直す
    if ('save' in data || 'written' in data) {
      await drain();
    }
    if (typeof data.workspace === 'string' && data.workspace !== screen.workspace) {
      store.update((next) => {
        next.workspace = String(data.workspace);
      });
      // キー（`Alt+5` など）から切り替えたときも取り直させる。
      revision += 1;
    }
    store.update((next) => {
      next.locale = bridge.locale();
    });
  }

  /** いまの節（`structure.breadcrumb` から）。 */
  async function currentScene(): Promise<string> {
    const found = await store.run('structure.breadcrumb');
    return found.ok ? (found.result.scene ?? '') : '';
  }

  /** macOS か（`Alt` の読み替え。§9.8.4）。 */
  function isMac(): boolean {
    return typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);
  }

  /** 節へ移る（`nav.section`。節一覧・文脈ビューから）。 */
  async function nav(target: string): Promise<void> {
    const found = await store.run('nav.section', { scene: target });
    if (!found.ok) {
      store.notice(found.message, 'error');
      return;
    }
    await openSection(found.result.scene);
  }

  /** 節を消す（`section.command` の `delete-section`。確認は §8.1.2 なので機能核に任せる）。 */
  async function remove(target: string): Promise<void> {
    await nav(target);
    const found = await store.run('session.command', { name: 'delete-section' });
    if (!found.ok) {
      store.notice(found.message, 'error');
      return;
    }
    await refresh();
  }

  // ---------------------------------------------------------------- 参照の引き口（§9.5.5・§9.7）
  //
  // 4 つの操作の口。意味は機能核が決める（`src/app/ops/knowledge.rs` / `lang.rs`）ので、
  // ここは「いつ呼ぶか」と「返ってきたものをどう渡すか」だけを持つ。
  //
  // - `cast.get` … 参照ペインの配役の行（ダブルクリック）。その人物のいる**場面**を渡す
  //   （`cast.get` の `scene` は必須で、4 階層の ID でなければ usage エラーになる）
  // - `dictionary.list` … ヘッダの辞書のボタン（`Ctrl+D` の辞書）
  // - `dictionary.term_full` … 参照ペインの用語の行（ダブルクリック。`Ctrl+Shift+W` と同じ）
  // - `diagnostics.place` … 点検の診断の行（正本の「どこ」を機能核へ聞く）

  /** 参照の面を開く前に、他の重ね物を閉じる（§9.7.5: 重ね物は 1 つずつ）。 */
  function closeLookup(): void {
    lookup = null;
    lookupSubject = '';
    lookupFailure = '';
    lookupCast = null;
    lookupDictionary = null;
    lookupTerms = null;
  }

  // ------------------------------------------------------------- `lang.*` の口（§9.7.1）
  //
  // `lang.*`（LSP 相当の操作群）9 件の口。パレット（`>`）から `runLang` で呼ぶ。
  // 意味は機能核が決める（`src/app/ops/lang.rs`）ので、ここは「いつ呼ぶか」と「返ってきた
  // ものをどう並べるか」だけを持つ。引数（`path` / `pos`）は画面の文脈（いまの節・キャレット）
  // から組み立てる（§19.2.3）。

  /** `lang.*` の面を開く前に、他の重ね物を閉じる（§9.7.5: 重ね物は 1 つずつ）。 */
  function closeLang(): void {
    langMode = null;
    langSubject = '';
    langRows = [];
    langCount = null;
    langFailure = '';
  }

  /** 定義・参照のジャンプ（`Location` の `path` へ移る）。 */
  async function jumpLocation(value: unknown): Promise<void> {
    const path = (value as { path?: unknown } | null)?.path;
    if (typeof path === 'string' && path && path !== screen.path) {
      await navTo('', path);
    }
    closeLang();
  }

  /**
   * `lang.*` の操作を 1 つ呼ぶ（パレットから）。
   *
   * `pos` はキャレット（UTF-16）を byte へ直したもの（`byteOffset`）。節が文書のスライスで
   * あっても、既存の `diagnostics.place` と同じく節相対の byte を渡す（§19.2.3 の文脈補完）。
   */
  async function runLang(op: string): Promise<void> {
    closeLookup();
    overlay = null;
    const path = screen.path;
    const pos = byteOffset(screen.text, caret);
    if (op === 'lang.tokens') {
      const found = await store.run('lang.tokens', { path });
      if (!found.ok) {
        store.notice(found.message, 'error');
        return;
      }
      const result = found.result as Parameters<typeof tokenRows>[0];
      langMode = op;
      langSubject = path;
      langRows = tokenRows(result);
      langCount = result.count;
      langFailure = '';
      return;
    }
    if (op === 'lang.hover') {
      const found = await store.run('lang.hover', { path, pos });
      if (!found.ok) {
        store.notice(found.message, 'error');
        return;
      }
      const result = found.result as Parameters<typeof hoverRows>[0];
      langMode = op;
      langSubject = path;
      langRows = hoverRows(result);
      langCount = null;
      langFailure = '';
      return;
    }
    if (op === 'lang.definition') {
      const found = await store.run('lang.definition', { path, pos });
      if (!found.ok) {
        store.notice(found.message, 'error');
        return;
      }
      const result = found.result as Parameters<typeof definitionRow>[0];
      langMode = op;
      langSubject = path;
      langRows = definitionRow(result);
      langCount = null;
      langFailure = '';
      return;
    }
    if (op === 'lang.references') {
      const found = await store.run('lang.references', { path, pos });
      if (!found.ok) {
        store.notice(found.message, 'error');
        return;
      }
      const result = found.result as Parameters<typeof referenceRows>[0];
      langMode = op;
      langSubject = path;
      langRows = referenceRows(result);
      langCount = result.count;
      langFailure = '';
      return;
    }
    if (op === 'lang.symbols') {
      const found = await store.run('lang.symbols', { path });
      if (!found.ok) {
        store.notice(found.message, 'error');
        return;
      }
      const result = found.result as Parameters<typeof symbolRows>[0];
      langMode = op;
      langSubject = path;
      langRows = symbolRows(result);
      langCount = result.count;
      langFailure = '';
      return;
    }
    if (op === 'lang.format') {
      // 整形は実行（`apply: true`）。結果は通知に出し、節を読み直す。
      const found = await store.run('lang.format', { path, apply: true });
      if (!found.ok) {
        store.notice(found.message, 'error');
        return;
      }
      const result = found.result as { count?: unknown; applied?: unknown };
      const applied = typeof result.applied === 'number' ? result.applied : 0;
      store.notice(applied > 0 ? t('lang.format_done') : t('lang.format_none'), 'info');
      await drain();
      // 本文が書き換わったので節を読み直す（エディタの本文を更新する）。
      await refresh();
      return;
    }
    if (op === 'lang.rename_preview') {
      const found = await store.run('lang.rename_preview', { path, pos });
      if (!found.ok) {
        store.notice(found.message, 'error');
        return;
      }
      const result = found.result as Parameters<typeof renamePlanRow>[0];
      langMode = op;
      langSubject = path;
      langRows = renamePlanRow(result);
      langCount = null;
      langFailure = '';
      return;
    }
    if (op === 'lang.rename_apply') {
      // 適用は入力欄を出す（`LangDialog` の `onapply`）。プレビューと対になる（§9.7）。
      langMode = op;
      langSubject = path;
      langRows = [];
      langCount = null;
      langFailure = '';
      return;
    }
    if (op === 'lang.folding') {
      const found = await store.run('lang.folding', { path });
      if (!found.ok) {
        store.notice(found.message, 'error');
        return;
      }
      const result = found.result as Parameters<typeof foldRows>[0];
      langMode = op;
      langSubject = path;
      langRows = foldRows(result);
      langCount = result.count;
      langFailure = '';
      return;
    }
  }

  /** `lang.rename_apply` の適用（`LangDialog` の `onapply`。失敗したら文言を返す）。 */
  async function applyRename(to: string): Promise<string> {
    const path = screen.path;
    const pos = byteOffset(screen.text, caret);
    const found = await store.run('lang.rename_apply', { path, pos, to, role: '' });
    if (!found.ok) {
      return found.message;
    }
    await drain();
    await refresh();
    store.notice(t('lang.applied'), 'info');
    return '';
  }

  /**
   * 配役の人物の情報を出す（`cast.get`。§9.5.5 の 3）。
   *
   * `cast.get` の `scene` はいまの節（`screen.scene`）。節が無ければ機能核が `usage` で
   * 落ちるので先に断る（押しても何も起きない面を開かない。§21.1.2）。
   */
  async function openCast(): Promise<void> {
    if (!screen.scene) {
      store.notice(t('editor.no_sections'), 'info');
      return;
    }
    overlay = null;
    lookup = 'cast';
    lookupSubject = screen.scene;
    lookupCast = null;
    lookupFailure = '';
    lookupBusy = true;
    const found = await store.run('cast.get', { scene: screen.scene });
    lookupBusy = false;
    if (!found.ok) {
      lookupFailure = found.message;
      return;
    }
    lookupCast = found.result as unknown as Record<string, unknown>;
  }

  /** 辞書の一覧を出す（`dictionary.list`。§6.1）。 */
  async function openDictionary(): Promise<void> {
    overlay = null;
    lookup = 'dictionary';
    lookupSubject = '';
    lookupDictionary = null;
    lookupFailure = '';
    lookupBusy = true;
    const found = await store.run('dictionary.list');
    lookupBusy = false;
    if (!found.ok) {
      lookupFailure = found.message;
      return;
    }
    lookupDictionary = found.result as unknown as Record<string, unknown>;
  }

  /**
   * 用語の全文を出す（`dictionary.term_full`。§9.5.5 の 4・`Ctrl+Shift+W` と同じ）。
   *
   * `query` は必須。語を指定しなければ**空の query**（機能核は空なら全件を返す。
   * `novnow.lexicon.search` の `needle.is_empty()` の分岐）。
   */
  async function openTerm(term: string): Promise<void> {
    overlay = null;
    lookup = 'term';
    lookupSubject = term;
    lookupTerms = null;
    lookupFailure = '';
    lookupBusy = true;
    const found = await store.run('dictionary.term_full', { query: term });
    lookupBusy = false;
    if (!found.ok) {
      lookupFailure = found.message;
      return;
    }
    lookupTerms = found.result as unknown as Record<string, unknown>;
  }

  /**
   * 診断の「どこ」を機能核へ聞く（`diagnostics.place`。§10.9.2）。
   *
   * 点検の一覧が持つ `placeOf` は画面の索引による近似なので、機能核の答え（正本）を
   * 別の行に出す。失敗したら `null`（画面は「決まらない」と出す）。
   */
  async function placeOfDiag(
    path: string,
    byte: number | null,
  ): Promise<{ path: string; place: unknown } | null> {
    const args = placeArgs(path, byte);
    if (args === null) {
      return null;
    }
    const found = await store.run('diagnostics.place', args);
    if (!found.ok) {
      return null;
    }
    const result = found.result as { path?: unknown; place?: unknown };
    return {
      path: typeof result.path === 'string' ? result.path : path,
      place: result.place ?? null,
    };
  }

  /**
   * 資料を添付する（`materials.attach`。§16.1・§16.2）。
   *
   * `path` があれば索引に無い資料 MD の**登録**、無ければ索引にある資料の**添付**
   * （索引の `updated_at` / `checksum` の更新）。どちらも機能核の同じ操作の 2 つの腕で、
   * 画面は `attachArgs` で引数を分ける（同じ引数で別の意味にしない）。
   *
   * 失敗したら文言を返す（呼んだ画面が通知に出す）。成功したら通知を出して空文字を返す。
   */
  async function attachMaterial(id: string, path: string): Promise<string> {
    const args = attachArgs(id, path);
    if (args === null) {
      return t('material.index_broken');
    }
    const found =
      args.path === undefined
        ? await store.run('materials.attach', { id: args.id, role: args.role })
        : await store.run('materials.attach', { id: args.id, path: args.path, role: args.role });
    if (!found.ok) {
      return found.message;
    }
    const result = attachResult(found.result as unknown as Record<string, unknown>);
    store.notice(t('material.registered', [result.attached, result.path]), 'info');
    await drain();
    return '';
  }

  /** 参照ペインの行を**ダブルクリック**した（§9.5.5: 単クリックは移動、ダブルクリックは開く）。 */
  async function openRow(kind: string, value: unknown): Promise<void> {
    if (kind === 'cast') {
      await openCast();
      return;
    }
    if (kind === 'word') {
      const term = (value as { term?: unknown } | null)?.term;
      await openTerm(typeof term === 'string' ? term : '');
      return;
    }
    // 資料（明示参照）は資料の画面で開く（§9.5.5 の 7）。移動の口（`nav.file`）を使う。
    const nav = (value as { nav?: { path?: string } } | null)?.nav;
    if (kind === 'material' && nav?.path) {
      await navTo('', nav.path);
    }
  }

  /** 参照ペインの行を押した（§9.5.5）。 */
  async function jump(kind: string, value: unknown): Promise<void> {
    const nav = (value as { nav?: { path?: string; scene?: string } } | null)?.nav;
    if (nav?.scene) {
      await navTo(nav.scene);
      return;
    }
    if (kind === 'material' && nav?.path) {
      await navTo('', nav.path);
    }
  }

  /** `nav.section` / `nav.file` を使う（行の飛び先）。 */
  async function navTo(scene: string, path?: string): Promise<void> {
    const found = path
      ? await store.run('nav.file', { path, ...(scene ? { scene } : {}) })
      : await store.run('nav.section', { scene });
    if (found.ok && found.result.scene) {
      await openSection(found.result.scene);
    }
  }

  /** 比率を保存する（`settings.set`）。 */
  async function setRatio(key: string, value: number): Promise<void> {
    const found = await store.run('settings.set', { key, value: String(value) });
    if (!found.ok) {
      store.notice(found.message, 'error');
    }
  }

  /** 保存をもう一度試す（ステータスバーの `⚠` から）。 */
  async function retrySave(): Promise<void> {
    await saveNow();
  }

  /** 言語を切り替える（`language` コマンド。`ui.json` にも書かれる）。 */
  async function toggleLanguage(): Promise<void> {
    const found = await store.run('session.command', { name: 'language' });
    if (found.ok) {
      store.update((next) => {
        next.locale = bridge.locale();
      });
    }
  }

  /** テーマを切り替える（`theme` コマンド）。 */
  async function toggleTheme(): Promise<void> {
    await store.run('session.command', { name: 'theme' });
  }

  /** ワークスペースを切り替える（`workspace-<name>`。W2 は write だけ）。 */
  async function setWorkspace(name: string): Promise<void> {
    const found = await store.run('session.workspace_switch', { workspace: name });
    if (found.ok) {
      store.update((next) => {
        next.workspace = found.result.current;
      });
      // 切り替えたら材料を取り直させる（古い一覧を出さない）。
      revision += 1;
    }
  }

  /**
   * ヘルプの行を押したとき、そのキーを実行する（§9.8.7 MUST: 案内の項目は押せる）。
   *
   * 押された表示（`Alt+↑ / Alt+↓` など）は**実際のキーではない**ので、`session.command` では
   * なく正規操作を直接呼ぶ。実行したら重ね物を閉じる（§9.8.7 の「閉じてから実行」）。
   */
  async function runHelpRow(key: string): Promise<void> {
    overlay = null;
    switch (key) {
      case 'Alt+0':
        await setWorkspace('write');
        return;
      case 'Ctrl+S':
        await saveNow();
        return;
      case 'Ctrl+Shift+W':
      case 'Ctrl+Alt+Shift+W':
        // 用語の全文（`dictionary.term_full`。§9.5.5 の 4）。語を指定しなければ全件。
        await openTerm('');
        return;
      case 'Ctrl+Alt+L':
        await toggleLanguage();
        return;
      case 'Ctrl+Alt+T':
        await toggleTheme();
        return;
      case 'F5':
        await runNamed('reload');
        return;
      case 'Ctrl+\\':
        local('ui:toggle-reference');
        return;
      case 'Ctrl+\\'.replace('+', '+Shift+'):
        await runNamed('swap-reference');
        return;
      case 'Alt+↑ / Alt+↓':
        await runNamed('previous-section');
        return;
      case 'Ctrl+Home / Ctrl+End':
        await runNamed('file-start');
        return;
      case 'Ctrl+Enter':
        await runNamed('confirm-next');
        return;
      case 'Ctrl+;':
        await runNamed('new-section');
        return;
      default:
        // 行によっては押しても動かない（複数のキーをまとめた表示）。**何も起きないより、
        // 何が起きたか分かるほうがよい**ので、動かせないことを通知に出す（§21.1.2）。
        store.notice(t('workspace.unavailable', ['']), 'info');
        return;
    }
  }

  /** `session.command` を名前で 1 回呼ぶ（結果を画面へ反映する）。 */
  async function runNamed(name: string): Promise<void> {
    const result = await store.run('session.command', { name });
    if (!result.ok) {
      store.notice(result.message, 'error');
      return;
    }
    await afterCommand(result.result as Record<string, unknown>);
  }

  // ---------------------------------------------------------------- 節・プロット・衝突（§8.1.1・§8.5・§10.3.3）
  //
  // 5 つの操作（`section.split` / `section.propose` / `plot.set_state` / `save.keep_mine` /
  // `save.reload_external`）の口。意味は機能核が決める（`src/app/ops/editing.rs`）ので、
  // ここは「いつ呼ぶか」と「結果をどう画面へ反映するか」だけを持つ。

  /**
   * 節をカーソルの位置で 2 つに分ける（`section.split`。`Ctrl+Enter`。§8.1.1）。
   *
   * `at` は**バイト位置**（節の本文の中）。キャレット（UTF-16）をそのまま渡すと和文で
   * 3 倍ずれるので、`splitArgs` が `byteOffset` で換算する。
   * 節の末尾・先頭の分岐（次へ移る / 何もしない / 新規節を作る）は機能核が持つ。
   */
  async function splitSection(position: number): Promise<void> {
    if (!screen.scene) {
      store.notice(t('editor.no_sections'), 'info');
      return;
    }
    const args = splitArgs(screen.scene, screen.text, position) as {
      at: number;
      role: string;
      scene: string;
    };
    const found = await store.run('section.split', args);
    if (!found.ok) {
      store.notice(found.message, 'error');
      return;
    }
    const result = found.result as Record<string, unknown>;
    // 分割できなかった（先頭に居る・文字境界でない）ときは黙って止まる（§8.1.1）。
    if (result.split === false) {
      return;
    }
    await drain();
    const scene = typeof result.scene === 'string' ? result.scene : '';
    if (scene) {
      await openSection(scene);
      return;
    }
    await refresh();
  }

  /** 提案の面を開く（`section.propose`。本文は変えない。§8.7.1）。 */
  function startProposal(): void {
    if (!screen.scene) {
      store.notice(t('editor.no_sections'), 'info');
      return;
    }
    overlay = null;
    proposing = true;
  }

  /**
   * 提案を置く（`section.propose` → `via_mcp` で `agents/proposals/` に 1 件）。
   *
   * **本文は変わらない**ので、画面を取り直す必要はない。置いた提案の ID を通知に出す
   * （何が起きたか分かるようにする。§21.1.2）。
   */
  async function proposeSection(raw: Record<string, unknown>): Promise<string> {
    const args = raw as { note?: string; scene: string; text: string };
    const found = await store.run('section.propose', args);
    if (!found.ok) {
      return found.message;
    }
    const result = found.result as { proposal?: unknown; status?: unknown };
    const id = typeof result.proposal === 'string' ? result.proposal : '';
    if (id) {
      store.notice(`${t('api.op.section.propose')} — ${id}`, 'info', 0);
    }
    return '';
  }

  /**
   * プロットの確認状態を切り替える（`plot.set_state`。§10.3.3）。
   *
   * `state` は `bool`（`true` → `fixed`）。いまの値を `stateOf` で読み、反転した値を送る
   * （同じ値を送って何も起きないボタンにしない）。
   */
  async function togglePlotState(scene: string): Promise<void> {
    const header = screen.sections.find((row) => row.id === scene)?.header ?? '';
    const next = stateOf(header) === 'fixed' ? 'draft' : 'fixed';
    const args = setStateArgs(scene, next) as { role: string; scene: string; state: boolean };
    const found = await store.run('plot.set_state', args);
    if (!found.ok) {
      store.notice(found.message, 'error');
      return;
    }
    await drain();
    store.notice(`${t('api.op.plot.set_state')} — ${next}`, 'info');
  }

  /** 衝突の面を開く（`save.state` の `conflicts` を材料にする。§8.5）。 */
  function openConflicts(): void {
    overlay = null;
    conflicting = true;
  }

  /** 差分を取る（`save.diff_external`。読むためだけ。§8.5）。 */
  async function conflictDiff(path: string): Promise<ConflictDiff[]> {
    const found = await store.run('save.diff_external', resolveArgs(path));
    if (!found.ok) {
      store.notice(found.message, 'error');
      return [];
    }
    return conflictDiffs(found.result as Record<string, unknown>);
  }

  /**
   * 自分の版を残す（`save.keep_mine`）／外部の版を読み込む（`save.reload_external`）。
   *
   * どちらを残すかは**利用者が選ぶ**（画面が勝手に決めない。§8.5 MUST）。解決したら一覧を
   * 取り直し、無くなったら面を閉じる。外部の版を読み込んだときは本文が変わるので開き直す。
   */
  async function resolveConflict(path: string, keep: boolean): Promise<string> {
    const op = keep ? 'save.keep_mine' : 'save.reload_external';
    const found = keep
      ? await store.run('save.keep_mine', resolveArgs(path))
      : await store.run('save.reload_external', resolveArgs(path));
    if (!found.ok) {
      return found.message;
    }
    await drain();
    await refresh();
    if (conflicts.filter((found) => found !== path).length === 0) {
      conflicting = false;
    }
    store.notice(t(keep ? 'api.op.save.keep_mine' : 'api.op.save.reload_external'), 'info');
    void op;
    return '';
  }

  // ---------------------------------------------------------------- ナビゲーション（§9.7・§10.0）
  //
  // 6 つの操作（`nav.prev` / `nav.next` / `nav.back` / `nav.forward` / `nav.switcher` /
  // `document.rename`）の口。意味は機能核が決める（`src/app/ops/editing.rs`）ので、ここは
  // 「いつ呼ぶか」と「結果をどう画面へ反映するか」だけを持つ。

  /**
   * 移動の操作を 1 つ実行する（`nav.prev` / `nav.next` / `nav.back` / `nav.forward`）。
   *
   * 端に居るときは `moved: false` が返る。それは**失敗ではない**ので通知を出さない
   * （§9.8 の移動は端で黙って止まる）。動いたときだけ節を開き直す。
   */
  async function move(op: 'nav.prev' | 'nav.next' | 'nav.back' | 'nav.forward'): Promise<void> {
    // 操作 ID はリテラルで呼ぶ（`run('nav.prev')` の形）。変数で回すと、到達の判定
    // （`webui/tests/parity.mjs` がソースから `run('<ID>')` を拾う）に映らない。
    const found =
      op === 'nav.prev'
        ? await store.run('nav.prev')
        : op === 'nav.next'
          ? await store.run('nav.next')
          : op === 'nav.back'
            ? await store.run('nav.back')
            : await store.run('nav.forward');
    if (!found.ok) {
      store.notice(found.message, 'error');
      return;
    }
    const result = found.result as Record<string, unknown>;
    const scene = movedScene(result);
    if (scene) {
      await openSection(scene);
      return;
    }
    if (didMove(result) && !scene) {
      // `scene` を返さない実装でも、位置が変わっていれば開き直す（今の節を取り直す）。
      const now = await currentScene();
      if (now) {
        await openSection(now);
      }
    }
  }

  /**
   * `>` パレットの行を実行する（`session.command` の名前 → 機能核が操作を決める）。
   */
  async function runPalette(name: string): Promise<void> {
    overlay = null;
    // `lang.*` は直接 `run('<操作 ID>')` で呼ぶ（`session.command` に名前が無いものもある）。
    if (isLangOp(name)) {
      await runLang(name);
      return;
    }
    await runNamed(name);
  }

  /** 切替パネルを開く（`nav.switcher` で一覧を取り直してから出す）。 */
  async function openSwitcher(): Promise<void> {
    if (overlay === 'switcher') {
      overlay = null;
      return;
    }
    // 打鍵を捨てない（保存は自動保存に任せるが、切替の前に確定させる。§10.0 の「未保存」）。
    await saveNow();
    const found = await store.run('nav.switcher');
    if (!found.ok) {
      store.notice(found.message, 'error');
      return;
    }
    switcher = found.result as Record<string, unknown>;
    overlay = 'switcher';
  }

  /** 切替パネルの行を押した（ワークスペースの切替と移動の両方）。 */
  async function pickSwitcher(row: SwitcherRow): Promise<void> {
    overlay = null;
    if (row.kind === 'move') {
      await move(row.op as 'nav.prev' | 'nav.next' | 'nav.back' | 'nav.forward');
      return;
    }
    const found = await store.run(row.op as OpId, row.args);
    if (!found.ok) {
      store.notice(found.message, 'error');
      return;
    }
    store.update((next) => {
      next.workspace = String((found.result as { current?: unknown }).current ?? next.workspace);
    });
  }

  /** 文書のリネームの入力欄を開く（`document.rename`）。開いている間にもう一度押したら閉じる。 */
  async function startRename(): Promise<void> {
    if (renaming) {
      renaming = false;
      return;
    }
    // いま開いている文書を対象にする（`section.get` の `path`）。無ければ何もしない。
    if (!screen.path) {
      store.notice(t('api.error.no_document'), 'error');
      return;
    }
    overlay = null;
    await saveNow();
    renamingFrom = screen.path;
    renaming = true;
  }

  /**
   * リネームを実行する（`document.rename`）。失敗したら文言を返す（入力欄がそのまま出す）。
   *
   * 成功したら、新しいパスの節を開き直す（いま開いていたパスが変わったため）。`App` は
   * リネームの前後関係を知らないので、機能核の戻り値（`from` / `to`）だけを材料にする。
   */
  async function renameDocument(from: string, to: string): Promise<string> {
    // `role` は MCP の関門（§19.1.5）のための欄。画面は人間の経路なので空で渡す
    // （`section.write` と同じ。関門は `Interface::Mcp` のときだけ効く）。
    const found = await store.run('document.rename', { from, to, role: '' });
    if (!found.ok) {
      return found.message;
    }
    renaming = false;
    const result = found.result as { to?: unknown; save?: unknown };
    // 保存は機能核が `save.now` まで溜める（状態を持つインターフェース）。ここで流し切る。
    await drain();
    const now = await currentScene();
    if (now) {
      await openSection(now);
    } else if (typeof result.to === 'string') {
      await navTo('', result.to);
    }
    return '';
  }

  // ---------------------------------------------------------------- プロジェクトの入口
  //
  // `Ctrl+O`（`ui:project`）とプロジェクト名のボタンから開く面（§9.7・§13.5）。
  // 2 段の段取りは第 1 の画面と同じ: 変更を保存してから一覧へ戻る。保存に失敗している間は
  // 戻らない（仕様 §9.7 の `Ctrl+O` の統一）。

  /** 入口の面を開く（保存が済んでから）。 */
  async function openProjectPicker(): Promise<void> {
    await saveNow();
    if (screen.save === 'error' || screen.save === 'conflict') {
      // 保存できていないので、いまの作品から離れない（§8.2・§9.7）。
      return;
    }
    overlay = 'project';
  }

  /** 最近開いた一覧（`project.recent_list`。ブラウザでは機能核が `no_fs` を返す）。 */
  async function listProjects(): Promise<{ path: string; opened_at: string }[]> {
    const found = await store.run('project.recent_list');
    if (!found.ok) {
      throw new Error(found.message);
    }
    const projects = (found.result as { projects?: unknown }).projects;
    return Array.isArray(projects) ? (projects as { path: string; opened_at: string }[]) : [];
  }

  /** その作品を開く（`bridge.open` のあと、画面の材料を集め直す）。 */
  async function openProject(path: string): Promise<void> {
    await bridge.open(path);
    store.update((next) => {
      next.projectId = bridge.projectId();
      next.version = bridge.version();
      next.locale = bridge.locale();
    });
    await refresh();
  }

  /** 新しく作る（`project.new` → その場所を開く）。 */
  async function createProject(path: string, title: string): Promise<void> {
    const found = await store.run('project.new', { path, title });
    if (!found.ok) {
      throw new Error(found.message);
    }
    await openProject(path);
  }

  /** 履歴から消す（`project.recent_remove`。作品のデータは消さない）。 */
  async function forgetProject(path: string): Promise<void> {
    const found = await store.run('project.recent_remove', { path });
    if (!found.ok) {
      throw new Error(found.message);
    }
  }

  /** Zip を取り込む（`project.import_zip`。取り込んだ本文を画面へ反映する）。 */
  async function importProject(args: Record<string, unknown>): Promise<string[]> {
    const found = await store.run('project.import_zip', args);
    if (!found.ok) {
      throw new Error(found.message);
    }
    const written = (found.result as { written?: unknown }).written;
    const list = Array.isArray(written) ? written.filter((item): item is string => typeof item === 'string') : [];
    await refresh();
    return list;
  }

  /** Zip を書き出す（`project.export_zip`。ブラウザは `bytes` をダウンロードする）。 */
  async function exportProject(args: Record<string, unknown>): Promise<ZipExport | null> {
    const found = await store.run('project.export_zip', args);
    if (!found.ok) {
      throw new Error(found.message);
    }
    const result = found.result as Partial<ZipExport>;
    if (typeof result?.bytes === 'string' && typeof result?.name === 'string') {
      return { bytes: result.bytes, name: result.name, out: result.out };
    }
    // `out` を渡した（ネイティブ）ときは `bytes` が返らない。書いた先だけを持つ。
    return typeof result?.out === 'string' ? { bytes: '', name: '', out: result.out } : null;
  }

  /** 画面を離れるときにタイマを片付ける。 */
  export function stop(): void {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    ime.dispose();
    unsubscribe();
  }

  // タブを閉じるときの確認（§13.3.7）。未保存があるときだけブラウザに尋ねさせる。
  $: if (typeof window !== 'undefined') {
    window.onbeforeunload = screen.unsaved.length > 0 ? () => true : null;
  }
</script>

<AppShell
  {t}
  {screen}
  {before}
  {after}
  {ratio}
  {modes}
  {composing}
  oninput={oninput}
  oncaret={(position) => (caret = position)}
  onkey={onkey}
  onnav={nav}
  ondelete={remove}
  onjump={jump}
  onratio={setRatio}
  onworkspace={setWorkspace}
  onlanguage={toggleLanguage}
  ontheme={toggleTheme}
  onsave={retrySave}
  onreview={() => setWorkspace('review')}
  onproject={() => void openProjectPicker()}
  onswitcher={() => void openSwitcher()}
  onrename={() => void startRename()}
  onsplit={(position) => void splitSection(position)}
  onpropose={startProposal}
  onconflict={openConflicts}
  {conflicts}
  session={store}
  {revision}
  onopen={openRow}
  ondictionary={() => void openDictionary()}
  onplace={placeOfDiag}
  onattach={attachMaterial}
  onreviewgoto={(scene) => void nav(scene)}
/>

<!-- 重ね物（`?` のヘルプ / `Ctrl+P` のパレット）。`Esc` と外側クリックで閉じる（§9.7.5）。 -->
<Overlay
  {t}
  mode={overlay === 'help' || overlay === 'palette' ? overlay : null}
  commands={PALETTE_ROWS}
  onclose={() => (overlay = null)}
  onrun={runHelpRow}
  oncommand={(name) => void runPalette(name)}
/>

<!-- 切替パネル（`Ctrl+K`。§10.0 の一覧 + §9.7 の移動の口）。 -->
{#if overlay === 'switcher'}
  <Switcher
    {t}
    found={switcher}
    document={screen.path}
    onclose={() => (overlay = null)}
    onpick={(row) => void pickSwitcher(row)}
    onrename={() => void startRename()}
    scene={screen.scene}
    plotState={stateOf(screen.sections.find((row) => row.id === screen.scene)?.header ?? '')}
    ontogglestate={() => void togglePlotState(screen.scene)}
  />
{/if}

<!-- 文書の名前の変更（切替パネルの文書行・ヘッダのボタン。`document.rename`）。 -->
{#if renaming}
  <RenameDialog
    {t}
    from={renamingFrom}
    onclose={() => (renaming = false)}
    onrename={renameDocument}
  />
{/if}

<!-- 節の提案（`section.propose`。本文は変えない。§8.7.1）。 -->
{#if proposing}
  <ProposalDialog
    {t}
    scene={screen.scene}
    text={screen.text}
    onclose={() => (proposing = false)}
    onpropose={proposeSection}
  />
{/if}

<!-- 外部変更との衝突（`save.keep_mine` / `save.reload_external`。§8.5）。 -->
{#if conflicting}
  <ConflictDialog
    {t}
    {runtime}
    {conflicts}
    onclose={() => (conflicting = false)}
    ondiffs={conflictDiff}
    onkeep={(path) => resolveConflict(path, true)}
    onreload={(path) => resolveConflict(path, false)}
  />
{/if}

<!-- 参照の面（`cast.get` / `dictionary.list` / `dictionary.term_full`。§9.5.5・§9.7）。 -->
<LookupDialog
  {t}
  mode={lookup}
  subject={lookupSubject}
  cast={lookupCast}
  dictionary={lookupDictionary}
  terms={lookupTerms}
  failure={lookupFailure}
  busy={lookupBusy}
  onclose={closeLookup}
  onfull={(term) => void openTerm(term)}
/>

<!-- `lang.*`（LSP 相当の操作群）の結果の面（§9.7.1 の `>` パレットから開く）。 -->
<LangDialog
  {t}
  mode={langMode}
  subject={langSubject}
  rows={langRows}
  count={langCount}
  failure={langFailure}
  busy={langBusy}
  onclose={closeLang}
  onjump={(value) => void jumpLocation(value)}
  onapply={applyRename}
/>

<!-- プロジェクトの入口の面（`Ctrl+O` とプロジェクト名。§9.7・§13.5）。 -->
{#if overlay === 'project'}
  <ProjectPicker
    {t}
    {runtime}
    current={screen.projectId}
    onclose={() => (overlay = null)}
    onlist={listProjects}
    onopen={openProject}
    oncreate={createProject}
    onforget={forgetProject}
    onimport={importProject}
    onexport={exportProject}
  />
{/if}
