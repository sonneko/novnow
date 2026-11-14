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
  import { Ime } from './lib/keyboard/ime';
  import { command as commandOf, isLocal, keepPayload, strokeOf } from './lib/keyboard/keys';
  import { texts, type Locale, type Texts } from './lib/i18n';
  import type { Session } from './lib/store/session';
  import { Session as SessionStore } from './lib/store/session';
  import type { Bridge, SaveState } from './lib/bridge/types';

  /** 橋渡し（wasm）。 */
  export let bridge: Bridge;

  /** 画面の状態（Svelte 5 の rune。`$state` を使えない箇所でも動くようストアで持つ）。 */
  let store = new SessionStore(bridge);
  let screen = store.state;
  let t: Texts['t'] = texts('ja').t;

  /** 変換中（IME）。 */
  let composing = false;
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
    if (found === 'ui:toggle-reference') {
      store.update((next) => {
        next.referenceVisible = !next.referenceVisible;
      });
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
    }
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
  oncaret={() => {}}
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
/>
