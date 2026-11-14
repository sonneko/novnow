// L6（ネイティブ = Tauri。`REF:docs/plans.md#webui-w5`）の回帰。`webui/src-tauri/` だけを見る。
//
// 受け入れ（§11.5）は 2 つ:
//
// - 同じ作品をブラウザ版とネイティブ版で開き、保存後のファイルが 1 バイトも違わない。
// - 規則 10 の検査（`REF:tests/layering.rs`）が緑のまま。
//
// ここで確かめること:
//
// 1. **置き場所と設定**（設計 §21.4）: `tauri.conf.json`・`capabilities/default.json`・
//    `Cargo.toml`・`src/main.rs` が決まりどおりか（コマンドは 4 つ・許可は 3 つ・プラグインは
//    ダイアログだけ・独自の枠を作らない・自動更新を持たない）。
// 2. **規則 10**: `webui/src-tauri/` の Rust が、ファイルに書く API を 1 つも呼ばない
//    （`tests/layering.rs` は `src/` しか走査しないので、同じ一覧をここで当てる）。
// 3. **受け入れ**: ブラウザ用の橋渡し（wasm + IndexedDB）とネイティブ用の橋渡し（`FsHost`）に
//    同じ作品・同じ操作列を流し、戻り値の文字列と、保存後の全ファイルを突き合わせる。
// 4. 橋渡しの Rust 側のテスト（`cargo test`）。
//
// 3 と 4 は cargo と wasm の生成物（`webui/src/wasm/`）が要る。無い環境では**理由を出して
// 省く**（CI では省かずに落とす）。Tauri の窓そのものはここでは組まない: WebView の開発
// ライブラリが要り、作業サンドボックスにも CI の `webui` ジョブにも無い。代わりに、Tauri の
// コマンドが呼ぶのと**同じ関数**を標準入出力から叩く（`src-tauri/bridge/examples/stdio.rs`）。

import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

// ブラウザの Host は時刻をローカルのオフセット付きで、ネイティブの Host（`FsHost`）は UTC で書く。
// 表記が必ず食い違う時間帯で走らせて、「時刻を刻まない操作」が本当に時刻を含まないことと、
// 「時刻を刻む操作」の差が時刻の欄だけであることを、実行環境の時間帯に依らずに確かめる。
process.env.TZ = 'Asia/Tokyo';

const WEBUI = fileURLToPath(new URL('..', import.meta.url));
const REPO = path.resolve(WEBUI, '..');
const TAURI = path.join(WEBUI, 'src-tauri');
const BRIDGE = path.join(TAURI, 'bridge');

const read = (file) => readFileSync(file, 'utf8');
const json = (file) => JSON.parse(read(file));

// ---------------------------------------------------------------------------
// 1. 置き場所と設定（設計 §21.4）
// ---------------------------------------------------------------------------

/** Cargo.toml の表 `[table]` に書かれたキー（1 行 1 件の形だけを読む）。 */
function tomlKeys(text, table) {
  const keys = [];
  let current = '';
  for (const raw of text.split('\n')) {
    const line = raw.trim();
    if (line.startsWith('#') || line === '') {
      continue;
    }
    const header = /^\[+([^\]]+)\]+$/.exec(line);
    if (header) {
      current = header[1].trim();
      continue;
    }
    const entry = /^([A-Za-z0-9_-]+)\s*=/.exec(line);
    if (entry && current === table) {
      keys.push(entry[1]);
    }
  }
  return keys;
}

/** Cargo.toml の表 `[table]` の 1 行（`key = …` の右辺）。 */
function tomlValue(text, table, key) {
  let current = '';
  for (const raw of text.split('\n')) {
    const line = raw.trim();
    const header = /^\[+([^\]]+)\]+$/.exec(line);
    if (header) {
      current = header[1].trim();
      continue;
    }
    if (current === table && line.startsWith(`${key} `)) {
      return line.slice(line.indexOf('=') + 1).trim();
    }
  }
  return null;
}

/** PNG の幅・高さ・色の種類（IHDR）。 */
function pngHeader(file) {
  const bytes = readFileSync(file);
  assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a', `${file} は PNG でない`);
  return {
    width: bytes.readUInt32BE(16),
    height: bytes.readUInt32BE(20),
    depth: bytes[24],
    color: bytes[25],
  };
}

test('ネイティブの置き場所に、設計 §21.4 のファイルが揃っている', () => {
  for (const file of [
    'Cargo.toml',
    'Cargo.lock',
    'build.rs',
    'tauri.conf.json',
    'capabilities/default.json',
    'src/main.rs',
    'bridge/Cargo.toml',
    'bridge/src/lib.rs',
    'bridge/examples/stdio.rs',
    'icons/icon.png',
    'icons/icon.ico',
  ]) {
    assert.ok(existsSync(path.join(TAURI, file)), `webui/src-tauri/${file} が無い`);
  }
  // Tauri は Unix で RGBA の PNG を、Windows で ICO を要る（無い・形が違うとビルドが止まる）
  const icon = pngHeader(path.join(TAURI, 'icons/icon.png'));
  assert.deepEqual([icon.depth, icon.color], [8, 6], 'icons/icon.png は 8 bit の RGBA にする');
  assert.equal(icon.width, icon.height, 'icons/icon.png は正方形にする');
  const ico = readFileSync(path.join(TAURI, 'icons/icon.ico'));
  assert.deepEqual([ico.readUInt16LE(0), ico.readUInt16LE(2)], [0, 1], 'icons/icon.ico は ICO でない');
  assert.ok(ico.readUInt16LE(4) >= 1, 'icons/icon.ico に画像が無い');
});

test('tauri.conf.json は Vite の出力を配り、独自の枠も自動更新も持たない', () => {
  const conf = json(path.join(TAURI, 'tauri.conf.json'));

  // ビルド（設計 §21.4）: frontendDist は Vite の dist、前処理は npm run dev / build
  assert.equal(conf.build.frontendDist, '../dist');
  assert.equal(conf.build.beforeDevCommand, 'npm run dev');
  assert.equal(conf.build.beforeBuildCommand, 'npm run build');
  const vite = read(path.join(WEBUI, 'vite.config.ts'));
  const port = /port:\s*(\d+)/.exec(vite)?.[1];
  assert.ok(port, 'vite.config.ts から開発サーバのポートを読めない');
  assert.equal(conf.build.devUrl, `http://localhost:${port}`, 'devUrl が Vite の開発サーバと違う');
  assert.match(vite, /outDir:\s*'dist'/, 'Vite の出力先が dist でない');

  // 版は W6 まで 0.0.0（設計 §21.1）。識別子は既定値のままにしない
  assert.equal(conf.version, '0.0.0');
  assert.match(conf.identifier, /^[A-Za-z0-9.-]+$/);
  assert.notEqual(conf.identifier, 'com.tauri.dev');

  // 配布物（§21.4.2）: インストーラを作らない（実行ファイルだけ。同梱物・署名は §20 の改訂で決める）
  assert.equal(conf.bundle.active, false);
  assert.equal(conf.bundle.createUpdaterArtifacts ?? false, false);

  // 更新（§21.4.3）: 自動更新・遠隔の設定を持たない
  assert.deepEqual(Object.keys(conf.plugins ?? {}), [], 'プラグインの設定（updater など）を置かない');
  assert.doesNotMatch(JSON.stringify(conf), /https?:\/\/(?!localhost[:/])/, '外部の URL を置かない');

  // 窓（設計 §21.4）: OS の既定の見た目のまま。独自の枠を作らない
  assert.equal(conf.app.windows.length, 1);
  const window = conf.app.windows[0];
  assert.equal(window.label ?? 'main', 'main');
  for (const key of [
    'decorations',
    'titleBarStyle',
    'transparent',
    'hiddenTitle',
    'trafficLightPosition',
    'windowEffects',
    'shadow',
    'alwaysOnTop',
    'fullscreen',
    'zoomHotkeysEnabled',
    'additionalBrowserArgs',
    'userAgent',
    'proxyUrl',
    'dataDirectory',
  ]) {
    assert.ok(!(key in window), `窓の ${key} は既定のままにする`);
  }
  // WebView に永続の保存領域を持たせない（規則 10: 設定もキャッシュも履歴もプロジェクトの外に置かない）
  assert.equal(window.incognito, true);

  // 画面の橋渡しは `window.__TAURI__` から呼べる（npm の依存を足さずに済む。README の「コマンド」）
  assert.equal(conf.app.withGlobalTauri, true);

  // 権限: 使うのは capabilities/default.json だけ。資産プロトコル・トレイ・非公開 API は使わない
  assert.deepEqual(conf.app.security.capabilities, ['default']);
  assert.equal(conf.app.security.assetProtocol?.enable ?? false, false);
  assert.equal(conf.app.trayIcon ?? null, null);
  assert.equal(conf.app.macOSPrivateApi ?? false, false);
});

test('許可は core:default と保存・選択のダイアログだけ（設計 §21.4）', () => {
  const dir = path.join(TAURI, 'capabilities');
  assert.deepEqual(readdirSync(dir).sort(), ['default.json'], 'capabilities/ に置くのは default.json だけ');
  const capability = json(path.join(dir, 'default.json'));
  assert.equal(capability.identifier, 'default');
  assert.deepEqual(capability.windows, ['main']);
  assert.deepEqual(capability.permissions, ['core:default', 'dialog:allow-save', 'dialog:allow-open']);
  assert.ok(!('remote' in capability), '外部の URL に許可を与えない');
  assert.ok(!('webviews' in capability) && !('platforms' in capability));
});

test('Tauri 側の依存はダイアログのプラグインだけ（fs / http / shell / updater を足さない）', () => {
  const manifest = read(path.join(TAURI, 'Cargo.toml'));
  assert.deepEqual(tomlKeys(manifest, 'dependencies').sort(), [
    'novnow-native-bridge',
    'tauri',
    'tauri-plugin-dialog',
  ]);
  assert.deepEqual(tomlKeys(manifest, 'build-dependencies'), ['tauri-build']);
  // tauri の追加の feature（資産プロトコル・トレイ・devtools・非公開 API など）を立てない
  for (const name of ['tauri', 'tauri-build']) {
    const table = name === 'tauri' ? 'dependencies' : 'build-dependencies';
    assert.match(tomlValue(manifest, table, name), /features\s*=\s*\[\s*\]/, `${name} の feature を足さない`);
  }
  // root の workspace にも入れず、橋渡し（Tauri 無しで組む側）も別の workspace にする
  assert.ok(tomlKeys(manifest, 'workspace').includes('exclude'), '`[workspace]` と exclude が要る');
  assert.match(tomlValue(manifest, 'workspace', 'exclude'), /"bridge"/);

  // 橋渡しは機能核と serde_json だけ（Tauri に依存しない。だから窓なしで試せる）
  const bridge = read(path.join(BRIDGE, 'Cargo.toml'));
  assert.deepEqual(tomlKeys(bridge, 'dependencies').sort(), ['novnow', 'serde_json']);
  assert.match(tomlValue(bridge, 'dependencies', 'novnow'), /path\s*=\s*"\.\.\/\.\.\/\.\."/);
  assert.match(tomlValue(bridge, 'dependencies', 'novnow'), /default-features\s*=\s*false/);
  assert.ok(/^\[workspace\]$/m.test(bridge), '橋渡しは自分の workspace を持つ');
});

// ---------------------------------------------------------------------------
// 2. Rust の本文の走査（コマンド 4 つ・規則 10）
// ---------------------------------------------------------------------------

/** Rust の本文から、コメントと文字列を空白にしたもの（説明の例文を検査が拾わないように）。 */
function rustCode(text) {
  let out = '';
  let index = 0;
  const blank = (found) => found.replace(/[^\n]/g, ' ');
  while (index < text.length) {
    const rest = text.slice(index);
    let found = /^\/\/[^\n]*/.exec(rest) ?? /^\/\*[\s\S]*?\*\//.exec(rest);
    if (found) {
      out += blank(found[0]);
      index += found[0].length;
      continue;
    }
    const raw = /^b?r(#*)"/.exec(rest);
    if (raw) {
      const end = rest.indexOf(`"${raw[1]}`, raw[0].length);
      const length = end < 0 ? rest.length : end + 1 + raw[1].length;
      out += blank(rest.slice(0, length));
      index += length;
      continue;
    }
    // 文字列と文字（`'a'`・`'\n'`・`'\u{1F600}'`。ライフタイムの `'a` は閉じないので当たらない）
    found = /^b?"(?:\\[\s\S]|[^"\\])*"/.exec(rest);
    found ??= /^b?'(?:\\(?:u\{[0-9a-fA-F]+\}|[\s\S])|[^'\\\n])'/.exec(rest);
    if (found) {
      out += blank(found[0]);
      index += found[0].length;
      continue;
    }
    out += text[index];
    index += 1;
  }
  return out;
}

/** `webui/src-tauri/` の Rust（ビルドの生成物と、テストの準備は除く）。 */
function rustSources() {
  const found = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      const relative = path.relative(TAURI, full).split(path.sep).join('/');
      if (entry.isDirectory()) {
        // `bridge/tests/` は一時ディレクトリに作品を置くためにファイルを書く（本体ではない）
        if (['target', 'gen', 'node_modules'].includes(entry.name) || relative === 'bridge/tests') {
          continue;
        }
        walk(full);
      } else if (entry.name.endsWith('.rs')) {
        found.push(relative);
      }
    }
  };
  walk(TAURI);
  return found.sort();
}

test('コマンドは open / run / poll_events / poll_external の 4 つだけ（設計 §21.4）', () => {
  const code = rustCode(read(path.join(TAURI, 'src/main.rs')));
  const commands = [...code.matchAll(/#\[tauri::command([^\]]*)\]\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)/g)];
  assert.deepEqual(
    commands.map((found) => found[2]),
    ['open', 'run', 'poll_events', 'poll_external'],
  );
  const handler = /generate_handler!\[([^\]]*)\]/.exec(code);
  assert.ok(handler, 'generate_handler! が無い');
  assert.deepEqual(
    handler[1].split(',').map((name) => name.trim()).filter(Boolean),
    ['open', 'run', 'poll_events', 'poll_external'],
  );
  // 引数と戻り値は文字列の JSON（§8.10 の 32）。戻り値はどれも `String`
  for (const name of ['open', 'run', 'poll_events', 'poll_external']) {
    const signature = new RegExp(`fn\\s+${name}\\s*\\([^)]*\\)\\s*->\\s*String\\b`);
    assert.match(code, signature, `${name} は String を返す`);
  }
  // `Session` は 1 つだけ、`Mutex` で持つ（§8.10 の 31）
  assert.match(code, /struct\s+Shared\s*\(\s*Mutex<NativeSession>\s*\)/);
  assert.equal([...code.matchAll(/\.manage\(/g)].length, 1, '管理する状態は 1 つだけ');
  // プラグインはダイアログだけ。ファイル入出力の口（fs プラグイン）を開かない
  const plugins = [...code.matchAll(/\.plugin\(\s*([\w:]+)/g)].map((found) => found[1]);
  assert.deepEqual(plugins, ['tauri_plugin_dialog::init']);
  // 新しい Host を書かない（`FsHost` をそのまま使う）
  for (const file of rustSources()) {
    const body = rustCode(read(path.join(TAURI, file)));
    assert.doesNotMatch(body, /impl\s+(?:[\w:]+::)?Host\s+for\b/, `${file}: Host を実装しない`);
    assert.doesNotMatch(body, /\bunsafe\b/, `${file}: unsafe を使わない`);
  }
  // 開く・作るは機能核の `Session::open` / `Session::create`（中で `FsHost` を作る）
  const bridge = rustCode(read(path.join(BRIDGE, 'src/lib.rs')));
  assert.match(bridge, /Session::open\(/);
  assert.match(bridge, /Session::create\(/);
});

test('引数の名前は Rust と同じ綴りで渡す（rename_all = "snake_case"）', () => {
  const text = read(path.join(TAURI, 'src/main.rs'));
  const attributes = [...text.matchAll(/^#\[tauri::command([^\]]*)\]/gm)].map((found) => found[1]);
  assert.equal(attributes.length, 4);
  for (const attribute of attributes) {
    assert.equal(attribute, '(rename_all = "snake_case")');
  }
});

/** `tests/layering.rs` の定数（文字列の配列）を読む。 */
function layeringList(name) {
  const text = read(path.join(REPO, 'tests/layering.rs'));
  const found = new RegExp(`const ${name}: &\\[&str\\] = &\\[([^\\]]*)\\];`).exec(text);
  assert.ok(found, `tests/layering.rs から ${name} を読めない（一覧の形が変わったら、ここも合わせる）`);
  const items = [...found[1].matchAll(/"([^"]+)"/g)].map((item) => item[1]);
  assert.ok(items.length > 0, `${name} が空`);
  return items;
}

/** 書き込み API の呼び出し（`tests/layering.rs` の `write_api` と同じ規則）。 */
function writeApis(code, fsWrites, fileWrites) {
  const hits = [];
  const names = (group) =>
    group
      .replace(/[{}]/g, '')
      .split(',')
      .map((item) => item.trim().split(/\s+as\s+/)[0].split('::').pop())
      .filter(Boolean);
  if (/\bOpenOptions\b/.test(code)) {
    hits.push('OpenOptions');
  }
  if (/\btokio\s*::\s*fs\b/.test(code)) {
    hits.push('tokio::fs');
  }
  // `use std::fs as f;` のように別名で隠さない
  if (/\bfs\s+as\s+\w+/.test(code)) {
    hits.push('fs as …');
  }
  for (const [head, list] of [
    ['fs', fsWrites],
    ['File', fileWrites],
  ]) {
    const pattern = new RegExp(`\\b${head}\\s*::\\s*(\\*|\\{[^}]*\\}|[A-Za-z_]\\w*)`, 'g');
    for (const found of code.matchAll(pattern)) {
      for (const name of found[1].startsWith('{') ? names(found[1]) : [found[1]]) {
        if (list.includes(name) || (head === 'fs' && name === '*')) {
          hits.push(`${head}::${name}`);
        }
      }
    }
  }
  return hits;
}

test('規則 10: webui/src-tauri の Rust は、ファイルに書く API を 1 つも呼ばない', () => {
  // 一覧は `tests/layering.rs`（§13.1.2 の検査）から読む。写しを持たない
  const fsWrites = layeringList('FS_WRITE_FNS');
  const fileWrites = layeringList('FILE_WRITE_FNS');
  assert.ok(fsWrites.includes('write') && fsWrites.includes('rename') && fileWrites.includes('create'));

  // 検査そのものが効いていること（素通しの検査で緑にしない）
  const samples = [
    // 書き込みは見つける（`use` の中・呼び出し・`File::create`）
    [
      'use std::fs::{self, read_dir, write};\nfn f() { fs::rename(a, b); std::fs::File::create(p); }',
      ['fs::write', 'fs::rename', 'File::create'],
    ],
    // コメントと文字列の中の例文は拾わない
    ['// fs::write(a, b)\nlet text = "fs::write";', []],
    // 読み取りは許す
    ['std::fs::read_dir(root); fs::symlink_metadata(p); File::open(p);', []],
  ];
  for (const [source, expected] of samples) {
    assert.deepEqual(writeApis(rustCode(source), fsWrites, fileWrites), expected, source);
  }

  const sources = rustSources();
  for (const expected of ['build.rs', 'src/main.rs', 'bridge/src/lib.rs', 'bridge/examples/stdio.rs']) {
    assert.ok(sources.includes(expected), `${expected} を走査していない`);
  }
  for (const file of sources) {
    const hits = writeApis(rustCode(read(path.join(TAURI, file))), fsWrites, fileWrites);
    assert.deepEqual(hits, [], `webui/src-tauri/${file}: 書き込みは FsHost（関門 FsGate）だけが行う`);
  }
});

// ---------------------------------------------------------------------------
// 依存の版（Cargo.lock）
// ---------------------------------------------------------------------------

/** Cargo.lock の `[[package]]`。 */
function lockPackages(text) {
  return text
    .split('[[package]]')
    .slice(1)
    .map((block) => {
      const field = (name) => new RegExp(`^${name} = "([^"]+)"`, 'm').exec(block)?.[1] ?? null;
      const dependencies = /^dependencies = \[([\s\S]*?)\]/m.exec(block)?.[1] ?? '';
      return {
        name: field('name'),
        version: field('version'),
        source: field('source'),
        checksum: field('checksum'),
        dependencies: [...dependencies.matchAll(/"([^"]+)"/g)].map((item) => item[1]),
      };
    });
}

/** lock の中で、依存の 1 行（`名前` / `名前 版` / `名前 版 (取得元)`）が指すパッケージ。 */
function lockTarget(packages, dependency) {
  const [name, version] = dependency.split(' ');
  return packages.filter((item) => item.name === name && (version === undefined || item.version === version));
}

/**
 * `start` から辿れる依存の辺のうち、2 つの lock で**同じ辺が違う版を指す**もの。
 *
 * `found` にだけある辺（Tauri が同じクレートの feature を足して増えた依存）は比べない。
 * 比べるのは「機能核がリンクする同じ依存が、同じ版・同じ照合値か」だけである。
 */
function lockDrift(found, expected, start) {
  const drift = [];
  const seen = new Set();
  const same = (left, right) => left.version === right.version && left.checksum === right.checksum;
  const visit = (mine, theirs) => {
    const key = `${mine.name} ${mine.version}`;
    if (seen.has(key)) {
      return;
    }
    seen.add(key);
    for (const dependency of mine.dependencies) {
      const name = dependency.split(' ')[0];
      const wanted = theirs.dependencies
        .filter((item) => item.split(' ')[0] === name)
        .flatMap((item) => lockTarget(expected, item));
      if (wanted.length === 0) {
        continue;
      }
      for (const target of lockTarget(found, dependency)) {
        const match = wanted.find((item) => same(item, target));
        if (match) {
          visit(target, match);
        } else {
          const versions = wanted.map((item) => item.version).join(' / ');
          drift.push(`${target.name} ${target.version}（root は ${versions}）`);
        }
      }
    }
  };
  const mine = found.find((item) => item.name === start);
  const theirs = expected.find((item) => item.name === start);
  if (!mine || !theirs) {
    return { drift: [`${start} が lock に無い`], compared: 0 };
  }
  if (!same(mine, theirs)) {
    drift.push(`${start} ${mine.version}（root は ${theirs.version}）`);
  }
  visit(mine, theirs);
  return { drift: [...new Set(drift)], compared: seen.size };
}

test('ネイティブの Cargo.lock は、機能核の依存を root の Cargo.lock と同じ版で固定している', (t) => {
  const root = lockPackages(read(path.join(REPO, 'Cargo.lock')));
  const native = lockPackages(read(path.join(TAURI, 'Cargo.lock')));
  const pinned = new Map(native.map((item) => [item.name, item]));

  // Tauri 2 と、ダイアログのプラグインだけ（fs は dialog が型を借りるだけで、初期化も許可もしない）
  assert.match(pinned.get('tauri')?.version ?? '', /^2\./, 'Tauri 2 を使う');
  assert.match(pinned.get('tauri-plugin-dialog')?.version ?? '', /^2\./);
  for (const forbidden of [
    'tauri-plugin-http',
    'tauri-plugin-shell',
    'tauri-plugin-updater',
    'tauri-plugin-autostart',
    'tauri-plugin-process',
    'tauri-plugin-store',
    'tauri-plugin-window-state',
    'tauri-plugin-persisted-scope',
  ]) {
    assert.ok(!pinned.has(forbidden), `${forbidden} を持ち込まない`);
  }
  for (const item of native) {
    assert.ok(
      item.source === null || item.source === 'registry+https://github.com/rust-lang/crates.io-index',
      `${item.name}: 取得元は crates.io だけ`,
    );
  }

  // 検査そのものが効いていること（版をずらした lock は見つかる）
  const shifted = native.map((item) => (item.name === 'serde_json' ? { ...item, version: '0.0.0' } : item));
  assert.ok(lockDrift(shifted, root, 'novnow').drift.some((found) => found.startsWith('serde_json 0.0.0')));

  // 機能核（novnow）がリンクする依存は、ブラウザ版（root の lock）と同じ版・同じ照合値
  const { drift, compared } = lockDrift(native, root, 'novnow');
  assert.ok(compared > 50, `比べた依存が少ない（${compared} 件）`);
  if (drift.length > 0) {
    const message =
      `root の Cargo.lock と版が違う: ${drift.join('、')}。` +
      'webui/src-tauri/README.md の「lock を揃える」で作り直す（ネイティブ版を組む前に必ず揃える）';
    // root の lock は他のレーンの持ち物なので、ふだんは他の作業を止めない。ネイティブ版を組む
    // ときだけ `WEBUI_NATIVE_STRICT=1` で落とす。
    if (process.env.WEBUI_NATIVE_STRICT) {
      assert.fail(message);
    }
    t.skip(message);
  }
});

// ---------------------------------------------------------------------------
// 3. 受け入れ: ブラウザ版と同じ JSON・同じファイル
// ---------------------------------------------------------------------------

/** cargo の呼び方。PATH に無ければ作業サンドボックスのツールチェーン（`docs/developers.md` §2.2）を読む。 */
let cargo_ = undefined;
function cargo() {
  if (cargo_ !== undefined) {
    return cargo_;
  }
  cargo_ = null;
  if (spawnSync('cargo', ['--version'], { stdio: 'ignore' }).status === 0) {
    cargo_ = ['cargo'];
    return cargo_;
  }
  const env = path.join(process.env.BUILD_DIR ?? path.join(homedir(), 'build'), 'env.sh');
  if (existsSync(env)) {
    const prefix = ['bash', '-c', '. "$0" && exec cargo "$@"', env];
    if (spawnSync(prefix[0], [...prefix.slice(1), '--version'], { stdio: 'ignore' }).status === 0) {
      cargo_ = prefix;
    }
  }
  return cargo_;
}

/** cargo を走らせる（失敗したら出力ごと落とす）。 */
function runCargo(args) {
  const [program, ...prefix] = cargo();
  const found = spawnSync(program, [...prefix, ...args], {
    cwd: BRIDGE,
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
  });
  assert.equal(
    found.status,
    0,
    `cargo ${args.join(' ')} が失敗した:\n${found.stderr?.slice(-6000)}\n${found.stdout?.slice(-6000)}`,
  );
  return found;
}

/**
 * 橋渡し（Tauri 無しの側）を組んで、標準入出力の口の場所を返す（1 回だけ）。
 *
 * 依存の版は **root の `Cargo.lock` に合わせる**（ブラウザ用の wasm と同じ版で組んだものを
 * 比べる）。`bridge/Cargo.lock` は置かずに、組むたびに root の lock から作る。
 */
let driver_ = null;
function driver() {
  if (driver_ === null) {
    copyFileSync(path.join(REPO, 'Cargo.lock'), path.join(BRIDGE, 'Cargo.lock'));
    const built = runCargo(['build', '--example', 'stdio', '--message-format=json']);
    for (const line of built.stdout.split('\n')) {
      if (!line.startsWith('{')) {
        continue;
      }
      const message = JSON.parse(line);
      if (
        message.reason === 'compiler-artifact' &&
        message.target?.name === 'stdio' &&
        message.target?.kind?.includes('example') &&
        message.executable
      ) {
        driver_ = message.executable;
      }
    }
    assert.ok(driver_, 'cargo の出力から stdio の実行ファイルを見つけられない');
  }
  return driver_;
}

/** wasm（`scripts/webui-build.sh` の生成物）。無ければ `null`。 */
let wasm_ = undefined;
async function wasm() {
  if (wasm_ === undefined) {
    const entry = path.join(WEBUI, 'src/wasm/novnow_wasm.js');
    if (!existsSync(entry)) {
      wasm_ = null;
    } else {
      wasm_ = await import('../src/wasm/novnow_wasm.js');
      const bytes = await readFile(path.join(WEBUI, 'src/wasm/novnow_wasm_bg.wasm'));
      await wasm_.default({ module_or_path: bytes });
    }
  }
  return wasm_;
}

/**
 * 道具が無いときの扱い: 手元では理由を出して省き、CI では落とす（受け入れの検査を黙って失わない）。
 * `needs.wasm` を `false` にすると cargo だけを要る（Rust 側だけのテスト）。
 */
async function ready(t, needs = { wasm: true }) {
  let missing = null;
  if (cargo() === null) {
    missing = 'cargo が無い（bash scripts/bootstrap-toolchain.sh）';
  } else if (needs.wasm && (await wasm()) === null) {
    missing = 'webui/src/wasm/ が無い（bash scripts/webui-build.sh）';
  }
  if (missing === null) {
    return true;
  }
  if (process.env.CI) {
    assert.fail(`${missing}。CI では省かない`);
  }
  t.skip(missing);
  return false;
}

/** ブラウザの代わりに置く最小の `window`（`REF:webui/tests/session.test.mjs` と同じ）。 */
async function browser() {
  const { IDBFactory, IDBKeyRange } = await import('fake-indexeddb');
  const factory = new IDBFactory();
  globalThis.indexedDB = factory;
  globalThis.IDBKeyRange = IDBKeyRange;
  globalThis.window = globalThis;
  globalThis.self = globalThis;
  return factory;
}

/** ブラウザ版が保存したファイル（IndexedDB の `files`。パス → バイト列）。 */
async function browserFiles(factory, project) {
  const databases = await factory.databases();
  assert.deepEqual(
    databases.map((found) => found.name),
    ['novnow-webui'],
    '第 2 のブラウザ版の保存領域は novnow-webui だけ',
  );
  const records = await new Promise((resolve, reject) => {
    const opening = factory.open('novnow-webui');
    opening.onerror = () => reject(opening.error);
    opening.onsuccess = () => {
      const db = opening.result;
      const request = db.transaction('files', 'readonly').objectStore('files').getAll();
      request.onerror = () => reject(request.error);
      request.onsuccess = () => {
        db.close();
        resolve(request.result);
      };
    };
  });
  const files = new Map();
  for (const record of records) {
    if (record.project_id === project) {
      assert.equal(typeof record.text, 'string', `${record.path}: テキストでないレコード`);
      files.set(record.path, Buffer.from(record.text, 'utf8'));
    }
  }
  return files;
}

/** ネイティブ版が保存したファイル（ディスク。パス → バイト列）。 */
async function nativeFiles(root, dir = root, files = new Map()) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      await nativeFiles(root, full, files);
    } else {
      files.set(path.relative(root, full).split(path.sep).join('/'), await readFile(full));
    }
  }
  return files;
}

/** ネイティブの橋渡し（標準入出力の口）。Tauri のコマンドと同じ 4 つ。 */
class Native {
  constructor() {
    this.child = spawn(driver(), [], { stdio: ['pipe', 'pipe', 'inherit'] });
    this.waiting = [];
    this.failure = null;
    createInterface({ input: this.child.stdout }).on('line', (line) => this.waiting.shift()?.resolve(line));
    const stop = (error) => {
      this.failure = error;
      for (const pending of this.waiting.splice(0)) {
        pending.reject(error);
      }
    };
    this.child.on('error', stop);
    this.child.on('exit', (code) => stop(new Error(`ネイティブの橋渡しが終わった（exit ${code}）`)));
  }

  call(request) {
    if (this.failure) {
      return Promise.reject(this.failure);
    }
    return new Promise((resolve, reject) => {
      this.waiting.push({ resolve, reject });
      this.child.stdin.write(`${JSON.stringify(request)}\n`);
    });
  }

  open(root, createOptions = null) {
    return this.call({ cmd: 'open', path: root, create_options: createOptions });
  }

  run(op, args) {
    return this.call({ cmd: 'run', op, args });
  }

  pollEvents() {
    return this.call({ cmd: 'poll_events' });
  }

  pollExternal() {
    return this.call({ cmd: 'poll_external' });
  }

  close() {
    this.child.removeAllListeners('exit');
    this.child.stdin.end();
  }
}

/**
 * 同じ作品を 2 つの橋渡しで開く。
 *
 * ブラウザ版は付録 C のサンプルを保存領域（IndexedDB）へ入れて開く。その**保存領域の中身を
 * そのまま**ディレクトリへ書き出し、ネイティブ版で開く（＝同じ作品）。
 */
async function openBoth(t) {
  const factory = await browser();
  const { NovnowSession } = await wasm();
  const session = new NovnowSession();
  const native = new Native();
  const root = await mkdtemp(path.join(tmpdir(), 'novnow-l6-'));
  t.after(async () => {
    native.close();
    await rm(root, { recursive: true, force: true });
  });

  // 開く前の答えも同じ（例外にしない・同じ文言）
  assert.equal(await native.run('project.info', '{}'), session.run('project.info', '{}'));
  assert.equal(await native.pollEvents(), session.poll_events());

  await session.open(null);
  const project = session.project_id();
  const seed = await browserFiles(factory, project);
  assert.ok(seed.size > 20, 'サンプル作品が入っていない');
  for (const [file, bytes] of seed) {
    const target = path.join(root, ...file.split('/'));
    await mkdir(path.dirname(target), { recursive: true });
    await writeFile(target, bytes);
  }
  const opened = JSON.parse(await native.open(root));
  assert.equal(opened.ok, true, `ネイティブ版で開けない: ${JSON.stringify(opened)}`);
  assert.deepEqual(Object.keys(opened.result), ['locale', 'root', 'version']);
  assert.equal(opened.result.locale, session.locale());
  assert.equal(opened.result.version, session.version());

  /** 同じ操作を両方へ流し、戻り値の**文字列**が同じことを確かめる。 */
  const run = async (op, args) => {
    const text = typeof args === 'string' ? args : JSON.stringify(args);
    const expected = session.run(op, text);
    assert.equal(await native.run(op, text), expected, `${op}: ブラウザ版と戻り値が違う`);
    return JSON.parse(expected);
  };
  /** 保存領域へ流し終えた後の、両方のファイル。 */
  const saved = async () => {
    const flushed = JSON.parse(await session.flush());
    assert.equal(flushed.ok, true, `flush に失敗した: ${JSON.stringify(flushed)}`);
    assert.equal(session.pending_writes(), 0);
    return { browser: await browserFiles(factory, project), native: await nativeFiles(root) };
  };
  return { session, native, root, seed, run, saved };
}

/** cargo のビルドを含むテストの上限（初回は機能核をネイティブ向けに組む）。 */
const SLOW = { timeout: 30 * 60_000 };

test('受け入れ: 同じ作品・同じ操作で、戻り値も保存後のファイルも 1 バイトも違わない', SLOW, async (t) => {
  if (!(await ready(t))) {
    return;
  }
  const { session, native, seed, run, saved } = await openBoth(t);

  const first = (await run('section.list', {})).result.sections[0].id;
  const steps = [
    // 読むだけの操作
    ['project.info', {}],
    ['structure.breadcrumb', {}],
    ['section.get', { scene: first }],
    ['settings.get', {}],
    ['save.state', {}],
    ['project.validate', {}],
    ['api.list', {}],
    ['plot.get', { level: 'all' }],
    ['search.file', { query: 'アルス' }],
    // 本文・プロット・辞書・アイデアを書く操作（時刻を刻まないもの）
    ['section.write', { scene: first, text: '受け入れの確認。\n二行目も書く。', role: '' }],
    ['nav.section', { scene: '01-04-02-01' }],
    ['section.insert', { after: '01-04-02-01', text: '挿し込んだ節。', role: '' }],
    ['section.split', { scene: '01-04-02-02', at: 5, role: '' }],
    ['section.delete', { scene: '01-04-02-04', role: '' }],
    ['plot.write', { level: 'all', text: '@01 失われた刻印\nsummary: 書き換えたあらすじ。\n;\n', role: '' }],
    ['document.write', { path: 'words.md', text: '# 星図\ndefinition: 古い星図。\n', role: '' }],
    ['dictionary.register_term', { term: '羅針盤', definition: '北を指さない羅針盤。', role: '' }],
    ['dictionary.register_character', { name: 'ミラ', reading: 'みら', role: '' }],
    ['idea.write', { id: 'I0001', text: '書き直した思いつき。', role: '' }],
    ['idea.drop', { id: 'I0001', role: '' }],
    // 設定（`.novnow/ui.json`）と画面の状態
    ['settings.set', { key: 'theme', value: 'dark' }],
    ['settings.set', { key: 'font_scale', value: '1.25' }],
    ['session.workspace_switch', { workspace: 'plot-outline' }],
    ['session.workspace_get', {}],
    ['agent.delegation_set', { level: 'propose' }],
    // 保存
    ['save.state', {}],
    ['save.now', {}],
    ['save.state', {}],
    // 保存の後の読み取り
    ['section.list', {}],
    ['project.validate', {}],
    ['diagnostics.run', { scope: 'full' }],
    ['project.stats', {}],
    ['lang.tokens', { path: 'contents/01-04.cnov' }],
    ['lang.symbols', { path: 'contents/01-04.cnov' }],
    ['dictionary.list', {}],
    ['cast.list', {}],
    ['idea.list', {}],
    ['materials.list', {}],
    ['review.list', {}],
    ['agent.task_list', {}],
    ['help.topic', { topic: 'shortcuts' }],
    // 失敗も同じ形・同じ文言
    ['no.such', {}],
    ['document.get', { path: 'nope.md' }],
    ['section.get', {}],
    // 引数の省略と誤り（文字列のまま渡す）
    ['project.info', ''],
    ['project.info', 'null'],
    ['project.info', '[1]'],
    ['project.info', '{'],
  ];
  let succeeded = 0;
  for (const [op, args] of steps) {
    if ((await run(op, args)).ok) {
      succeeded += 1;
    }
  }
  assert.ok(succeeded >= 40, `成功した操作が少ない（${succeeded} 件）。筋書きが古い`);

  // 出来事（`AppEvent` の配列）も同じ文字列
  const events = session.poll_events();
  assert.equal(await native.pollEvents(), events, 'poll_events がブラウザ版と違う');
  assert.ok(JSON.parse(events).length > 0, '出来事が 1 件も無い');

  // 保存後のファイル: 一覧も中身も同じ
  const files = await saved();
  const names = (found) => [...found.keys()].sort();
  assert.deepEqual(names(files.native), names(files.browser), '保存後のファイルの一覧が違う');
  for (const [file, bytes] of files.browser) {
    assert.ok(bytes.equals(files.native.get(file)), `${file}: ブラウザ版と中身が違う`);
  }
  // 何も書いていない比較で緑にしない: 本文・プロット・辞書・アイデア・設定が実際に変わっている
  const changed = names(files.browser).filter((file) => !seed.get(file)?.equals(files.browser.get(file)));
  for (const expected of [
    '.novnow/ui.json',
    'characters.md',
    'contents/01-04.cnov',
    'ideas.md',
    'plots/all.cnov',
    'words.md',
  ]) {
    assert.ok(changed.includes(expected), `${expected} が保存されていない（変わったのは ${changed}）`);
  }
  // 時刻を刻まない操作だけなので、監査ログも増えていない
  assert.deepEqual(
    changed.filter((file) => file.startsWith('agents/log/')),
    [],
  );

  // 保存の後は、どちらにも外部変更が無い（ブラウザは監視しない。ネイティブは自分の保存に反応しない）
  await native.pollExternal();
  for (const event of JSON.parse(await native.pollEvents())) {
    assert.equal(event.kind, 'document_changed', `自分の保存に反応した: ${JSON.stringify(event)}`);
    assert.equal(event.change, 'unchanged', `自分の保存に反応した: ${JSON.stringify(event)}`);
  }
});

/** RFC 3339 の時刻（Host の時計が入れる値）。 */
const TIMESTAMP = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})/g;
/** 時刻があるか（`g` 付きの正規表現は `test` が位置を覚えるので、別に持つ）。 */
const HAS_TIMESTAMP = new RegExp(TIMESTAMP.source);
/** 書き出しの既定の名前の日時（`YYYYMMDD-HHMM`）。 */
const STAMP = /\d{8}-\d{4}/g;
/** 監査ログの名前の日付（`agents/log/YYYY-MM-DD.jsonl`）。 */
const LOG_DATE = /^(agents\/log\/)\d{4}-\d{2}-\d{2}(\.jsonl)$/;

test('時刻を刻む操作は、時刻の欄だけがブラウザ版と違う（Host の時計の表記）', SLOW, async (t) => {
  if (!(await ready(t))) {
    return;
  }
  const { session, native, seed, saved } = await openBoth(t);

  /** 時刻を伏せた文字列。 */
  const mask = (text) => text.replace(TIMESTAMP, '<time>').replace(STAMP, '<stamp>');
  const steps = [
    ['idea.capture', { title: '時刻の確認', role: '' }],
    ['materials.propose', { title: '時刻の資料', text: '本文', role: '' }],
    ['search.replace_file', { query: '図書室', replace: '書庫', role: '' }],
    ['save.now', {}],
    ['idea.list', {}],
    ['document.get', { path: 'ideas.md' }],
    ['materials.list', {}],
    ['export.text', {}],
  ];
  for (const [op, args] of steps) {
    const text = JSON.stringify(args);
    const expected = session.run(op, text);
    const found = await native.run(op, text);
    assert.equal(JSON.parse(expected).ok, true, `${op}: ${expected}`);
    assert.equal(mask(found), mask(expected), `${op}: 時刻のほかにブラウザ版と違うところがある`);
  }

  const files = await saved();
  const name = (file) => file.replace(LOG_DATE, '$1<date>$2');
  const masked = (found) =>
    new Map([...found].map(([file, bytes]) => [name(file), mask(bytes.toString('utf8'))]));
  const browser = masked(files.browser);
  const disk = masked(files.native);
  assert.deepEqual([...disk.keys()].sort(), [...browser.keys()].sort(), '保存後のファイルの一覧が違う');
  for (const [file, text] of browser) {
    assert.equal(disk.get(file), text, `${file}: 時刻のほかにブラウザ版と違うところがある`);
  }

  // 違いは Host の時計の表記そのもの: ブラウザはローカルのオフセット付き、`FsHost` は UTC
  const stamped = [...files.browser.keys()].filter((file) => {
    const bytes = files.browser.get(file);
    return !seed.get(file)?.equals(bytes) && HAS_TIMESTAMP.test(bytes.toString('utf8'));
  });
  for (const expected of ['ideas.md', 'materials/index.json']) {
    assert.ok(stamped.includes(expected), `${expected} に時刻が無い（時刻を刻んだのは ${stamped}）`);
  }
  /** 足したアイデア（I0002）の `created`。 */
  const created = (bytes) =>
    /# I0002 [^\n]*\n(?:[^\n]*\n)*?created: ([^\n]+)\n/.exec(bytes.toString('utf8'))?.[1];
  assert.match(created(files.browser.get('ideas.md')), /\+09:00$/, 'ブラウザ版はローカルのオフセットで書く');
  assert.match(created(files.native.get('ideas.md')), /Z$/, 'ネイティブ版（FsHost）は UTC で書く');
});

// ---------------------------------------------------------------------------
// 4. 橋渡しの Rust 側のテスト
// ---------------------------------------------------------------------------

test('橋渡しの Rust 側のテストが通る（規則 10・外部変更・新規作成・スレッド）', SLOW, async (t) => {
  if (!(await ready(t, { wasm: false }))) {
    return;
  }
  driver();
  const tested = runCargo(['test']);
  const summaries = [...tested.stdout.matchAll(/test result: (\w+)\. (\d+) passed; (\d+) failed/g)];
  assert.ok(summaries.length >= 6, `テストの集計が少ない:\n${tested.stdout.slice(-2000)}`);
  const passed = summaries.reduce((sum, found) => sum + Number(found[2]), 0);
  assert.ok(passed >= 30, `通ったテストが少ない（${passed} 件）`);
  for (const found of summaries) {
    assert.deepEqual([found[1], found[3]], ['ok', '0'], tested.stdout.slice(-2000));
  }
});

test('橋渡しは、ブラウザ用の wasm と同じ版の依存で組んでいる（root の Cargo.lock）', SLOW, async (t) => {
  if (!(await ready(t, { wasm: false }))) {
    return;
  }
  driver();
  const root = lockPackages(read(path.join(REPO, 'Cargo.lock')));
  const known = new Set(root.map((item) => `${item.name} ${item.version} ${item.checksum}`));
  const built = lockPackages(read(path.join(BRIDGE, 'Cargo.lock')));
  assert.ok(built.length > 50, 'bridge/Cargo.lock が小さすぎる');
  for (const item of built) {
    if (item.name === 'novnow-native-bridge') {
      continue;
    }
    assert.ok(
      known.has(`${item.name} ${item.version} ${item.checksum}`),
      `${item.name} ${item.version}: root の Cargo.lock と違う版で組んだ`,
    );
  }
  assert.ok(statSync(driver()).isFile());
});
