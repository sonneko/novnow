<script lang="ts">
  // `write` ワークスペース（§10.1）。
  //
  // 構成は §9.1 のとおり:
  //
  //   ┌───────────────────────┬──────────────┐
  //   │ 文脈ビュー（55%）      │ 参照ペイン    │
  //   ├───────────────────────┤              │
  //   │ 節エディタ（45%）      │              │
  //   └───────────────────────┴──────────────┘
  //
  // 比率は `ui.json` の `pane_ratio.main` / `pane_ratio.context_view` に保存する（§9.1）。
  // 保存は `settings.set`（正規操作）を通す（§21.1.3）。画面が勝手にファイルを書かない。
  //
  // ## このレーンが数えるもの（§9.3.2・§9.4.2）
  //
  // 台詞比率と節の文字数は**この画面で数える**（打鍵から 200ms のデバウンス。§9.3.2 の 6・
  // §9.4.2 の 6）。数え方は `editor/ratio.ts` に置いた（決定論・第 1 の `metrics::section`
  // と同じ規則）。シェル（L0）から渡る `ratio` / `modes` は W2 の近似値なので使わない
  // （どの節でも「同じ本文から同じ値」になることをこのレーンで保証する）。
  //
  // ## このレーンが持つ状態（セッション内だけ）
  //
  // 句切れ提案のオン・オフ（§9.4.3）と、文脈ビューの折りたたみ（§9.3.3）は、いまのところ
  // **画面の中だけ**で持つ。`ui.json` の `suggest_comma` / `folded.context_view` へ書く配線
  // （`settings.get` / `settings.set` を呼ぶ口）はシェル（L0）の持ち物で、このレーンからは
  // 呼べない。黙って「保存した」ことにしないため、画面の下に**その旨を明記する**（§21.1.2）。
  import { tick } from 'svelte';
  import SectionEditor from '../../editor/SectionEditor.svelte';
  import ContextView from '../../context/ContextView.svelte';
  import ReferencePane from '../../reference/ReferencePane.svelte';
  import Splitter from '../../shell/Splitter.svelte';
  import { Debounced, dialogueRatio } from '../../editor/ratio';
  import { activeModes, modesLabel } from '../../editor/modes';
  import { charsWithoutSpace, lineBytes, sectionByteOf } from '../../editor/text';
  import { foldAll, foldKey, unfoldKey } from '../../context/fold';
  import type { Screen, Session } from '../../store/session';
  import type { Texts } from '../../i18n';

  /** 文言。 */
  export let t: Texts['t'];
  /** 画面の状態。 */
  export let screen: Screen;
  /** 変換中か（IME）。 */
  export let composing = false;
  /** 前後の節（`section.get`）。 */
  export let before: { scene: string; text: string }[] = [];
  export let after: { scene: string; text: string }[] = [];
  /**
   * 台詞比率（§9.4.2。0〜1）と有効モードの表示。どちらも**シェルの近似値は使わない**
   * （この画面で数え直す）。シェルからは渡ってくるが、受け取るだけで中身は読まない。
   */
  export let ratio = 0;
  export let modes = '';
  // 受け取っただけの値（捨てる）。未使用の警告を出さないよう、ここで 1 度だけ読む。
  $: {
    void ratio;
    void modes;
  }
  /**
   * セッション（正規操作を呼ぶ口。§21.1.3）。**シェル（L0）から受け取る想定で、まだ繋がっていない。**
   *
   * この画面が自分で呼ぶのは `diagnostics.run` と `document.get` の 2 つだけ。
   * 行内の下線（§11.3）を打鍵に追随させるには、`diagnostics.run` の `line`（**文書の中**）を
   * 節の中の byte へ直す必要があり、そのためには節の先頭の byte が要る。節の先頭は
   * `document.get` の本文を数えないと分からない。
   *
   * `store` が `null` のあいだは、**下線を 1 本も出さない**（位置のずれた下線を引くより
   * 出さないほうがよい。`sectionStart()` が `null` を返す）。下線がまだ出ないことは
   * 画面に明記する（§21.1.2 MUST）。
   */
  export let store: Session | null = null;
  /** 本文が変わった。 */
  export let oninput: ((text: string) => void) | null = null;
  /** キャレットが動いた。 */
  export let oncaret: ((position: number) => void) | null = null;
  /** キー（エディタが扱わなかったもの）。 */
  export let onkey: ((event: KeyboardEvent) => void) | null = null;
  /** 節を選んだ。 */
  export let onnav: ((scene: string) => void) | null = null;
  /** 節を消した。 */
  export let ondelete: ((scene: string) => void) | null = null;
  /** 参照ペインの行を押した。 */
  export let onjump: ((kind: string, value: unknown) => void) | null = null;
  /** 比率を保存した。 */
  export let onratio: ((key: string, value: number) => void) | null = null;
  /** 節を分ける（`section.split`。`Ctrl+Enter`。§8.1.1）。 */
  export let onsplit: ((caret: number) => void) | null = null;
  /** この節の提案を置く（`section.propose`。§8.7.1。本文は変えない）。 */
  export let onpropose: (() => void) | null = null;
  /** 参照ペインを出すか（`Ctrl+\`）。 */
  export let reference = true;
  /**
   * 設定を `ui.json` へ保存できる口（`settings.set`）を受け取ったか。
   *
   * シェル（L0）がまだ繋いでいないので既定は `false`。**繋がっていないあいだは
   * 「保存されない」ことを画面に明記する**（§21.1.2 MUST: 未提供を黙って隠さない）。
   */
  export let persistable = false;

  /** エディタ（`Ctrl+2` でフォーカスするために持つ）。 */
  let editor: SectionEditor | null = null;

  /** 分割比（`pane_ratio.main` / `pane_ratio.context_view`）。 */
  let main = 0.62;
  let context = 0.55;

  /** 台詞比率（§9.4.2。200ms デバウンス後）。 */
  let measured = 0;
  /** 節の文字数（§9.3.2。200ms デバウンス後）。 */
  let chars = 0;
  /** 句切れ提案（§9.4.3。既定はオフ。セッション内だけ）。 */
  let suggestComma = false;
  /** 畳んでいる単位（§9.3.3。セッション内だけ）。 */
  let folded: string[] = [];
  /**
   * 文脈ビューでポインタの載っている節（`Ctrl+Shift+[` / `]` はこの単位を畳む。§9.3.3）。
   * 載っていなければ現在の節を対象にする。
   */
  let hovered = '';
  /** `Ctrl+K` を押した直後か（`Ctrl+K Ctrl+0` / `Ctrl+K Ctrl+J` の 2 打鍵。§9.3.3）。 */
  let chordPending = false;
  let chordTimer: ReturnType<typeof setTimeout> | null = null;

  /** 長い文の閾値（`writing.long_sentence_chars` の既定。§9.4.3）。 */
  const longSentenceChars = 120;

  /** 台詞比率のデバウンス（§9.4.2 の 6）。 */
  const ratioDebounce = new Debounced<number>((next) => (measured = next), 0, 200);
  /** 文字数のデバウンス（§9.3.2）。 */
  const charsDebounce = new Debounced<number>((next) => (chars = next), 0, 200);

  /** 本文が変わったら数え直す（最初の 1 回はすぐ数える）。 */
  $: if (screen.text !== undefined) {
    const nextChars = charsWithoutSpace(screen.text);
    if (firstMeasure) {
      firstMeasure = false;
      chars = nextChars;
      measured = dialogueRatio(screen.text).value;
    } else {
      charsDebounce.set(nextChars);
      ratioDebounce.set(dialogueRatio(screen.text).value);
    }
  }
  let firstMeasure = true;

  /** 節が変わったら、畳んだ単位のうち現在の節を含むものを開く（§9.3.3 の MUST）。 */
  $: if (screen.scene && folded.some((key) => containsKey(key, screen.scene))) {
    folded = folded.filter((key) => !containsKey(key, screen.scene));
  }

  /** 有効モード（§9.4.2。`'リリア B+C`）。 */
  $: activeLabel = modesLabel(activeModes(screen.text, castOf(screen.reference)));

  /**
   * この節の診断（§11.3 の下線。`byte` は**節の中**の位置）。
   *
   * 材料は 2 つ:
   *
   * 1. `reference.pane.diagnostics`（節を開いたときのもの。`byte` は**文書の中**の byte）
   * 2. `diagnostics.run`（打鍵から 400ms 後に自分で引き直す。`line` は**文書の中**の行）
   *
   * 2 を足すのは、シェル（L0）が節を開き直したときにしか `reference.pane` を取り直さず、
   * 打鍵した瞬間の誤り（`'存在しない名前` など）に下線が出ないため（§11.3 の MUST）。
   *
   * どちらも文書の座標なので、節の先頭（`section.lines` の 1 行目の先頭）を引いて
   * 節の中の byte に直す。引けないとき（節の行が分からない）は下線を出さない。
   */
  let fetched: { byte: number | null; code: string; message: string; severity: string }[] = [];
  /** 診断の引き直し（打鍵から 400ms）。 */
  const diagnosticsDebounce = new Debounced<null>(() => void refreshDiagnostics(), null, 400);
  /** 引き直しの途中か（重ねて呼ばない）。 */
  let refreshing = false;

  /** 診断を引き直す（`diagnostics.run`。失敗しても画面は止めない）。 */
  async function refreshDiagnostics(): Promise<void> {
    if (!store || !screen.scene || refreshing) {
      return;
    }
    refreshing = true;
    try {
      const found = await store.run('diagnostics.run', { scene: screen.scene });
      if (!found.ok) {
        return;
      }
      const base = await sectionStart();
      if (base === null) {
        return;
      }
      const rows = Array.isArray(found.result.diagnostics) ? found.result.diagnostics : [];
      fetched = (rows as Record<string, unknown>[])
        .filter((row): row is Record<string, unknown> => typeof row === 'object' && row !== null)
        .filter((row) => row['path'] === screen.path)
        // `info` は行に下線を引かない（§11.3 の表。下線は error・warning だけ）
        .filter((row) => row['severity'] === 'error' || row['severity'] === 'warning')
        .map((row) => {
          const line = typeof row['line'] === 'number' ? (row['line'] as number) : 0;
          return {
            byte: sectionByteOf(base.byLine, base.start, line),
            code: typeof row['code'] === 'string' ? row['code'] : '',
            message: typeof row['message'] === 'string' ? row['message'] : '',
            severity: typeof row['severity'] === 'string' ? row['severity'] : 'warning',
          };
        })
        .filter((row) => row.byte !== null && row.byte >= 0);
    } finally {
      refreshing = false;
    }
  }

  /**
   * 節の先頭の byte（文書の中）と、行番号 → 行頭の byte（文書の中）。
   *
   * `section.list` の `lines` は**節の本文の行範囲**（0 始まり・見出し行と区切り `;` を
   * 含まない。`REF:src/core/features/sections.rs` の `text_of_lines` と同じ範囲）なので、
   * その先頭の行の行頭が節の先頭になる。取れなければ `null`（下線は出さない。
   * 位置のずれた下線を引くより、出さないほうがよい）。
   */
  async function sectionStart(): Promise<{ start: number; byLine: Map<number, number> } | null> {
    if (!store || !screen.path) {
      return null;
    }
    const ref = screen.sections.find((row) => row.id === screen.scene);
    if (!ref) {
      return null;
    }
    const doc = await store.run('document.get', { path: screen.path });
    if (!doc.ok) {
      return null;
    }
    const byLine = lineBytes(doc.result.text);
    const start = byLine.get(ref.lines[0] + 1);
    if (start === undefined) {
      return null;
    }
    return { start, byLine };
  }

  // 本文が変わったら診断を引き直す（節を開いた直後の 1 回は `reference.pane` があるので待つ）
  $: if (screen.text !== undefined && !firstMeasure) {
    diagnosticsDebounce.set(null);
  }

  /** 診断（2 つの材料の和。同じコード・同じ位置は 1 つ）。 */
  $: diagnostics = (() => {
    const pane = screen.reference?.['diagnostics'];
    const listed: { byte: number | null; code: string; message: string; severity: string }[] =
      Array.isArray(pane)
        ? (pane as Record<string, unknown>[])
            .filter((row): row is Record<string, unknown> => typeof row === 'object' && row !== null)
            .filter((row) => typeof row['byte'] === 'number')
            .map((row) => ({
              byte: row['byte'] as number,
              code: typeof row['code'] === 'string' ? row['code'] : '',
              message: typeof row['message'] === 'string' ? row['message'] : '',
              severity: typeof row['severity'] === 'string' ? row['severity'] : 'warning',
            }))
        : [];
    // `reference.pane` の `byte` も文書の中の byte。節の先頭を引いて節の中へ直す。
    const base = paneBase;
    const local = base === null ? [] : listed.map((row) => ({ ...row, byte: (row.byte ?? 0) - base }));
    const seen = new Set<string>();
    const out: typeof listed = [];
    for (const row of [...local, ...fetched]) {
      if (row.byte === null || row.byte < 0) {
        continue;
      }
      const key = `${row.code}@${row.byte}`;
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      out.push(row);
    }
    return out;
  })();

  /** `reference.pane` の `byte` を節の中へ直すための、節の先頭（文書の中の byte）。 */
  let paneBase: number | null = null;

  // 節が変わったら、節の先頭の byte を取り直す（下線の位置合わせ。§11.3）
  $: if (store && screen.scene && screen.path) {
    const wanted = screen.scene;
    void sectionStart().then((found) => {
      // 節が変わっていたら捨てる（前の節の先頭で下線を引かない）
      if (wanted !== screen.scene) {
        return;
      }
      paneBase = found === null ? null : found.start;
    });
  }

  /** 参照ペインの `cast` を取り出す（形は `raw` のまま。解釈は `editor/modes.ts`）。 */
  function castOf(pane: Record<string, unknown> | null): { name: string; modes: string[] }[] {
    const found = pane?.['cast'];
    if (!Array.isArray(found)) {
      return [];
    }
    return (found as Record<string, unknown>[])
      .filter((row): row is Record<string, unknown> => typeof row === 'object' && row !== null)
      .map((row) => ({
        name: typeof row['name'] === 'string' ? row['name'] : '',
        modes: Array.isArray(row['modes'])
          ? (row['modes'] as unknown[]).filter((mode): mode is string => typeof mode === 'string')
          : [],
      }))
      .filter((row) => row.name.length > 0);
  }

  /** その鍵が節を含むか（`@01-04` は `01-04-02-03` を含む）。 */
  function containsKey(key: string, scene: string): boolean {
    const prefix = key.replace(/^@/u, '');
    return prefix.length > 0 && (scene === prefix || scene.startsWith(`${prefix}-`));
  }

  /** メインと参照の比を変えた（5% 刻みで保存）。 */
  function setMain(next: number): void {
    main = next;
    onratio?.('pane_ratio.main', next);
  }

  /** 文脈ビューと節エディタの比を変えた。 */
  function setContext(next: number): void {
    context = next;
    onratio?.('pane_ratio.context_view', next);
  }

  /**
   * 畳む（`Ctrl+Shift+[`。§9.3.3）。
   *
   * 「カーソル位置の 1 階層上」を畳む。現在の節を含む単位は畳めない（MUST）ので、
   * `foldKey` が「その 1 つ上」を返す。
   */
  function foldHere(): boolean {
    const key = foldKey(hovered || screen.scene, screen.scene, folded);
    if (!key) {
      return false;
    }
    folded = [...folded, key];
    return true;
  }

  /** 展開する（`Ctrl+Shift+]`。§9.3.3）。 */
  function unfoldHere(): boolean {
    const key = unfoldKey(hovered || screen.scene, folded);
    if (!key) {
      return false;
    }
    folded = folded.filter((found) => found !== key);
    return true;
  }

  /**
   * 章だけが見える状態にする（`Ctrl+K Ctrl+0`）。
   *
   * 「すべて畳む」は章の見出しだけが残る状態（§9.3.3）。ただし**現在の節を含む単位は
   * 畳めない**（MUST）ので、現在の節の章だけは畳まない（その章の本文は見えたまま）。
   */
  function foldEverything(): void {
    folded = foldAll(screen.sections.map((found) => found.id)).filter(
      (key) => !containsKey(key, screen.scene),
    );
  }

  /** すべて展開する（`Ctrl+K Ctrl+J`）。 */
  function unfoldEverything(): void {
    folded = [];
  }

  /** 畳んだ単位を開く（文脈ビューの 1 行を押したとき）。 */
  function unfoldKey_(key: string): void {
    folded = folded.filter((found) => found !== key);
  }

  /**
   * ワークスペースのキー（§9.3.3 の折りたたみ・§9.4.3 の句切れ提案）。
   *
   * `Ctrl+Shift+[` / `]` は全体のキー写像では構成ツリーの開閉（`plot.outline_fold`）に
   * 割り当てられている。**`write` の画面では**文脈ビューを畳むのが §9.3.3 の意味なので、
   * この画面に居るときだけ先に受け取る（掴んだキーは後ろへ流さない）。
   *
   * `Ctrl+K Ctrl+0` / `Ctrl+K Ctrl+J` は表に無い 2 打鍵なので、ここで受ける。
   */
  function onWindowKey(event: KeyboardEvent): void {
    if (event.isComposing || composing) {
      return;
    }
    const ctrl = event.ctrlKey || event.metaKey;
    // `Ctrl+Alt+,`: 句切れ提案のトグル（§9.4.3）
    if (ctrl && event.altKey && !event.shiftKey && (event.key === ',' || event.code === 'Comma')) {
      event.preventDefault();
      event.stopPropagation();
      suggestComma = !suggestComma;
      return;
    }
    // `Ctrl+Shift+[` / `]`: 文脈ビューの折りたたみ（§9.3.3）
    if (ctrl && event.shiftKey && !event.altKey && (event.key === '[' || event.code === 'BracketLeft')) {
      event.preventDefault();
      event.stopPropagation();
      foldHere();
      return;
    }
    if (ctrl && event.shiftKey && !event.altKey && (event.key === ']' || event.code === 'BracketRight')) {
      event.preventDefault();
      event.stopPropagation();
      unfoldHere();
      return;
    }
    // `Ctrl+K Ctrl+0` / `Ctrl+K Ctrl+J`（§9.3.3）
    if (chordPending && ctrl && !event.shiftKey && !event.altKey) {
      const key = event.key.toLowerCase();
      if (key === '0' || key === 'j') {
        event.preventDefault();
        event.stopPropagation();
        chordPending = false;
        if (key === '0') {
          foldEverything();
        } else {
          unfoldEverything();
        }
        return;
      }
    }
    if (ctrl && !event.shiftKey && !event.altKey && event.key.toLowerCase() === 'k') {
      chordPending = true;
      if (chordTimer !== null) {
        clearTimeout(chordTimer);
      }
      chordTimer = setTimeout(() => {
        chordPending = false;
        chordTimer = null;
      }, 1500);
    }
  }

  /** エディタへフォーカスする（`Ctrl+2`）。 */
  export function focusEditor(): void {
    editor?.focus();
  }

  /** エディタのキャレット（UTF-16。`nav.jump` に渡す）。 */
  export function caret(): number {
    return editor?.position() ?? 0;
  }

  /** エディタの行（1 始まり）。 */
  export function line(): number {
    return editor?.line() ?? 1;
  }
</script>

<svelte:window on:keydown|capture={onWindowKey} />

<div
  class="workspace"
  style={`grid-template-columns:${reference ? `${Math.round(main * 100)}% auto 1fr` : '1fr'};`}
>
  <div class="main" style={`grid-template-rows:${Math.round(context * 100)}% auto 1fr auto;`}>
    <ContextView
      {t}
      scene={screen.scene}
      {before}
      {after}
      sectionChars={chars}
      target={null}
      save={screen.save}
      {folded}
      foldable
      onnav={onnav}
      {ondelete}
      onfold={unfoldKey_}
      onhover={(scene) => (hovered = scene)}
    />
    <Splitter axis="vertical" ratio={context} onchange={setContext} onmove={(next) => (context = next)} />
    <SectionEditor
      bind:this={editor}
      t={t}
      section={screen.sections.find((found) => found.id === screen.scene) ?? null}
      text={screen.text}
      {composing}
      ratio={measured}
      modes={activeLabel}
      {suggestComma}
      {longSentenceChars}
      {diagnostics}
      oninput={oninput}
      oncaret={oncaret}
      onkey={onkey}
      {onsplit}
      {onpropose}
    />
    {#if !persistable || !store}
      <!-- この版でまだ提供していないもの（§21.1.2 MUST: 黙って隠さない）。
           設定の保存（`suggest_comma` / `folded.context_view`）と、節の目標字数（`length`）、
           行内の下線（`store` が無いと節の先頭の byte を引けないので位置を出せない。§11.3）。 -->
      <p class="limits" role="note">
        {#if !persistable}
          {t('workspace.unavailable', [t('api.op.settings.set')])}
          ·
          {t('workspace.unavailable', [t('stage.length_target_ratio')])}
        {/if}
        {#if !store}
          {#if !persistable}·{/if}
          {t('workspace.unavailable', [t('api.op.diagnostics.run')])}
        {/if}
      </p>
    {/if}
  </div>
  {#if reference}
    <Splitter axis="horizontal" ratio={main} onchange={setMain} onmove={(next) => (main = next)} />
    <ReferencePane {t} pane={screen.reference} onjump={onjump} />
  {/if}
</div>

<style>
  .workspace {
    display: grid;
    /* シェルの `main.body` は素のブロックなので、高さを明示しないと中身の高さに縮み、
       下に余白が残る（実測で画面 900px に対し 663px で止まっていた）。
       ヘッダとステータスバーの分を引いた残りを埋める。§9.1 */
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }
  .main {
    display: grid;
    min-height: 0;
    min-width: 0;
    /* 参照ペインは 320〜560px（§9.1） */
    contain: layout;
  }
  .limits {
    margin: 0;
    padding: 2px 8px 4px;
    color: var(--fg-faint);
    font-size: 10px;
    line-height: 1.4;
  }
  @media (max-width: 900px) {
    .workspace {
      /* 900px 未満は参照ペインを畳む（`Ctrl+3` でオーバーレイ。§9.1） */
      grid-template-columns: 1fr !important;
    }
  }
</style>
