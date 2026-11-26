<script lang="ts">
  // §9.7.4 の入力面。検索そのものは `search.file` / `search.project` に任せる。
  import { onMount } from 'svelte';
  import type { Texts } from '../i18n';
  import type { Screen, Session } from '../store/session';

  export let t: Texts['t'];
  export let session: Session;
  export let screen: Screen;
  export let scope: 'file' | 'project' = 'project';
  export let replaceMode = false;
  export let regexMode = false;
  export let onclose: (() => void) | null = null;
  export let onsave: (() => Promise<void>) | null = null;
  export let onapplied: (() => void) | null = null;

  type Hit = { path: string; line: number; text: string; before: string; matched: string; after: string };
  let query = '';
  let replacement = '';
  let regex = regexMode;
  let caseSensitive = true;
  let busy = false;
  let error = '';
  let hits: Hit[] = [];
  let preview = '';
  let previewCount = 0;
  let previewSignature = '';
  let previewReady = false;
  let timedOut = false;
  let input: HTMLInputElement;

  onMount(() => input?.focus());

  function contextAt(text: string, start: number, end: number): Omit<Hit, 'path' | 'line' | 'text'> {
    return {
      before: [...text.slice(0, start)].slice(-20).join(''),
      matched: text.slice(start, end),
      after: [...text.slice(end)].slice(0, 20).join(''),
    };
  }

  function projectSignature(rows: unknown): string {
    if (!Array.isArray(rows)) return '';
    return JSON.stringify(rows.map((row) => {
      const hit = row as Record<string, unknown>;
      return [hit.path, hit.line, hit.before, hit.matched, hit.after];
    }));
  }

  const REGEX_META = new Set(['.', '*', '+', '?', '^', '$', '{', '}', '(', ')', '|', '[', ']', '\\']);
  function escapeRegex(value: string): string {
    return [...value].map((character) => REGEX_META.has(character) ? `\\${character}` : character).join('');
  }

  function coreQuery(): { query: string; regex: boolean } {
    const escaped = escapeRegex(query);
    const useRegex = regex || !caseSensitive;
    const source = regex ? query : escaped;
    return { query: !caseSensitive ? `(?i)${source}` : source, regex: useRegex };
  }

  async function search(): Promise<void> {
    if (!query || busy) { hits = []; return; }
    busy = true;
    error = '';
    previewReady = false;
    timedOut = false;
    try {
      const args = coreQuery();
      if (scope === 'file') {
        const found = await session.run('search.file', args);
        if (!found.ok) { error = found.message; hits = []; return; }
        // 機能核の range は UTF-8 byte 位置、DOM の文字列は UTF-16。
        // 数値を直接 slice せず、正規操作の成功を確認したうえで同じ条件を JS 文字位置へ写す。
        const source = regex ? query : escapeRegex(query);
        const matcher = new RegExp(source, caseSensitive ? 'g' : 'gi');
        hits = [];
        for (const match of screen.text.matchAll(matcher)) {
          const start = match.index ?? 0;
          const end = start + match[0].length;
          const line = screen.text.slice(0, start).split('\n').length;
          hits.push({ path: screen.path, line, text: screen.text, ...contextAt(screen.text, start, end) });
          if (hits.length >= 200) break;
        }
      } else {
        const found = await session.projectSearch({ query, regex, case_sensitive: caseSensitive });
        if (!found.ok) { error = found.message; hits = []; return; }
        timedOut = found.result.timed_out === true;
        const rows = Array.isArray(found.result.hits) ? found.result.hits as Record<string, unknown>[] : [];
        hits = rows.flatMap((row) => typeof row.path === 'string' ? [{
          path: row.path,
          line: typeof row.line === 'number' ? row.line : 1,
          text: typeof row.text === 'string' ? row.text : '',
          before: typeof row.before === 'string' ? row.before : '',
          matched: typeof row.matched === 'string' ? row.matched : '',
          after: typeof row.after === 'string' ? row.after : '',
        }] : []);
      }
    } finally { busy = false; }
  }

  function convertReplacement(value: string): string {
    return value.replace(/\\n/g, '\n');
  }

  function makePreview(): void {
    error = '';
    if (scope === 'project') {
      previewCount = hits.length;
      previewSignature = JSON.stringify(hits.map((hit) => [hit.path, hit.line, hit.before, hit.matched, hit.after]));
      const byPath = new Map<string, number>();
      for (const hit of hits) byPath.set(hit.path, (byPath.get(hit.path) ?? 0) + 1);
      preview = [...byPath].map(([path, count]) => `${path}: ${count}`).join('\\n');
      previewReady = true;
      return;
    }
    try {
      const source = screen.text;
      const flags = caseSensitive ? 'g' : 'gi';
      const patternSource = regex ? query : escapeRegex(query);
      const pattern = new RegExp(patternSource, flags);
      let count = 0;
      preview = source.replace(pattern, (...args: unknown[]) => {
        count += 1;
        if (!regex) return convertReplacement(replacement);
        const values = args;
        const match = String(values[0] ?? '');
        const captures = values.slice(1, -2) as (string | undefined)[];
        const named = typeof values.at(-1) === 'object' && values.at(-1) !== null
          ? values.at(-1) as Record<string, string | undefined>
          : {};
        return convertReplacement(replacement).replace(/\$\{([\w]+)\}|\$(\d+)/g, (_all, name: string | undefined, number: string | undefined) =>
          name ? named[name] ?? '' : captures[Number(number) - 1] ?? '',
        );
      });
      previewCount = count;
      previewReady = true;
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
  }

  async function apply(): Promise<void> {
    if (!previewReady || previewCount === 0 || busy) return;
    busy = true;
    try {
      if (scope === 'project') {
        const result = await session.projectSearch({ query, regex, case_sensitive: caseSensitive, replacement });
        if (!result.ok) { error = result.message; return; }
        const rows = Array.isArray(result.result.hits) ? result.result.hits as Record<string, unknown>[] : [];
        const currentSignature = projectSignature(rows);
        if (result.result.count !== previewCount || currentSignature !== previewSignature) {
          error = '検索結果が変わりました。もう一度プレビューしてください。';
          return;
        }
        timedOut = result.result.timed_out === true;
        const saved = await session.run('save.now');
        if (!saved.ok) { error = saved.message; return; }
        await onsave?.();
        onapplied?.();
      } else {
        const result = await session.run('search.file', { ...coreQuery(), replace: replacement });
        if (!result.ok) { error = result.message; return; }
        const saved = await session.run('save.now');
        if (!saved.ok) { error = saved.message; return; }
        await onsave?.();
        onapplied?.();
      }
      onclose?.();
    } finally { busy = false; }
  }

  async function go(hit: Hit): Promise<void> {
    if (hit.path !== screen.path) {
      if (!hit.path.startsWith('contents/')) { error = 'この結果は本文の節へ移動できません。'; return; }
      const listed = await session.run('section.list');
      if (!listed.ok) { error = listed.message; return; }
      const rows = Array.isArray(listed.result.sections) ? listed.result.sections as Record<string, unknown>[] : [];
      const row = rows.find((item) => item.path === hit.path && Array.isArray(item.lines) && Number(item.lines[0]) <= hit.line && Number(item.lines[1]) >= hit.line);
      if (typeof row?.id !== 'string') { error = '一致箇所を含む節が見つかりません。'; return; }
      const moved = await session.run('nav.section', { scene: row.id });
      if (!moved.ok) { error = moved.message; return; }
    }
    onclose?.();
  }

  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') { event.preventDefault(); onclose?.(); }
    if (event.altKey && event.key.toLowerCase() === 'r') { event.preventDefault(); regex = !regex; void search(); }
    if (event.altKey && event.key.toLowerCase() === 'c') { event.preventDefault(); caseSensitive = !caseSensitive; void search(); }
    if (event.key === 'Enter' && event.target === input) { event.preventDefault(); void search(); }
  }
</script>

<svelte:window on:keydown={keydown} />
<div class="backdrop" role="presentation" on:click={(event) => event.target === event.currentTarget && onclose?.()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={scope === 'project' ? t('stage.search') : '検索'} tabindex="-1">
    <header><h2>{scope === 'project' ? t('stage.search') : '検索 / 置換'}</h2><button type="button" on:click={() => onclose?.()}>×</button></header>
    <label>検索 <input bind:this={input} bind:value={query} on:input={() => void search()} placeholder={t('search.placeholder')} /></label>
    {#if replaceMode}<label>置換 <input bind:value={replacement} on:input={() => (previewReady = false)} /></label>{/if}
    <div class="options">
      <button type="button" aria-pressed={regex} on:click={() => { regex = !regex; void search(); }}>.* 正規表現 (Alt+R)</button>
      <label><input type="checkbox" bind:checked={caseSensitive} on:change={() => void search()} /> 大小文字を区別 (Alt+C)</label>
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if replaceMode}
      <div class="replace-actions"><button type="button" disabled={!query || busy} on:click={makePreview}>プレビュー</button>
        {#if previewReady}<p>{previewCount} 箇所{scope === 'project' ? ` · ${new Set(hits.map((hit) => hit.path)).size} ファイル` : ' · 変更前 / 変更後'}</p>
          {#if scope === 'file'}<pre>{screen.text}</pre><pre>{preview}</pre>{:else}<pre>{preview}</pre>{/if}
          <button type="button" disabled={!previewCount || busy} on:click={() => void apply()}>確認して置換</button>{/if}
      </div>
    {/if}
    <p aria-live="polite">{busy ? '検索中…' : `${hits.length} 件`}{timedOut ? ' · 時間上限のため部分結果' : ''}</p>
    <ul>{#each hits as hit, index (`${hit.path}:${hit.line}:${index}`)}
      <li><button type="button" on:click={() => void go(hit)}><code>{hit.path}{hit.line ? `:${hit.line}` : ''}</code><span>{hit.before}<mark>{hit.matched}</mark>{hit.after}</span></button></li>
    {/each}</ul>
  </div>
</div>

<style>
  .backdrop{position:fixed;inset:0;z-index:1000;background:#0008;display:grid;place-items:start center;padding:8vh 1rem}
  .dialog{width:min(760px,96vw);max-height:84vh;overflow:auto;background:var(--surface,#242424);color:var(--text,#eee);border:1px solid var(--border,#555);border-radius:10px;padding:1rem;box-shadow:0 12px 48px #0008}
  header,.options,.replace-actions{display:flex;align-items:center;gap:.75rem;justify-content:space-between}h2{margin:.2rem 0 1rem}label{display:flex;gap:.5rem;align-items:center;margin:.6rem 0}label input:not([type=checkbox]){flex:1;min-width:0;padding:.55rem}button{cursor:pointer}ul{list-style:none;padding:0}li button{width:100%;display:flex;gap:.75rem;text-align:left;padding:.5rem;border:0;border-top:1px solid var(--border,#555);background:transparent;color:inherit}li button:hover{background:#ffffff12}code{min-width:180px;color:#9bc}mark{background:#e8c547;color:#111}.error{color:#ff8d8d}pre{white-space:pre-wrap;max-height:12rem;overflow:auto;background:#0002;padding:.5rem}
</style>
