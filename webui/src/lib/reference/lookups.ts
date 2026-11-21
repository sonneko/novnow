// 参照の引き口（`cast.get` / `dictionary.list` / `dictionary.term_full` / `diagnostics.place`）と
// 資料の添付（`materials.attach`）の規則。**画面を持たない純粋な関数だけ**（`node --test` で検証できる）。
//
// ここが持つのは「機能核の戻り値を、画面が並べられる形へ直す」判断だけである。意味は作らない
// （並べ替え・絞り込み・欠けた値の落とし方は、機能核の戻り値の形から決める）。
//
// ## 4 つの操作の戻り値（`REF:tests/golden/api_catalog.json`）
//
// | 操作 | 戻り値 | 備考 |
// |---|---|---|
// | `cast.get` | `{ cast: string }` | `cast` は**テキストのブロック**（1 人分が `- name:` で始まる） |
// | `dictionary.list` | `{ count, entries: [{ key, kind, normalized }] }` | `kind` は §6.1.1 の**種別名** |
// | `dictionary.term_full` | `{ words: [{ term, definition }] }` | `definition` は先頭の定義だけ |
// | `diagnostics.place` | `{ path, place }` | `place` は作家の言葉での「どこ」。決まらなければ `null` |
//
// `cast.get` のブロックは `REF:src/core/aicontext/packet.rs` の `cast_entry` が作る:
//
// ```
// - name: リリア
//   mode: B 冷静な観察者 / C 決意
//   personality: 呆れながらも見捨てない。
//   emotion: 好奇心が警戒心を上回りつつある。
//   role: 主人公の暴走を止める者。
//   role-language: 「*なのよ」「*あげるわよ」
// ```
//
// 見出し（`name`）と `key: 値` の行だけを読む。**知らない見出しも捨てない**（機能核が項目を
// 増やしても、画面が黙って落とさない）。並びは書かれた順のまま。

/** 参照の面が扱う 3 つの引き口（`LookupDialog.svelte` が出す面）。 */
export type LookupMode = 'cast' | 'dictionary' | 'term';

/** `cast.get` の 1 人分（`cast` のブロックを読んだもの）。 */
export interface CastEntry {
  /** 名前（`- name: …` の値。無ければ空）。 */
  name: string;
  /** `mode` / `personality` / … の値（書かれた順。見出しは小文字へ寄せる）。 */
  fields: { key: string; value: string }[];
}

/**
 * `cast.get` の `cast`（テキストのブロック）を行へ直す。
 *
 * 規則:
 *
 * - `name:` で 1 人を始める（先頭の `- ` は有っても無くてもよい）。
 * - `key: 値` の行は**直前の 1 人**の項目にする（`name` も項目として残す）。
 * - 人の前に行が来たら捨てる（見出しの無い値は誰のものか決められない）。
 * - 名前が空でも残す（「不明」と出すのは画面。ここで捨てない）。
 */
export function castEntries(block: unknown): CastEntry[] {
  if (typeof block !== 'string' || block.trim() === '') {
    return [];
  }
  const out: CastEntry[] = [];
  let current: CastEntry | null = null;
  for (const raw of block.split('\n')) {
    if (raw.trim() === '') {
      continue;
    }
    const field = /^\s*(?:-\s*)?([A-Za-z][A-Za-z0-9_-]*):\s?(.*)$/.exec(raw.trimEnd());
    if (!field) {
      continue;
    }
    const key = (field[1] ?? '').toLowerCase();
    const value = (field[2] ?? '').trim();
    if (key === 'name') {
      current = { name: value, fields: [{ key, value }] };
      out.push(current);
      continue;
    }
    if (current === null) {
      continue;
    }
    current.fields.push({ key, value });
  }
  return out;
}

/**
 * 配役の項目の見出し（`cast_entry` の 5 つ）を `pane.*` / `cast.*` の文言キーへ写す。
 *
 * 知らない見出しは空文字を返し、画面は**見出しの文字をそのまま**出す（新しい項目が増えても
 * 文言が消えない）。
 */
export function castFieldLabel(key: string): string {
  switch (key) {
    case 'mode':
      return 'cast.tab_modes';
    case 'personality':
      return 'pane.personality';
    case 'emotion':
      return 'pane.emotion';
    case 'role':
      return 'pane.role';
    case 'role-language':
      return 'pane.tone';
    default:
      return '';
  }
}

/** 辞書の 1 件（`dictionary.list` の `entries` の要素）。 */
export interface DictionaryEntry {
  /** 語（`characters.md` の名前・`words.md` の見出し・資料 ID など）。 */
  key: string;
  /** 種別名（`Character` / `Address` / `Word` / `Material` / `MaterialTitle`）。 */
  kind: string;
  /** 正規化した語（照合に使う形。機能核が返す）。 */
  normalized: string;
}

/** §6.1.1 の種別名（この 5 つだけが辞書に入る）。 */
export const DICTIONARY_KINDS = ['Character', 'Address', 'Word', 'Material', 'MaterialTitle'] as const;

/** 種別名が辞書の 5 種か。 */
export function isDictionaryKind(kind: string): boolean {
  return (DICTIONARY_KINDS as readonly string[]).includes(kind);
}

/**
 * 種別名を `api.op.*` 以外の既存の文言キーへ写す（画面が種別名をそのまま出さない。§12.2）。
 *
 * 知らない種別は空文字（画面は**そのまま**出す。勝手に「その他」へまとめない）。
 */
export function dictionaryKindLabel(kind: string): string {
  switch (kind) {
    case 'Character':
      return 'pane.cast';
    case 'Address':
      return 'stage.addresses';
    case 'Word':
      return 'pane.words';
    case 'Material':
    case 'MaterialTitle':
      return 'pane.materials';
    default:
      return '';
  }
}

/**
 * `dictionary.list` の `entries` を読む（壊れた要素は捨てる）。
 *
 * **並べ替えない**（機能核が種別 → 語の順に並べて返す。その順を画面で作り直さない）。
 */
export function dictionaryEntries(value: unknown): DictionaryEntry[] {
  if (!Array.isArray(value)) {
    return [];
  }
  const out: DictionaryEntry[] = [];
  for (const raw of value) {
    const row = raw as { key?: unknown; kind?: unknown; normalized?: unknown } | null;
    if (!row || typeof row.key !== 'string' || row.key === '') {
      continue;
    }
    out.push({
      key: row.key,
      kind: typeof row.kind === 'string' ? row.kind : '',
      normalized: typeof row.normalized === 'string' ? row.normalized : '',
    });
  }
  return out;
}

/**
 * 照合用の正規化（**画面の絞り込みだけ**に使う簡易版）。
 *
 * NFKC で全角英数・半角カナを寄せ、英字は小文字にし、前後の空白を落とす。辞書の衝突判定
 * （§6.1.1）は機能核が持つ。ここは「打った語を含む行を探す」ためだけの規則。
 */
export function normalize(text: string): string {
  return text.normalize('NFKC').toLowerCase().trim();
}

/**
 * 一覧を語で絞る（打鍵ごとの絞り込み）。空の語は全部通す。
 *
 * 照合には機能核が返す `normalized` を使う（画面が独自の正規化規則を持たない）。
 */
export function filterDictionary(
  entries: readonly DictionaryEntry[],
  query: string,
): DictionaryEntry[] {
  const needle = normalize(query);
  if (needle === '') {
    return [...entries];
  }
  return entries.filter(
    (entry) =>
      normalize(entry.normalized || entry.key).includes(needle) ||
      normalize(entry.key).includes(needle),
  );
}

/** 用語の 1 件（`dictionary.term_full` の `words` の要素）。 */
export interface TermHit {
  /** 語。 */
  term: string;
  /** 定義（先頭の定義だけ。空なら未定義）。 */
  definition: string;
}

/**
 * `dictionary.term_full` の `words` を読む（壊れた要素は捨てる）。
 *
 * 絞り込みは機能核が済ませている（`query` が必須）。画面は**並べ替えない**。
 */
export function termHits(value: unknown): TermHit[] {
  if (!Array.isArray(value)) {
    return [];
  }
  const out: TermHit[] = [];
  for (const raw of value) {
    const row = raw as { term?: unknown; definition?: unknown } | null;
    if (!row || typeof row.term !== 'string' || row.term === '') {
      continue;
    }
    out.push({
      term: row.term,
      definition: typeof row.definition === 'string' ? row.definition : '',
    });
  }
  return out;
}

/** `diagnostics.place` の `place`（作家の言葉での「どこ」）。 */
export interface PlaceRef {
  /** `contents` / `episode_plot` / `chapter_plot` / `whole_plot` / `character` / `word` / `idea` / `material` / `prompt` / `settings`。 */
  kind: string;
  /** 構造 ID（`contents` / `*_plot` のとき）。 */
  id: string;
  /** 人物名。 */
  name: string;
  /** モード（人物のとき）。 */
  mode: string;
  /** 用語。 */
  term: string;
  /** 資料の題。 */
  title: string;
  /** 役割（プロンプトのとき）。 */
  role: string;
  /** 節（プロンプトのとき）。 */
  section: string;
}

/**
 * `diagnostics.place` の `place` を読む（`null` なら「決まらなかった」）。
 *
 * 機能核は決まらなければ `place: null` を返す（`REF:src/app/ops/lang.rs` の `place_op`）。
 * **推測で埋めない**（決まらなければ「決まらない」と出す。§21.1.2）。
 */
export function placeRef(value: unknown): PlaceRef | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return null;
  }
  const row = value as Record<string, unknown>;
  const str = (key: string): string => (typeof row[key] === 'string' ? (row[key] as string) : '');
  const kind = str('kind');
  if (kind === '') {
    return null;
  }
  return {
    kind,
    id: str('id'),
    name: str('name'),
    mode: str('mode'),
    term: str('term'),
    title: str('title'),
    role: str('role'),
    section: str('section'),
  };
}

/**
 * 場所を `place.*` の文言へ写す（`REF:src/ui/i18n/mod.rs` の `place` と同じ表）。
 *
 * 画面が独自に文を組み立てない（§12.2）。知らない種別は `null`（画面は決まらないと出す）。
 */
export function placeKey(place: PlaceRef): { key: string; args: string[] } | null {
  switch (place.kind) {
    case 'contents':
      return { key: 'place.contents', args: [place.id] };
    case 'episode_plot':
      return { key: 'place.episode_plot', args: [place.id] };
    case 'chapter_plot':
      return place.id === ''
        ? { key: 'place.chapter_plot_head', args: [] }
        : { key: 'place.chapter_plot', args: [place.id] };
    case 'whole_plot':
      return place.id === ''
        ? { key: 'place.whole_plot_head', args: [] }
        : { key: 'place.whole_plot', args: [place.id] };
    case 'character':
      return place.mode === ''
        ? { key: 'place.character', args: [place.name] }
        : { key: 'place.character_mode', args: [place.name, place.mode] };
    case 'word':
      return { key: 'place.word', args: [place.term] };
    case 'idea':
      return { key: 'place.idea', args: [place.id, place.title] };
    case 'material':
      return place.title === ''
        ? { key: 'place.material_id', args: [place.id] }
        : { key: 'place.material', args: [place.id, place.title] };
    case 'prompt':
      return place.section === ''
        ? { key: 'place.prompt', args: [place.role] }
        : { key: 'place.prompt_section', args: [place.role, place.section] };
    case 'settings':
      return { key: 'place.settings', args: [] };
    default:
      return null;
  }
}

/**
 * `diagnostics.place` の引数を組み立てる（`path` は必須、`pos` は byte 位置）。
 *
 * 機能核は `pos` を省くと**文書全体**の場所を返す（`place::at` の `byte: None`）。
 * 診断に位置が無い（`byte` が `null`）ときは `pos` を送らない（0 を送ると先頭を指す）。
 */
export function placeArgs(path: string, byte: number | null): { path: string; pos?: number } | null {
  const found = path.trim();
  if (found === '') {
    return null;
  }
  return typeof byte === 'number' && Number.isFinite(byte) && byte >= 0
    ? { path: found, pos: Math.trunc(byte) }
    : { path: found };
}

/**
 * 資料の添付（`materials.attach`）の引数を組み立てる。
 *
 * 機能核（`REF:src/app/ops/knowledge.rs` の `materials_attach`）は:
 *
 * - `id` は**索引にある資料の ID**（無ければ `NotFound`）。
 * - `path` を渡すと、その MD が索引に無いときに**登録する**（W064 の「登録する」と同じ口。
 *   このとき `id` は索引の 1 件を探すためだけに使われ、返る `attached` は**新しい ID**）。
 * - `path` を省くと、その資料自身の MD を `updated_at` / `checksum` で更新する（添付）。
 *
 * 画面は 2 つの意味を分けて呼ぶ（同じ引数で別の意味にしない）。空の `id` は送らない
 * （機能核は必須引数なので `usage` で落ちる）。
 */
export function attachArgs(id: string, path = ''): { id: string; path?: string; role: string } | null {
  const found = id.trim();
  if (found === '') {
    return null;
  }
  const target = path.trim();
  return target === '' ? { id: found, role: '' } : { id: found, path: target, role: '' };
}

/**
 * 索引に無い資料 MD を登録するときの `id`（機能核が索引の 1 件を探すために使う**錨**）。
 *
 * 索引に 1 件も無ければ `null`（そのときは登録できない。押せないボタンを置かない。§21.1.2）。
 */
export function attachAnchor(ids: readonly (string | null)[]): string | null {
  return ids.find((id): id is string => typeof id === 'string' && id.trim() !== '') ?? null;
}

/**
 * 添付の結果（`materials.attach` の戻り値 `{ attached, path, save }`）を読む。
 *
 * `attached` は**登録された／更新された資料の ID**。登録のときは新しい ID なので、画面は
 * 選んでいた行の ID ではなくこちらを通知に出す。
 */
export function attachResult(value: unknown): { attached: string; path: string } {
  const row = (value ?? {}) as { attached?: unknown; path?: unknown };
  return {
    attached: typeof row.attached === 'string' ? row.attached : '',
    path: typeof row.path === 'string' ? row.path : '',
  };
}
