// プロジェクトの入口の面（`project.new` / `project.recent_list` / `project.recent_remove` /
// `project.import_zip` / `project.export_zip`。仕様 §9.7 の `Ctrl+O`、§13.5 の Zip の入出力）。
//
// ここが持つのは**規則**だけで、`run` は呼ばない（`ProjectPicker.svelte` と `App.svelte` が呼ぶ）。
// 第 1 の画面（`REF:src/ui/app.rs` のプロジェクト選択画面）と同じ並び・同じ段取りにする。
//
// ## 提供範囲（環境で違う。隠さない。§21.1.2）
//
// 正規操作そのものの可否は**機能核**が決める（`REF:src/app/ops/project.rs` の `#[cfg]`）。
// 画面は「押せるか」を先に知りたいので、同じ規則をここにも写す。写し間違えないよう、
// `webui/tests/project.test.mjs` が `src/app/ops/project.rs` の `cfg` を読んで突き合わせる。
//
// | 操作 | ネイティブ（Tauri） | ブラウザ（wasm） |
// |---|---|---|
// | `project.new` | 可（保存する場所を選んで作る） | **不可**（ファイルに書けない。`api.error.no_fs`） |
// | `project.recent_list` | 可（最近開いた一覧） | **不可**（同じ理由） |
// | `project.recent_remove` | 可（履歴からのみ削除） | **不可**（同じ理由） |
// | `project.import_zip` | 可 | 可（`bytes` で渡す。Zip を選んで読む） |
// | `project.export_zip` | 可（`out` に書く） | 可（`out` を省いて `bytes`/`name` を受け取り、画面がダウンロードする） |
//
// ブラウザの `export_zip` で `out` を渡さない理由: ブラウザ用の Host の `write_chosen` は
// **UTF-8 の文字列だけ**を受ける（`REF:crates/novnow-wasm/src/host.rs`）。Zip はバイナリなので
// `out` を渡すと失敗する。`out` を省けば `bytes`（base64）と `name` が返るので、画面が
// `Blob` を作ってダウンロードする（§21.2.3 の「利用者が明示したときだけ書き出す」に合う）。

/** 画面が動いている環境。 */
export type Runtime = 'native' | 'browser';

/** 入口の面で使える操作。 */
export interface ProjectCapabilities {
  /** 保存する場所を選んで新しく作れるか。 */
  newProject: boolean;
  /** 最近開いた一覧を出せるか。 */
  recentList: boolean;
  /** 履歴から削除できるか。 */
  recentRemove: boolean;
  /** Zip を取り込めるか。 */
  importZip: boolean;
  /** Zip を書き出せるか。 */
  exportZip: boolean;
  /**
   * `export_zip` に `out` を渡すか。
   *
   * - ネイティブ: 渡す（保存ダイアログの行き先へ Host が書く）。
   * - ブラウザ: 渡さない（`bytes` を受け取って画面がダウンロードする。上の注記）。
   */
  exportToPath: boolean;
}

/**
 * 環境から「押せるか」を決める。
 *
 * ブラウザでは `project.new` / `project.recent_*` が機能核で `no_fs` になるため押せない。
 * 未提供は押せないだけでなく、**理由を画面に書く**（§21.1.2 MUST）。
 */
export function capabilities(runtime: Runtime): ProjectCapabilities {
  const native = runtime === 'native';
  return {
    newProject: native,
    recentList: native,
    recentRemove: native,
    // Zip の入出力は両方で提供する（渡し方だけが違う）。
    importZip: true,
    exportZip: true,
    exportToPath: native,
  };
}

/** 一覧の 1 行（`project.recent_list` の `projects[]` と、いま開いている 1 件）。 */
export interface ProjectRow {
  /** 保存領域のキー／フォルダのパス。 */
  path: string;
  /** 最後に開いた時刻（`opened_at`）。いま開いている 1 件は空。 */
  openedAt: string;
  /** いま開いているか。 */
  current: boolean;
}

/**
 * 一覧の行を組み立てる。
 *
 * - いま開いている 1 件を**先頭**に置く（第 1 の画面と同じ。開いているものが一覧に無いことがある）。
 * - 同じパスは 1 度だけ（履歴が重複していても行を増やさない）。
 * - 最近開いた順は `opened_at` の**降順**（新しい順）。空（いま開いている 1 件）は先頭のまま。
 */
export function projectRows(current: string, recent: readonly { path: string; opened_at: string }[]): ProjectRow[] {
  const rows: ProjectRow[] = [];
  const seen = new Set<string>();
  if (current) {
    rows.push({ path: current, openedAt: '', current: true });
    seen.add(current);
  }
  const rest = recent
    .filter((item) => item.path && !seen.has(item.path))
    .map((item) => ({ path: item.path, openedAt: item.opened_at, current: false }));
  rest.sort((left, right) => (left.openedAt < right.openedAt ? 1 : left.openedAt > right.openedAt ? -1 : 0));
  for (const row of rest) {
    if (seen.has(row.path)) {
      continue;
    }
    seen.add(row.path);
    rows.push(row);
  }
  return rows;
}

/** 新しいプロジェクトの名前から、作る先のパスを決める。 */
export interface NewProjectPlan {
  /** `project.new` の `path`。 */
  path: string;
  /** `project.new` の `title`。 */
  title: string;
}

/** 作る先を組み立てられない理由。 */
export type NewProjectProblem = 'empty' | 'no-separator' | 'not-native';

/** 名前の検査（第 1 の画面の「名前に使えない文字」に合わせる）。 */
export function validProjectName(name: string): boolean {
  const trimmed = name.trim();
  return trimmed !== '' && !/[\\/:*?"<>|]/.test(trimmed);
}

/**
 * 新しいプロジェクトをどこに作るか（`project.new` の引数）。
 *
 * - `parent` は選んだフォルダ（ネイティブで利用者が選ぶ）。`parent` が空なら作れない
 *   （ブラウザにはフォルダの選択が無い。`capabilities` が `newProject: false` にしている）。
 * - パスの区切りは `/` に揃える（第 1 の画面と同じ文字列にするため）。
 */
export function newProjectPlan(
  parent: string,
  name: string,
  runtime: Runtime,
): NewProjectPlan | NewProjectProblem {
  if (runtime !== 'native') {
    return 'not-native';
  }
  const title = name.trim();
  if (title === '') {
    return 'empty';
  }
  const base = parent.trim().replace(/[\\/]+$/, '');
  if (base === '') {
    return 'no-separator';
  }
  return { path: `${base}/${title}`, title };
}

/** Zip の書き出しで受け取るもの（`project.export_zip` の戻り値）。 */
export interface ZipExport {
  /** base64 の Zip。 */
  bytes: string;
  /** ダウンロードの名前（`novnow-….zip`）。 */
  name: string;
  /** 実際に書いたパス（ネイティブで `out` を渡したときだけ）。 */
  out?: string;
}

/** `project.export_zip` の引数を作る（環境で `out` の有無が変わる）。 */
export function exportArgs(runtime: Runtime, out: string): Record<string, unknown> {
  if (capabilities(runtime).exportToPath && out.trim() !== '') {
    return { out: out.trim() };
  }
  // ブラウザは `bytes` を受け取る（`out` を省く）。ネイティブで行き先が空なら
  // 書き出さずに中身だけ返させる（黙って別の場所へ書かない）。
  return {};
}

/** `project.import_zip` の引数を作る（ブラウザは選んだファイルの base64 を渡す）。 */
export function importArgs(runtime: Runtime, source: { path?: string; bytes?: string }): Record<string, unknown> {
  if (runtime === 'native' && source.path) {
    return { path: source.path };
  }
  if (source.bytes) {
    return { bytes: source.bytes };
  }
  return {};
}

/** 取り込みの結果から、画面に出す 1 行を作る（`written` と `skipped` の件数）。 */
export function importSummary(written: readonly string[], skipped: readonly unknown[]): string {
  return `${written.length}/${written.length + skipped.length}`;
}

/**
 * 取り込みの前に確かめること（足りないファイルがあれば名前を返す）。
 *
 * 第 1 の画面は `project.pick.incomplete` で「不備のまま取り込む」か取り消すかを尋ねる。
 * ここでは判定だけを持ち、尋ねるのは画面が行う。
 */
export function missingFiles(written: readonly string[]): string[] {
  const required = ['project.json', 'ideas.md', 'words.md', 'characters.md'];
  const found = new Set(written.map((path) => path.trim()));
  return required.filter((name) => !found.has(name));
}

/** 履歴から消してよいか（いま開いている 1 件は消さない。第 1 の画面と同じ）。 */
export function canForget(row: ProjectRow): boolean {
  return !row.current && row.path.trim() !== '';
}
