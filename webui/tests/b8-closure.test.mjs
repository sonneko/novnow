// §21.5.2 #1 closure audit。gap を解消扱いにする前に各実装経路と回帰テストを照合する。
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';

const WEBUI = new URL('../', import.meta.url);
const ROOT = new URL('../../', import.meta.url);
const read = (base, path) => readFile(new URL(path, base), 'utf8');
const catalog = async () => JSON.parse(await read(ROOT, 'tests/golden/api_catalog.json'));

function operation(catalog, id) {
  const operations = Array.isArray(catalog) ? catalog : catalog.operations;
  const found = operations.find((item) => item.id === id);
  assert.ok(found, `catalog operation missing: ${id}`);
  return found;
}

test('§9.7: native project operations are reachable through the Tauri-backed Bridge', async () => {
  const [main, native, app, picker, nativeMain, projectOps, operations] = await Promise.all([
    read(WEBUI, 'src/main.ts'),
    read(WEBUI, 'src/lib/bridge/native.ts'),
    read(WEBUI, 'src/App.svelte'),
    read(WEBUI, 'src/lib/project/picker.ts'),
    read(WEBUI, 'src-tauri/src/main.rs'),
    read(ROOT, 'src/app/ops/project.rs'),
    catalog(),
  ]);
  assert.match(main, /native \? nativeBridge\(\) : wasmBridge\(\)/);
  assert.match(native, /invoke<string>\('run'/);
  assert.match(app, /store\.run\('project\.recent_list'\)/);
  assert.match(app, /store\.run\('project\.new'/);
  assert.match(native, /invoke<string>\('open'/);
  assert.match(picker, /newProject: native,[\s\S]*recentList: native/);
  assert.match(nativeMain, /fn run\(/);
  assert.match(nativeMain, /novnow_native_bridge::NativeSession/);
  assert.match(projectOps, /cfg\(target_arch = "wasm32"\)[\s\S]*no_fs/);
  assert.deepEqual(operation(operations, 'project.new').arguments.required, ['path']);
  assert.ok('title' in operation(operations, 'project.new').arguments.properties);
  assert.deepEqual(operation(operations, 'project.recent_list').arguments.properties, {});
});

test('§9.6: the native Bridge polls external changes and exposes conflicts to App', async () => {
  const [types, native, app, nativeMain, externalTests] = await Promise.all([
    read(WEBUI, 'src/lib/bridge/types.ts'),
    read(WEBUI, 'src/lib/bridge/native.ts'),
    read(WEBUI, 'src/App.svelte'),
    read(WEBUI, 'src-tauri/src/main.rs'),
    read(WEBUI, 'src-tauri/bridge/tests/external.rs'),
  ]);
  assert.match(nativeMain, /fn poll_external\(/);
  assert.match(native, /invoke<string>\('poll_external'/);
  assert.match(types, /pollExternal\?\(\): Promise<RunOutcome>/);
  assert.match(app, /setInterval\(\(\) => void pollExternal\(\), 1000\)/);
  assert.match(app, /conflicts = conflictPaths\(state\.result/);
  assert.match(externalTests, /fn an_external_edit_is_reloaded_when_clean/);
  assert.match(externalTests, /fn an_external_edit_is_a_conflict_when_dirty/);
});

test('§9.8.3: the second screen returns partial search results and applies one multi-file WorkspaceEdit', async () => {
  const [feature, firstUi, workbench, firstUiTest, editing, wasm, native, dialog, nativeTest, operations] = await Promise.all([
    read(ROOT, 'src/core/features/search.rs'),
    read(ROOT, 'src/ui/workspaces/regex_search.rs'),
    read(ROOT, 'src/app/workbench.rs'),
    read(ROOT, 'tests/stage_two.rs'),
    read(ROOT, 'src/app/ops/editing.rs'),
    read(ROOT, 'crates/novnow-wasm/src/lib.rs'),
    read(WEBUI, 'src-tauri/bridge/src/lib.rs'),
    read(WEBUI, 'src/lib/search/SearchDialog.svelte'),
    read(WEBUI, 'src-tauri/bridge/tests/project_search.rs'),
    catalog(),
  ]);
  assert.match(feature, /elapsed_ms\(\) >= 1000/);
  assert.match(feature, /out\.timed_out = true/);
  assert.match(feature, /pub fn replacement\([\s\S]*?Result<WorkspaceEdit, String>/);
  assert.match(firstUi, /search::replacement\([\s\S]*?preview\(ui, locale, bench, &edit\.changes\)/);
  assert.match(firstUi, /bench\.apply_edit\(&edit, now\)/);
  assert.match(workbench, /workspace_undo\.push\(\(before, after\)\)/);
  assert.match(firstUiTest, /fn search_regex_case_deadline_replacement_and_snapshots/);
  assert.match(editing, /pub fn search_project\([\s\S]*?for document in session\.aggregate\(\)\.documents\(\)/);
  assert.doesNotMatch(editing.slice(editing.indexOf('pub fn search_project('), editing.indexOf('/// `search.find_next`')), /timed_out|elapsed_ms/);
  assert.match(wasm, /fn project_search\([\s\S]*?search::replacement\([\s\S]*?apply_edit/);
  assert.match(native, /fn project_search_session\([\s\S]*?search::replacement\([\s\S]*?apply_edit/);
  assert.match(dialog, /session\.projectSearch\(\{ query, regex, case_sensitive: caseSensitive \}\)/);
  assert.match(dialog, /timedOut \? ' · 時間上限のため部分結果'/);
  assert.match(nativeTest, /one_multifile_workspace_edit/);
  assert.match(editing, /pub fn replace_file\([\s\S]*?let \(path, text, start\) = editing_document\(session\)/);
  assert.deepEqual(Object.keys(operation(operations, 'search.project').returns.properties).sort(), ['count', 'hits', 'query']);
  assert.deepEqual(Object.keys(operation(operations, 'search.replace_file').returns.properties).sort(), ['count', 'path', 'save']);
  const coverage = await read(WEBUI, 'src/lib/coverage.ts');
  assert.ok(!coverage.includes("section: '9.8.3'"), 'the implemented second-screen path must not remain listed as a gap');
});

test('§10.0: fixed shortcut routing resolves the requested cast, material, and outline destination', async () => {
  const [app, keys, reference, wasm, native, shell, write, castTest, operations] = await Promise.all([
    read(WEBUI, 'src/App.svelte'),
    read(WEBUI, 'src/lib/keyboard/keys.ts'),
    read(ROOT, 'src/core/features/reference.rs'),
    read(ROOT, 'crates/novnow-wasm/src/lib.rs'),
    read(WEBUI, 'src-tauri/bridge/src/lib.rs'),
    read(WEBUI, 'src/lib/shell/AppShell.svelte'),
    read(WEBUI, 'src/lib/workspaces/write/WriteWorkspace.svelte'),
    read(WEBUI, 'src-tauri/bridge/tests/shortcut_cast.rs'),
    catalog(),
  ]);
  const route = app.slice(app.indexOf('async function landWorkspaceShortcut'), app.indexOf('async function setWorkspace'));
  assert.match(route, /reference\.pane/);
  assert.match(route, /ui\.shortcut\.cast/);
  assert.match(wasm, /metadata::inherited\(&rows, id, \"mode\"\)/);
  assert.match(native, /metadata::inherited\(&rows, id, \"mode\"\)/);
  assert.match(route, /castFocusIndex = typeof index === 'number' \? index : null/);
  assert.match(route, /materialFocusId = first\?\.id/);
  assert.match(route, /plot\.get', \{ level: 'all' \}/);
  assert.match(route, /row\.state !== 'Fixed'/);
  assert.match(route, /writeFocusCaret = 0/);
  assert.match(shell, /focusCaret=\{writeFocusCaret\}/);
  assert.match(write, /editor\.setCaret\(focusCaret\)/);
  assert.match(castTest, /resolves_the_first_character_from_the_scene_mode_field/);
  assert.match(keys, /return digit \? \{ workspace: KEEP_WORKSPACES\[digit\] \} : null/);
  assert.ok(reference.includes('Character(usize)'));
  assert.ok(reference.includes('OpenMaterial(String)'));
  assert.ok(reference.includes('AddMaterial(String)'));
  assert.deepEqual(Object.keys(operation(operations, 'session.workspace_switch').arguments.properties), ['workspace']);
  assert.deepEqual(Object.keys(operation(operations, 'session.command').arguments.properties), ['name']);
});

test('実装を確認したSPEC_GAPSは空の一覧になり、PENDINGは別管理のまま0件', async () => {
  const coverage = await read(WEBUI, 'src/lib/coverage.ts');
  assert.ok(coverage.includes('export const SPEC_GAPS: readonly SpecGap[] = [];'));
  assert.ok(coverage.includes('export const PENDING: readonly string[] = [];'));
});
