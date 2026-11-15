// 点検（`review`。`REF:docs/specifications/ui.md` §10.9）の規則。
//
// 材料は正規操作の戻り値だけ（`review.*` / `diagnostics.run` / `structure.tree` / `section.list` /
// `document.get` / `project.stats` / `search.project` / `agent.proposal_*`。§11.11.5）。
// ここではファイル形式を解釈しない（§21.1.3）。行うのは:
//
// 1. 9 タブの並び・切替（`Ctrl+Shift+← / →`。§10.9.1）と、タブごとの提供状態。
// 2. 診断一覧の形の確認・フィルタ・ソート（§10.9.2）と「作家の言葉でのどこ」（`place.*`）。
// 3. 進捗ツリー（章 → 話 → 場面 → 節）の組み立て（文字数は本文の表示数え。§9.6.1 と同じ）。
// 4. 分量の T スコア（§10.3.4。`T = 50 + 10 × (x − 平均) / 標準偏差`、母集団標準偏差）と
//    ヒストグラム（§10.9.5(c)）。
// 5. 文体観察の指標グループ（§10.9.4。診断コードの集計だけ。スコアも「改善」の語も出さない）。
// 6. 挿絵メモ（`! ` 行と `illustration:`。`search.project` の行一致を SceneId 順に並べる）。
//
// 未提供（この版で材料が無いもの。§21.1.2 MUST で画面は「未提供」と出す）:
// - 時系列（`when:` の一覧を作る操作が無い。`when:` が 1 件も無いときはタブ自体を出さない
//   規則（§10.9.1）は、`search.project('when:')` の件数で守る）
// - 話者別の「登場節・視点節」（節ごとの `pov:` / `cast:` の集計が操作に無い）
// - 字数分布の「目標比」（話プロットの `length:` は操作が構造で返さない）
// - 文体観察の「1 文の長さ分布」のヒストグラム（`metrics::sentence_lengths` が核にあり、
//   操作が返さない。W089 の行は出す）
// - 執筆統計（`.novnow/stats.json` の日別・曜日別・時間帯別・連続日数を読む操作が無い）
// - 五感の宣言集計（`senses:` の集計が操作に無い。W083 の区間は診断タブに出る）
// - 診断の有効設定（`project.toml` の `[features.diagnostics]` を書く操作が無い）

/** 9 タブ（§10.9.1 の表の順。ラベルは `stage.*`）。 */
export const REVIEW_TABS = [
  'base',
  'timeline',
  'speakers',
  'histogram',
  'observations',
  'stats',
  'senses',
  'illustrations',
  'proposals',
] as const;

export type ReviewTab = (typeof REVIEW_TABS)[number];

/** タブの見出しの文言キー。 */
export const TAB_LABEL: Record<ReviewTab, string> = {
  base: 'stage.base',
  timeline: 'stage.timeline',
  speakers: 'stage.speakers',
  histogram: 'stage.histogram',
  observations: 'stage.observations',
  stats: 'stage.stats',
  senses: 'stage.senses',
  illustrations: 'stage.illustrations',
  proposals: 'stage.proposals',
};

/**
 * タブの提供状態。
 * `ready` は操作だけで出せる。`partial` は一部だけ（残りは「未提供」を添える）。`unavailable`
 * は材料が無く、中身を出せない（§21.1.2。黙って別の結果に置き換えない）。
 */
export function tabState(tab: ReviewTab): 'ready' | 'partial' | 'unavailable' {
  switch (tab) {
    case 'base':
    case 'illustrations':
    case 'proposals':
      return 'ready';
    case 'speakers':
    case 'histogram':
    case 'observations':
      return 'partial';
    default:
      return 'unavailable';
  }
}

/** 次のタブ（`Ctrl+Shift+→`）と前のタブ（`Ctrl+Shift+←`）。端では循環（§10.9.1）。 */
export function moveTab(tab: ReviewTab, step: 1 | -1, visible: readonly ReviewTab[]): ReviewTab {
  const list = visible.length > 0 ? visible : REVIEW_TABS;
  const at = list.indexOf(tab);
  const from = at < 0 ? 0 : at;
  const next = (from + step + list.length) % list.length;
  return list[next] ?? 'base';
}

/** 表示するタブ（`when:` が 1 件も無ければ時系列を出さない。§10.9.1 MUST）。 */
export function visibleTabs(hasTimeline: boolean): ReviewTab[] {
  return REVIEW_TABS.filter((tab) => tab !== 'timeline' || hasTimeline);
}

/** 基本タブの内部切替（診断 / 進捗 / スレッド。`review.tab_*`）。 */
export const BASE_VIEWS = ['diagnostics', 'progress', 'threads'] as const;

export type BaseView = (typeof BASE_VIEWS)[number];

/** 内部切替の見出しの文言キー。 */
export const BASE_VIEW_LABEL: Record<BaseView, string> = {
  diagnostics: 'review.tab_diagnostics',
  progress: 'review.tab_progress',
  threads: 'review.tab_threads',
};

/** 内部切替の選択（ボタンの並び。`BASE_VIEWS` の順）。キーボードはフォーカスで到達する（§9.9）。 */
export function isBaseView(value: unknown): value is BaseView {
  return typeof value === 'string' && (BASE_VIEWS as readonly string[]).includes(value);
}

// ---------------------------------------------------------------- 診断一覧（§10.9.2）

/** 診断 1 行（`review.list` の `diagnostics`）。 */
export interface DiagRow {
  /** `review.list` の並びの位置（`review.goto` に渡す。§10.9.2 の `index`）。 */
  index: number;
  /** `W087` など。 */
  code: string;
  /** `error` / `warning` / `info`（`severity_name`）。 */
  severity: string;
  path: string;
  /** 1 始まり。場所が無ければ 0。 */
  line: number;
  column: number;
  /** 文言（利用者の言語で返る）。 */
  message: string;
  /** 文書内のバイト位置（無ければ null）。 */
  byte: number | null;
}

/** `review.list` の行の形（`index` 以外）。 */
export type RawDiag = Omit<DiagRow, 'index'>;

/** 値が診断の形をしているか。 */
export function isDiag(value: unknown): value is RawDiag {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const found = value as Record<string, unknown>;
  return typeof found.code === 'string' && typeof found.path === 'string' && typeof found.message === 'string';
}

/** `review.list` / `diagnostics.run` の行を読む（形が違う行は捨てる。`index` は操作の並びのまま）。 */
export function rowsOf(result: unknown): DiagRow[] {
  const list = (result as { diagnostics?: unknown } | null)?.diagnostics;
  if (!Array.isArray(list)) {
    return [];
  }
  const out: DiagRow[] = [];
  for (let at = 0; at < list.length; at += 1) {
    const raw = list[at];
    if (!isDiag(raw)) {
      continue;
    }
    const found = raw as unknown as Record<string, unknown>;
    out.push({
      index: at,
      code: raw.code,
      severity: typeof found.severity === 'string' ? found.severity : 'info',
      path: raw.path,
      line: typeof found.line === 'number' ? found.line : 0,
      column: typeof found.column === 'number' ? found.column : 0,
      message: raw.message,
      byte: typeof found.byte === 'number' ? found.byte : null,
    });
  }
  return out;
}

/** 重大度の並び（エラー → 警告 → 情報。`review.sort_severity`）。 */
export function severityRank(severity: string): number {
  switch (severity) {
    case 'error':
      return 0;
    case 'warning':
      return 1;
    default:
      return 2;
  }
}

/** ソート（`review.sort_position` / `sort_code` / `sort_severity`。同順は場所で決着）。 */
export function sortRows(rows: readonly DiagRow[], mode: 'position' | 'code' | 'severity'): DiagRow[] {
  const out = [...rows];
  out.sort((a, b) => {
    if (mode === 'code') {
      const byCode = a.code.localeCompare(b.code);
      if (byCode !== 0) {
        return byCode;
      }
    } else if (mode === 'severity') {
      const bySeverity = severityRank(a.severity) - severityRank(b.severity);
      if (bySeverity !== 0) {
        return bySeverity;
      }
    }
    return a.path.localeCompare(b.path) || a.line - b.line || a.column - b.column || a.code.localeCompare(b.code);
  });
  return out;
}

/** コードの絞り込み（`review.settings_filter_hint`。前方一致。例 `W08`）。 */
export function filterRows(rows: readonly DiagRow[], codePrefix: string): DiagRow[] {
  const found = codePrefix.trim().toUpperCase();
  if (!found) {
    return [...rows];
  }
  return rows.filter((row) => row.code.toUpperCase().startsWith(found));
}

/** 重大度の件数（`review.errors` / `warnings` / `infos` の文言に渡す）。 */
export function severityCounts(rows: readonly DiagRow[]): { error: number; warning: number; info: number } {
  let error = 0;
  let warning = 0;
  let info = 0;
  for (const row of rows) {
    if (row.severity === 'error') {
      error += 1;
    } else if (row.severity === 'warning') {
      warning += 1;
    } else {
      info += 1;
    }
  }
  return { error, warning, info };
}

// ---------------------------------------------------------------- 「作家の言葉でのどこ」（§10.9.2）

/** 場所の文言（`place.*` のキーと引数）。 */
export interface PlaceText {
  key: string;
  args: readonly string[];
}

/** 場所の索引を作る材料。 */
export interface PlaceIndex {
  /** 節（`section.list`）。行の範囲から節 ID を引く。 */
  sections: readonly { id: string; path: string; lines: [number, number] }[];
  /** A 形式のブロック（`plot.get`）。行からブロック ID を引く。 */
  blocks: readonly { id: string; path: string; lines: [number, number] }[];
  /** 資料（`materials.list`）。パスから資料 ID・題を引く。 */
  materials: readonly { id: string; title: string; path: string }[];
}

/** パスの種類から「どこ」を作る（決まらなければ null。`REF:src/ui/i18n/mod.rs` の `place` と同じ表）。 */
export function placeOf(path: string, line: number, index: PlaceIndex): PlaceText | null {
  if (path.startsWith('contents/')) {
    const found = index.sections.find(
      (section) => section.path === path && line > 0 && line >= section.lines[0] + 1 && line <= section.lines[1],
    );
    return found ? { key: 'place.contents', args: [found.id] } : null;
  }
  if (path === 'plots/all.cnov') {
    const block = blockAt(index, path, line);
    return block ? { key: 'place.whole_plot', args: [block] } : { key: 'place.whole_plot_head', args: [] };
  }
  const chapter = /^plots\/(\d{1,3})\.cnov$/u.exec(path);
  if (chapter) {
    const block = blockAt(index, path, line);
    return block ? { key: 'place.chapter_plot', args: [block] } : { key: 'place.chapter_plot_head', args: [] };
  }
  const episode = /^plots\/(\d{1,3}-\d{1,3})\.cnov$/u.exec(path);
  if (episode) {
    return { key: 'place.episode_plot', args: [episode[1] ?? ''] };
  }
  if (path === 'project.toml') {
    return { key: 'place.settings', args: [] };
  }
  const prompt = /^agents\/prompts\/([^/]+)\.md$/u.exec(path);
  if (prompt) {
    return { key: 'place.prompt', args: [prompt[1] ?? ''] };
  }
  const material = index.materials.find((entry) => entry.path === path);
  if (material) {
    return material.title
      ? { key: 'place.material', args: [material.id, material.title] }
      : { key: 'place.material_id', args: [material.id] };
  }
  return null;
}

/** 行を含むブロックの ID（A 形式。無ければ null）。 */
function blockAt(index: PlaceIndex, path: string, line: number): string | null {
  const found = index.blocks.find(
    (block) => block.path === path && line > 0 && line >= block.lines[0] + 1 && line <= block.lines[1],
  );
  return found?.id ?? null;
}

/** 文言の組み立て（`Session['t']` と同じ形）。 */
type Tf = (key: string, args?: readonly string[]) => string;

/** 場所の表示（`place.contents` = 「本文 @…」。決まらなければ生パス:行:列）。 */
export function placeText(
  t: Tf,
  place: PlaceText | null,
  raw: { path: string; line: number; column: number },
): string {
  if (place === null) {
    return raw.path + (raw.line > 0 ? ':' + raw.line + ':' + raw.column : '');
  }
  return t(place.key, place.args);
}

// ---------------------------------------------------------------- 進捗ツリー（§10.9.1 基本）

/** ツリーの 1 行。 */
export interface TreeRow {
  /** `01` / `01-04` / `01-04-02` / `01-04-02-03`。 */
  id: string;
  /** 階層（0 = 章 … 3 = 節）。 */
  depth: number;
  /** 題（章・話はプロットの題、場面・節は見出し）。 */
  title: string;
  /** 本文の文字数（空白を除く。上の階層は下の合計）。 */
  chars: number;
  /** 状態（A 形式の `state:` がある章・話だけ。無ければ null＝未提供）。 */
  state: string | null;
  /** 子が 1 つでも本文を持つか。 */
  prose: boolean;
}

/**
 * `structure.tree` の 4 回分（chapter / episode / scene / section）と文字数から、
 * 4 段階のツリー行を作る。並びは ID の順（ストック `00` も含めて ID 順。§10.3.1 と同じ精神）。
 */
export function treeRows(
  titles: ReadonlyMap<string, string>,
  chars: ReadonlyMap<string, number>,
  states: ReadonlyMap<string, string>,
): TreeRow[] {
  const ids = [...new Set([...titles.keys(), ...chars.keys()])].sort(compareId);
  const out: TreeRow[] = [];
  const seen = new Set<string>();
  for (const id of ids) {
    for (const prefix of ancestorsOf(id)) {
      if (seen.has(prefix)) {
        continue;
      }
      seen.add(prefix);
      out.push({
        id: prefix,
        depth: prefix.split('-').length - 1,
        title: titles.get(prefix) ?? '',
        chars: 0,
        state: states.get(prefix) ?? null,
        prose: false,
      });
    }
  }
  out.sort((a, b) => compareId(a.id, b.id) || a.depth - b.depth);
  // 文字数は下の合計（節は自分の値）
  for (const row of out) {
    const own = chars.get(row.id);
    if (row.depth === 3 && own !== undefined) {
      row.chars = own;
    }
  }
  for (let at = out.length - 1; at >= 0; at -= 1) {
    const row = out[at];
    if (!row) {
      continue;
    }
    if (row.depth < 3) {
      row.chars = out
        .filter((child) => child.id.startsWith(`${row.id}-`) && child.depth === row.depth + 1)
        .reduce((sum, child) => sum + child.chars, 0);
    }
    row.prose = (chars.get(row.id) ?? 0) > 0 || out.some((child) => child.id.startsWith(`${row.id}-`) && child.prose);
  }
  return out;
}

/** 祖先の ID（自分を含む。`01-04-02-03` → `01` `01-04` `01-04-02` `01-04-02-03`）。 */
export function ancestorsOf(id: string): string[] {
  const parts = id.split('-');
  const out: string[] = [];
  for (let at = 1; at <= parts.length; at += 1) {
    out.push(parts.slice(0, at).join('-'));
  }
  return out;
}

/** ID の並び（数値の順。`00` のストックも自然な位置に来る）。 */
export function compareId(a: string, b: string): number {
  const left = a.split('-').map(Number);
  const right = b.split('-').map(Number);
  for (let at = 0; at < Math.max(left.length, right.length); at += 1) {
    const diff = (left[at] ?? 0) - (right[at] ?? 0);
    if (diff !== 0) {
      return diff;
    }
  }
  return left.length - right.length;
}

/** 本文の文字数（空白を除く。§9.6.1）。 */
export function countChars(text: string): number {
  let found = 0;
  for (const ch of text) {
    if (!/\s/u.test(ch)) {
      found += 1;
    }
  }
  return found;
}

/** 節の文字数（`document.get` の全文と `section.list` の行の範囲から。表示の数え）。 */
export function sectionChars(text: string, lines: [number, number]): number {
  const rows = text.split('\n');
  const start = Math.max(0, Math.min(lines[0], rows.length));
  const end = Math.max(start, Math.min(lines[1], rows.length));
  return countChars(rows.slice(start, end).join('\n'));
}

// ---------------------------------------------------------------- 分量（§10.3.4・§10.9.5(c)）

/**
 * T スコア（`T = 50 + 10 × (x − 平均) / 標準偏差`。母集団標準偏差。小数第 0 位に四捨五入）。
 *
 * 文字数 0 の単位は母集団から除外（書いていないだけで「短い」と言わない）。
 * 母集団が 3 未満なら計算しない（null）。標準偏差 0 は全員 T 50。
 */
export function tScores(values: readonly number[]): (number | null)[] {
  const live = values.filter((value) => value > 0);
  const base = values.map((value) => (value > 0 ? value : null));
  if (live.length < 3) {
    return base.map(() => null);
  }
  const mean = live.reduce((sum, value) => sum + value, 0) / live.length;
  const variance = live.reduce((sum, value) => sum + (value - mean) ** 2, 0) / live.length;
  const sigma = Math.sqrt(variance);
  return base.map((value) => {
    if (value === null) {
      return null;
    }
    if (sigma === 0) {
      return 50;
    }
    return Math.round(50 + (10 * (value - mean)) / sigma);
  });
}

/** 外れ値か（`T < 35` または `T > 65`。W091 の条件）。 */
export function isOutlier(score: number | null): boolean {
  return score !== null && (score < 35 || score > 65);
}

/** ヒストグラム（`width` 字刻み。§10.9.5(c) は 100、§10.9.4 の 1 文は 20）。 */
export function histogram(values: readonly number[], width: number): { from: number; to: number; count: number }[] {
  const out: { from: number; to: number; count: number }[] = [];
  for (const value of values) {
    const from = Math.floor(value / width) * width;
    let found = out.find((bin) => bin.from === from);
    if (!found) {
      found = { from, to: from + width - 1, count: 0 };
      out.push(found);
    }
    found.count += 1;
  }
  out.sort((a, b) => a.from - b.from);
  return out;
}

/** 代表値（中央値・平均・最長・最短。`stage.length_summary` の文言に渡す）。 */
export function lengthSummary(values: readonly number[]): { median: number; mean: number; max: number; min: number } {
  if (values.length === 0) {
    return { median: 0, mean: 0, max: 0, min: 0 };
  }
  const sorted = [...values].sort((a, b) => a - b);
  const middle = sorted.length / 2;
  const median =
    sorted.length % 2 === 0
      ? Math.round(((sorted[middle - 1] ?? 0) + (sorted[middle] ?? 0)) / 2)
      : (sorted[Math.floor(middle)] ?? 0);
  const mean = Math.round(values.reduce((sum, value) => sum + value, 0) / values.length);
  return { median, mean, max: sorted[sorted.length - 1] ?? 0, min: sorted[0] ?? 0 };
}

// ---------------------------------------------------------------- 文体観察（§10.9.4）

/** 指標 1 つ（診断コードがあるものだけ集計する）。 */
export interface ObservationGroup {
  /** 指標の名前（`diag.W087` などの文言キー）。 */
  labelKey: string;
  /** 診断コード（`null` はこの版で未提供）。 */
  code: string | null;
  /** 既定で表示するか（§10.9.4: 既定は W082 と役割語未検出のみ ON）。 */
  defaultOn: boolean;
  /** この版で出せるか。 */
  available: boolean;
}

/** §10.9.4 の表の指標（上の順。役割語未検出は `cast.scan` の結果。1 文の長さ分布は W089 の行を出し、
 * ヒストグラム自体は操作が無いため「未提供」と出す）。 */
export const OBSERVATION_GROUPS: readonly ObservationGroup[] = [
  { labelKey: 'stage.obs_sentence_lengths', code: 'W089', defaultOn: false, available: true },
  { labelKey: 'diag.W081', code: 'W081', defaultOn: false, available: true },
  { labelKey: 'diag.W087', code: 'W087', defaultOn: false, available: true },
  { labelKey: 'diag.W088', code: 'W088', defaultOn: false, available: true },
  { labelKey: 'diag.W097', code: 'W097', defaultOn: false, available: true },
  { labelKey: 'stage.obs_dialogue_bands', code: 'W090', defaultOn: false, available: true },
  { labelKey: 'diag.W082', code: 'W082', defaultOn: true, available: true },
  { labelKey: 'diag.W096', code: 'W096', defaultOn: false, available: true },
  { labelKey: 'cast.tab_unmatched', code: null, defaultOn: true, available: true },
];

/** 既定で ON な指標（§10.9.4 MUST）。 */
export function defaultOnGroups(): string[] {
  return OBSERVATION_GROUPS.filter((group) => group.defaultOn).map((group) => group.labelKey);
}

/** 指標ごとに診断を分ける（コードが無い指標は空。フィルタの一時絞込は別）。 */
export function groupRows(
  rows: readonly DiagRow[],
  on: ReadonlySet<string>,
): { labelKey: string; code: string | null; rows: DiagRow[] }[] {
  return OBSERVATION_GROUPS.filter((group) => on.has(group.labelKey)).map((group) => ({
    labelKey: group.labelKey,
    code: group.code,
    rows: group.code ? rows.filter((row) => row.code === group.code) : [],
  }));
}

// ---------------------------------------------------------------- 挿絵メモ（追加仕様 23）

/** 挿絵メモの 1 行（`search.project` の行一致）。 */
export interface MemoRow {
  /** 節 ID（無ければ空）。 */
  scene: string;
  path: string;
  /** 1 始まり。 */
  line: number;
  /** 行そのもの（`! ` または `illustration:`）。 */
  text: string;
}

/** 挿絵メモか（`! ` 行または `illustration:` 行）。 */
export function isMemo(text: string): boolean {
  const line = text.trimStart();
  return line.startsWith('! ') || /^illustration:/u.test(line);
}

/** `search.project` のヒットを SceneId 順に並べる（追加仕様 23）。 */
export function memoRowsOf(
  hits: readonly { path: string; line: number; text: string }[],
  sections: readonly { id: string; path: string; lines: [number, number] }[],
): MemoRow[] {
  const out: MemoRow[] = [];
  for (const hit of hits) {
    if (!isMemo(hit.text)) {
      continue;
    }
    const found = sections.find(
      (section) => section.path === hit.path && hit.line >= section.lines[0] + 1 && hit.line <= section.lines[1],
    );
    out.push({ scene: found?.id ?? '', path: hit.path, line: hit.line, text: hit.text });
  }
  out.sort((a, b) => compareId(a.scene, b.scene) || a.path.localeCompare(b.path) || a.line - b.line);
  return out;
}

// ---------------------------------------------------------------- キー（この画面が横取りするもの）

/** この画面が横取りするキー（§10.9.1・§10.9.2）。 */
export type ReviewAction = 'tab-prev' | 'tab-next' | 'rerun' | 'open' | 'prev' | 'next' | 'first' | 'last';

/** キーの写像（無いキーは `null`＝全体の割当のまま）。 */
export function reviewAction(key: string): ReviewAction | null {
  switch (key) {
    case 'Ctrl+Shift+ArrowLeft':
      return 'tab-prev';
    case 'Ctrl+Shift+ArrowRight':
      return 'tab-next';
    case 'Ctrl+Shift+E':
      return 'rerun';
    case 'Enter':
      return 'open';
    case 'ArrowUp':
      return 'prev';
    case 'ArrowDown':
      return 'next';
    case 'Home':
      return 'first';
    case 'End':
      return 'last';
    default:
      return null;
  }
}
