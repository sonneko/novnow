<script lang="ts">
  // プロジェクトの入口の面（§9.7 の `Ctrl+O`、§13.5 の Zip の入出力）。
  //
  // 流れは第 1 の画面と同じ 2 段（仕様 §9.7 の `Ctrl+O` の統一）:
  //
  // 1. 現在の変更を保存してから、この面を開く（保存に失敗している間は開かない。`App.svelte`）。
  // 2. この面で「最近開いた一覧から選ぶ」「新規作成」「Zip から読み込む / 書き出す」。
  //
  // 環境で押せるものが違う（`picker.ts` の `capabilities`）。押せないものは**理由を書いて**置く
  // （§21.1.2 MUST: 黙って効かないボタンを置かない・未提供を隠さない）。ブラウザでは
  // `project.new` と `project.recent_list` が機能核で `no_fs` を返すため、その 2 つは
  // `workspace.unavailable` で「この版ではまだ使えません」と示す（押しても何も起きない
  // ボタンを並べない）。
  //
  // 規則（並び・重複の落とし方・引数の組み立て）は `picker.ts` が持つ。ここは描画と入力だけ。
  // 文言は生成物 `i18n/catalog.ts` に**実在するキーだけ**を使う（§12.3。第 2 用の文言を足さない）。

  import type { Texts } from '../i18n';
  import {
    capabilities,
    canForget,
    exportArgs,
    importArgs,
    missingFiles,
    newProjectPlan,
    projectRows,
    validProjectName,
    type ProjectRow,
    type Runtime,
    type ZipExport,
  } from '../project/picker';

  /** 文言。 */
  export let t: Texts['t'];
  /** 動いている環境（`native` = Tauri、`browser` = wasm）。 */
  export let runtime: Runtime;
  /** いま開いているプロジェクト。 */
  export let current = '';
  /** 閉じる。 */
  export let onclose: (() => void) | null = null;
  /** 一覧を取り直す（`project.recent_list`。ブラウザでは呼ばない）。 */
  export let onlist: (() => Promise<{ path: string; opened_at: string }[]>) | null = null;
  /** そのプロジェクトを開く（`bridge.open` の後に画面を作り直す）。 */
  export let onopen: ((path: string) => Promise<void>) | null = null;
  /** 新しく作る（`project.new`）。 */
  export let oncreate: ((path: string, title: string) => Promise<void>) | null = null;
  /** 履歴から消す（`project.recent_remove`）。 */
  export let onforget: ((path: string) => Promise<void>) | null = null;
  /** Zip を取り込む（`project.import_zip`）。戻り値は `written`。 */
  export let onimport: ((args: Record<string, unknown>) => Promise<string[]>) | null = null;
  /** Zip を書き出す（`project.export_zip`）。 */
  export let onexport: ((args: Record<string, unknown>) => Promise<ZipExport | null>) | null = null;

  const caps = capabilities(runtime);

  /** 一覧（`project.recent_list` の結果。ブラウザでは空）。 */
  let rows: ProjectRow[] = projectRows(current, []);
  /** 新しいプロジェクトの名前（`project.new` の `title`）。 */
  let name = '';
  /** 作る先（ネイティブで利用者が選ぶフォルダ）。 */
  let parent = '';
  /** 処理中（二重に押させない）。 */
  let working = false;
  /** 画面に出す結果（成功・失敗の 1 行）。 */
  let note = '';
  /** Zip の書き出し先（ネイティブ。空なら中身だけ受け取る）。 */
  let out = '';

  /** 開いたときに一覧を取る（ブラウザは取れないので、いま開いている 1 件だけ）。 */
  async function load(): Promise<void> {
    if (!caps.recentList || onlist === null) {
      rows = projectRows(current, []);
      return;
    }
    working = true;
    try {
      rows = projectRows(current, await onlist());
    } catch (error) {
      note = t('project.pick.failed', [message(error)]);
    } finally {
      working = false;
    }
  }

  function message(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  /** 作る先と名前が揃っているか（揃うまで押させない）。 */
  $: plan = newProjectPlan(parent, name, runtime);
  $: canCreate = typeof plan !== 'string' && validProjectName(name);

  /** 外側を押したら閉じる（§9.7.5）。 */
  function outside(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      onclose?.();
    }
  }

  async function open(path: string): Promise<void> {
    if (working || onopen === null || path === '') {
      return;
    }
    working = true;
    try {
      await onopen(path);
      onclose?.();
    } catch (error) {
      note = t('project.pick.failed', [message(error)]);
    } finally {
      working = false;
    }
  }

  async function create(): Promise<void> {
    if (typeof plan === 'string' || oncreate === null) {
      return;
    }
    working = true;
    try {
      await oncreate(plan.path, plan.title);
      name = '';
      onclose?.();
    } catch (error) {
      note = t('project.pick.failed', [message(error)]);
    } finally {
      working = false;
    }
  }

  async function forget(row: ProjectRow): Promise<void> {
    if (working || onforget === null) {
      return;
    }
    working = true;
    try {
      await onforget(row.path);
      await load();
    } catch (error) {
      note = t('project.pick.failed', [message(error)]);
    } finally {
      working = false;
    }
  }

  /** Zip を選んで取り込む（ブラウザも同じ流れ。`bytes` で渡す）。 */
  async function importZip(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file || onimport === null) {
      return;
    }
    working = true;
    try {
      const bytes = await base64(file);
      const written = await onimport(importArgs(runtime, { path: file.name, bytes }));
      const missing = missingFiles(written);
      note = missing.length === 0
        ? t('project.pick.imported', [String(written.length)])
        : t('project.pick.incomplete', [missing.join(', ')]);
    } catch (error) {
      note = t('project.pick.failed', [message(error)]);
    } finally {
      input.value = '';
      working = false;
    }
  }

  /** Zip を書き出す（ブラウザは `bytes` からダウンロードする）。 */
  async function exportZip(): Promise<void> {
    if (working || onexport === null) {
      return;
    }
    working = true;
    try {
      const found = await onexport(exportArgs(runtime, out));
      // `out` を渡した（ネイティブ）なら Host が書いている。渡していなければ画面が保存する。
      if (found && found.out) {
        note = t('status.zip_exported', [found.out]);
      } else if (found) {
        download(found);
        note = t('status.zip_exported', [found.name]);
      }
    } catch (error) {
      note = t('status.zip_failed', [message(error)]);
    } finally {
      working = false;
    }
  }

  /** ファイルを base64 にする（`project.import_zip` の `bytes`）。 */
  function base64(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onerror = () => reject(reader.error ?? new Error('read failed'));
      reader.onload = () => {
        const text = String(reader.result ?? '');
        const comma = text.indexOf(',');
        resolve(comma >= 0 ? text.slice(comma + 1) : '');
      };
      reader.readAsDataURL(file);
    });
  }

  /** 受け取った Zip をダウンロードさせる（§21.2.3: 利用者が明示したときだけ書き出す）。 */
  function download(found: ZipExport): void {
    const binary = atob(found.bytes);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) {
      bytes[index] = binary.charCodeAt(index);
    }
    const url = URL.createObjectURL(new Blob([bytes], { type: 'application/zip' }));
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = found.name || 'novnow.zip';
    anchor.click();
    URL.revokeObjectURL(url);
  }

  // 開いたときに 1 度だけ取る。
  void load();
</script>

<div class="overlay" role="presentation" on:click={outside}>
  <div class="sheet" role="dialog" aria-modal="true" aria-label={t('project.pick.title')} tabindex="-1">
    <h2>{t('project.pick.title')}</h2>

    {#if caps.recentList}
      {#if rows.length === 0}
        <p class="empty" role="status">{t('project.pick.empty')}</p>
      {:else}
        <ul class="rows" aria-label={t('project.pick.title')}>
          {#each rows as row (row.path)}
            <li>
              <button type="button" class="open" disabled={working} on:click={() => void open(row.path)}>
                {row.path}
              </button>
              {#if canForget(row)}
                <button
                  type="button"
                  class="forget"
                  title={t('project.pick.remove_history')}
                  aria-label={t('project.pick.remove_history')}
                  disabled={working}
                  on:click={() => void forget(row)}>{t('project.pick.remove_history')}</button
                >
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    {:else}
      <!-- 未提供を理由付きで示す（§21.1.2 MUST）。一覧が空なのではなく、この環境では取れない。 -->
      <p class="unavailable">{t('workspace.unavailable', [t('api.op.project.recent_list')])}</p>
    {/if}

    <section class="block">
      {#if caps.newProject}
        <h3>{t('project.pick.new')}</h3>
        <label>{t('project.pick.new_name')}
          <input type="text" bind:value={name} />
        </label>
        <label>{t('project.pick.open')}
          <input type="text" bind:value={parent} placeholder="/home/you/projects" />
        </label>
        <button type="button" disabled={working || !canCreate} on:click={() => void create()}>
          {t('project.pick.new')}
        </button>
      {:else}
        <h3>{t('project.pick.new')}</h3>
        <!-- ブラウザはファイルに書けない（機能核が `no_fs` を返す）。押せない理由を書く。 -->
        <p class="unavailable">{t('workspace.unavailable', [t('api.op.project.new')])}</p>
      {/if}
    </section>

    <section class="block">
      <h3>{t('command.export_zip')}</h3>
      <label class="file">{t('project.pick.import')}
        <input type="file" accept=".zip,application/zip" disabled={working} on:change={importZip} />
      </label>
      {#if caps.exportToPath}
        <label>{t('command.export_zip')}
          <input type="text" bind:value={out} placeholder="novnow.zip" />
        </label>
      {/if}
      <button type="button" disabled={working} on:click={() => void exportZip()}>
        {t('command.export_zip')}
      </button>
    </section>

    {#if note}<p class="note" role="status">{note}</p>{/if}
    <p class="close-hint">{t('modal.close_hint')}</p>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgb(0 0 0 / 45%);
  }
  .sheet {
    background: var(--bg-raised);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 16px;
    width: min(560px, 90vw);
    max-height: 80vh;
    overflow: auto;
  }
  h2 {
    margin: 0 0 12px;
    font-size: 1rem;
  }
  h3 {
    margin: 16px 0 6px;
    font-size: 0.9rem;
    color: var(--text-muted);
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .rows li {
    display: flex;
    align-items: center;
    gap: 8px;
    border-bottom: 1px solid var(--border);
  }
  .open {
    flex: 1;
    text-align: left;
    background: none;
    border: 0;
    color: inherit;
    padding: 6px 4px;
    cursor: pointer;
    font: inherit;
  }
  .open:hover:not(:disabled) {
    background: var(--bg-hover);
  }
  .forget {
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-muted);
    cursor: pointer;
    font: inherit;
    font-size: 0.75rem;
    padding: 2px 6px;
    min-width: 24px;
    min-height: 24px;
  }
  .block label {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 4px 0;
    font-size: 0.9rem;
  }
  .block input[type='text'] {
    flex: 1;
    background: var(--bg-base);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px 6px;
    font: inherit;
  }
  .block button {
    margin-top: 6px;
    background: var(--bg-base);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px 10px;
    min-height: 24px;
    cursor: pointer;
    font: inherit;
  }
  .block button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .unavailable,
  .empty {
    color: var(--text-muted);
    font-size: 0.85rem;
    margin: 6px 0;
  }
  .note {
    margin-top: 12px;
    font-size: 0.85rem;
  }
  .close-hint {
    margin: 12px 0 0;
    color: var(--text-muted);
    font-size: 0.75rem;
  }
</style>
