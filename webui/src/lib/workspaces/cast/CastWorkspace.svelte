<script lang="ts">
  // `cast` ワークスペース（§10.6）。配役・モード・役割語・呼称。
  //
  // 構成は §10.6 のとおり:
  //
  //   ┌────────────┬──────────────────────────┬──────────────┐
  //   │ キャラ一覧   │ characters.md の編集     │ タブ（既定: モード）│
  //   │ （reading） │                        │ モード/役割語/呼称 │
  //   └────────────┴──────────────────────────┴──────────────┘
  //
  // 材料は正規操作の戻り値だけ（§11.11.5。`characters.md` の構造解析を自前でしない）。
  // 使える操作: `cast.list` / `cast.timeline` / `cast.scan` / `dictionary.*` /
  // `document.get`・`document.write`（`characters.md` の全文）。
  //
  // 凍結カタログに無い操作のため**未提供**なもの（bridge 提案待ち。画面は空にせず告知）:
  // - モードタイムラインの `range` 書き換え（帯のドラッグ編集）
  // - 役割語テスターの入力台詞に対する逐語マッチ（`cast.scan` はプロジェクト全体の未一致一覧）
  // - 呼称マトリクスのセル書き込み（`address:` の読み書き操作が無い）
  // これらは布局と表示のみ整え、操作できないことを通知で示す（§21.1.2）。
  import type { OpArgs, OpId } from '../../bridge/types';
  import type { Outcome } from '../../store/session';
  import type { Texts } from '../../i18n';
  import { DIAGONAL, buildMatrix, cellText, sortByReading, type AddressCell } from './cast';

  /** 文言。 */
  export let t: Texts['t'];
  /** 正規操作 1 回（`Session.run` と同じ手順）。 */
  export let run: <T extends OpId>(op: T, args?: OpArgs[T]) => Promise<Outcome<T>>;
  /** 節へ移る（`cast.scan` の未一致行・呼称違反から）。 */
  export let onnav: ((scene: string) => void) | null = null;
  /** 通知（操作の失敗など。§21.1.2 で黙って別の結果にしない）。 */
  export let onnotice: ((text: string, kind?: 'info' | 'error') => void) | null = null;
  /** 変換中か（IME）。 */
  export let composing = false;

  /** タブ（§10.6。既定は「モード」）。 */
  let tab: 'mode' | 'speech' | 'address' = 'mode';

  /** キャラ一覧（`cast.list`）。 */
  let characters: { index: number; name: string; reading: string }[] = [];
  /** 選んだキャラの index。 */
  let selected = 0;

  /** モードタイムライン（`cast.timeline`）。 */
  let bands: { id: string; label: string; range: string; line: number; covered: boolean[] }[] = [];
  let chapters: number[] = [];

  /** 全モード一覧（`dictionary.modes_list`）。 */
  let allModes: { character: string; index: number; id: string; label: string; range: string }[] = [];

  /** `characters.md` の全文（中央の編集欄）。 */
  let docText = '';
  /** 編集欄の更新待ち（打鍵から 400ms デバウンス）。 */
  let docTimer: ReturnType<typeof setTimeout> | null = null;
  let docDirty = false;

  /** 役割語の走査結果（`cast.scan`）。 */
  let unmatched: { scene: string; speaker: string; text: string }[] = [];
  let scanCount = 0;

  /** 呼称マトリクス（`address:` の読み書き操作が無いため、名前のみで構成。セルは未定義）。 */
  let matrix: AddressCell[][] = [];

  /** キャラ一覧・タイムライン・全文を読み直す。 */
  async function load(): Promise<void> {
    const list = await run('cast.list');
    if (list.ok) {
      characters = (list.result.characters as unknown as { index: number; name: string; reading: string }[]) ?? [];
      if (selected >= characters.length) {
        selected = Math.max(0, characters.length - 1);
      }
    }
    const timeline = await run('cast.timeline', { index: selected });
    if (timeline.ok) {
      const found = timeline.result as unknown as {
        bands: { id: string; label: string; range: string; line: number; covered: boolean[] }[];
        chapters: number[];
      };
      bands = found.bands ?? [];
      chapters = found.chapters ?? [];
    }
    const modes = await run('dictionary.modes_list');
    if (modes.ok) {
      allModes = (modes.result as unknown as { modes: { character: string; index: number; id: string; label: string; range: string }[] }).modes ?? [];
    }
    const doc = await run('document.get', { path: 'characters.md' });
    if (doc.ok) {
      docText = (doc.result as unknown as { text: string }).text;
    }
    loadMatrix();
  }

  /** 呼称マトリクス（名前のみ。セルは未定義を示す空）。 */
  function loadMatrix(): void {
    const rows = sortByReading(characters.map((row) => ({ name: row.name, reading: row.reading ?? '' })));
    const cells = new Map<string, Map<string, AddressCell>>();
    matrix = buildMatrix(
      characters.map((row) => ({ name: row.name, reading: row.reading ?? '' })),
      cells,
    );
    void rows;
  }

  /** キャラを選んだ。 */
  function select(index: number): void {
    selected = index;
    void load();
  }

  /** キャラを登録（`dictionary.register_character`。§9.10 の段階 5 と同じ口）。 */
  async function addCharacter(): Promise<void> {
    const name = prompt(t('pane.add_character'));
    if (!name) {
      return;
    }
    const found = await run('dictionary.register_character', { name, role: '' });
    if (!found.ok) {
      onnotice?.(found.message, 'error');
      return;
    }
    await load();
  }

  /** `characters.md` の全文を書き直す（`document.write`。節単位でない全文の書き込み）。 */
  async function writeDoc(): Promise<void> {
    if (docTimer !== null) {
      clearTimeout(docTimer);
      docTimer = null;
    }
    const found = await run('document.write', { path: 'characters.md', text: docText, role: '' });
    if (!found.ok) {
      onnotice?.(found.message, 'error');
      return;
    }
    docDirty = false;
  }

  /** 編集欄の打鍵（400ms デバウンスで書き戻す）。 */
  function onDocInput(text: string): void {
    docText = text;
    docDirty = true;
    if (docTimer !== null) {
      clearTimeout(docTimer);
    }
    docTimer = setTimeout(() => void writeDoc(), 400);
  }

  /** 役割語の走査（`cast.scan`。§10.6 の `Ctrl+Shift+V`）。 */
  async function scan(): Promise<void> {
    const found = await run('cast.scan', { role: '' });
    if (!found.ok) {
      onnotice?.(found.message, 'error');
      return;
    }
    const result = found.result as unknown as { count: number; unmatched: { scene: string; speaker: string; text: string }[] };
    unmatched = result.unmatched ?? [];
    scanCount = result.count ?? unmatched.length;
  }

  /** 未一致の台詞を押した（本文へ）。 */
  function openUnmatched(scene: string): void {
    onnav?.(scene);
  }

  /** キー（`cast` 固有。§10.6）。 */
  function onkey(event: KeyboardEvent): void {
    if (composing) {
      return;
    }
    if (event.key === 'Tab' && !event.shiftKey) {
      event.preventDefault();
      tab = tab === 'mode' ? 'speech' : tab === 'speech' ? 'address' : 'mode';
      return;
    }
    if (event.key === 's' && event.ctrlKey) {
      event.preventDefault();
      void scan();
    }
  }

  /** 初期読み込み。 */
  $: void load();
</script>

<svelte:window on:keydown={onkey} />

<div class="cast">
  <!-- 左: キャラ一覧（§10.6。`reading` 順） -->
  <aside class="list" aria-label={t('header.workspace.cast')}>
    <div class="list-head">
      <button type="button" class="add" on:click={() => void addCharacter()} title={t('pane.add_character')}>
        +
      </button>
    </div>
    {#if characters.length === 0}
      <p class="empty">{t('cast.none')}</p>
    {:else}
      {#each characters as row, index (row.name)}
        <button
          type="button"
          class="character"
          class:active={index === selected}
          aria-current={index === selected ? 'true' : undefined}
          on:click={() => select(index)}
        >
          <span class="name">{row.name}</span>
          <span class="reading">{row.reading}</span>
        </button>
      {/each}
    {/if}
  </aside>

  <!-- 中: characters.md の編集（§10.6。左端 3px の縦線は §9.2.4） -->
  <section class="editor" aria-label={t('pane.cast')}>
    <textarea
      class="doc"
      value={docText}
      on:input={(event) => onDocInput(event.currentTarget.value)}
      spellcheck={false}
    ></textarea>
    {#if docDirty}
      <span class="dirty" aria-hidden="true">●</span>
    {/if}
  </section>

  <!-- 右: タブ（§10.6。既定「モード」） -->
  <aside class="tabs" aria-label={t('cast.timeline')}>
    <div class="tab-bar" role="tablist">
      <button type="button" role="tab" aria-selected={tab === 'mode'} class:active={tab === 'mode'} on:click={() => (tab = 'mode')}>
        {t('cast.tab_modes')}
      </button>
      <button type="button" role="tab" aria-selected={tab === 'speech'} class:active={tab === 'speech'} on:click={() => (tab = 'speech')}>
        {t('cast.tab_tester')}
      </button>
      <button type="button" role="tab" aria-selected={tab === 'address'} class:active={tab === 'address'} on:click={() => (tab = 'address')}>
        {t('stage.addresses')}
      </button>
    </div>

    {#if tab === 'mode'}
      <!-- モードタイムライン（§10.6）。帯は表示のみ（書き換え操作は bridge に無い）。 -->
      <div class="timeline">
        {#if chapters.length > 0}
          <div class="chapters">
            {#each chapters as chapter (chapter)}
              <span class="chapter">@{String(chapter).padStart(2, '0')}</span>
            {/each}
          </div>
        {/if}
        {#if bands.length === 0}
          <p class="empty">{t('cast.no_modes')}</p>
        {:else}
          {#each bands as band (band.id)}
            <div class="band">
              <span class="band-label" title={band.range || t('cast.range_all')}>{band.label}</span>
              <span class="band-track">
                {#each band.covered as covered, at (at)}
                  <span class="band-cell" class:on={covered}></span>
                {/each}
              </span>
            </div>
          {/each}
        {/if}
        <p class="hint">{t('cast.timeline_keys')}</p>
      </div>
    {:else if tab === 'speech'}
      <!-- 役割語（§10.6）。`cast.scan` で未一致の台詞を一覧。 -->
      <div class="speech">
        <button type="button" class="scan" on:click={() => void scan()}>{t('cast.scan_button')}</button>
        {#if scanCount > 0}
          <p class="count">{t('cast.unmatched_count', [String(scanCount)])}</p>
          <ul class="unmatched">
            {#each unmatched as row, index (index)}
              <li>
                <button type="button" class="line" on:click={() => openUnmatched(row.scene)}>
                  <span class="speaker">{row.speaker || t('stage.unknown')}</span>
                  <span class="text">{row.text}</span>
                </button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="empty">{t('cast.all_matched')}</p>
        {/if}
      </div>
    {:else}
      <!-- 呼称マトリクス（§10.6.1）。セル書き込みは bridge に操作が無い。 -->
      <div class="address">
        {#if matrix.length === 0}
          <p class="empty">{t('cast.none')}</p>
        {:else}
          <table class="matrix">
            <thead>
              <tr>
                <th scope="col" class="corner"></th>
                {#each characters as column (column.name)}
                  <th scope="col">{column.name}</th>
                {/each}
              </tr>
            </thead>
            <tbody>
              {#each characters as row, rowIndex (row.name)}
                <tr>
                  <th scope="row">{row.name}</th>
                  {#each matrix[rowIndex] as cell, colIndex (colIndex)}
                    <td class:diagonal={rowIndex === colIndex}>
                      {#if rowIndex === colIndex}
                        <span class="diag">{DIAGONAL}</span>
                      {:else}
                        <span class="cell">{cellText(cell) || '—'}</span>
                      {/if}
                    </td>
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
          <p class="hint">{t('stage.addr_edit_hint')}</p>
        {/if}
      </div>
    {/if}
  </aside>
</div>

<style>
  .cast {
    display: grid;
    grid-template-columns: 220px 1fr 320px;
    min-height: 0;
    min-width: 0;
    overflow: hidden;
  }
  @media (max-width: 1100px) {
    .cast {
      grid-template-columns: 180px 1fr 280px;
    }
  }
  @media (max-width: 900px) {
    .cast {
      grid-template-columns: 1fr;
      grid-template-rows: auto auto auto;
    }
  }
  /* 左: キャラ一覧 */
  .list {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: auto;
    padding: 8px;
    border-right: 1px solid var(--border);
    background: var(--bg-raised);
  }
  .list-head {
    display: flex;
    justify-content: flex-end;
    margin-bottom: 4px;
  }
  .add {
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 16px;
    cursor: pointer;
    text-decoration: underline;
    text-decoration-color: var(--border);
  }
  .add:hover {
    background: var(--bg-selection);
    color: var(--fg-primary);
  }
  .character {
    display: flex;
    flex-direction: column;
    gap: 0;
    width: 100%;
    border: 0;
    background: transparent;
    color: var(--fg-primary);
    text-align: left;
    padding: 4px 6px;
    cursor: pointer;
    border-radius: 2px;
    text-decoration: underline;
    text-decoration-color: var(--border);
  }
  .character:hover {
    background: var(--bg-selection);
  }
  .character.active {
    background: var(--bg-selection);
    color: var(--accent);
  }
  .character .name {
    font-size: 13px;
  }
  .character .reading {
    font-size: 10px;
    color: var(--fg-faint);
  }
  .empty {
    margin: 8px;
    color: var(--fg-faint);
    font-size: 12px;
  }
  /* 中: characters.md の編集（§9.2.4 の左端 3px 縦線） */
  .editor {
    position: relative;
    display: flex;
    min-height: 0;
    overflow: hidden;
    border-right: 1px solid var(--border);
    background: var(--bg-base);
  }
  .editor::before {
    content: '';
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--accent-faint);
  }
  .doc {
    flex: 1 1 auto;
    min-width: 0;
    width: 100%;
    padding: 12px 12px 12px 16px;
    border: 0;
    background: transparent;
    color: var(--fg-primary);
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 13px;
    line-height: 1.8;
    resize: none;
    outline: none;
  }
  .dirty {
    position: absolute;
    right: 8px;
    bottom: 6px;
    color: var(--state-unsaved);
    font-size: 10px;
  }
  /* 右: タブ */
  .tabs {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: auto;
    padding: 8px;
    background: var(--bg-raised);
  }
  .tab-bar {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }
  .tab-bar button {
    border: 0;
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    font-size: 12px;
    padding: 2px 6px;
    cursor: pointer;
    border-radius: 2px;
    text-decoration: underline;
    text-decoration-color: var(--border);
  }
  .tab-bar button:hover {
    background: var(--bg-selection);
  }
  .tab-bar button.active {
    color: var(--accent);
    background: var(--bg-selection);
  }
  .hint {
    margin-top: 8px;
    color: var(--fg-faint);
    font-size: 11px;
  }
  .count {
    color: var(--fg-muted);
    font-size: 11px;
  }
  /* モードタイムライン */
  .timeline {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .chapters {
    display: flex;
    gap: 2px;
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 9px;
    color: var(--fg-faint);
  }
  .band {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .band-label {
    flex: 0 0 auto;
    width: 84px;
    font-size: 11px;
    color: var(--fg-muted);
    text-align: right;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .band-track {
    display: flex;
    flex: 1 1 auto;
    gap: 1px;
  }
  .band-cell {
    flex: 1 1 0;
    height: 8px;
    background: var(--bg-inset);
    border-radius: 1px;
  }
  .band-cell.on {
    background: var(--modifier-mode);
  }
  /* 役割語 */
  .speech {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .scan {
    border: 0;
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    font-size: 12px;
    padding: 2px 4px;
    cursor: pointer;
    text-decoration: underline;
    text-decoration-color: var(--border);
  }
  .scan:hover {
    background: var(--bg-selection);
    color: var(--fg-primary);
  }
  .unmatched {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .line {
    display: flex;
    gap: 6px;
    width: 100%;
    border: 0;
    background: transparent;
    color: var(--fg-primary);
    text-align: left;
    padding: 2px 4px;
    cursor: pointer;
    border-radius: 2px;
    text-decoration: underline;
    text-decoration-color: var(--border);
    font-size: 12px;
  }
  .line:hover {
    background: var(--bg-selection);
  }
  .line .speaker {
    color: var(--speaker);
    flex: 0 0 auto;
  }
  .line .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* 呼称マトリクス */
  .matrix {
    border-collapse: collapse;
    font-size: 11px;
  }
  .matrix th,
  .matrix td {
    border: 1px solid var(--border);
    padding: 2px 4px;
    text-align: center;
    white-space: nowrap;
  }
  .matrix th {
    color: var(--fg-faint);
    font-weight: 400;
  }
  .matrix .diagonal {
    background: var(--bg-inset);
  }
  .matrix .diag {
    color: var(--fg-faint);
  }
  .matrix .cell {
    color: var(--fg-muted);
  }
</style>
