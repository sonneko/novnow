// §21.5.2 #1 の closure audit。SPEC_GAPS を消すのではなく、実装形態と各経路を実物で照合する。
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

test('§9.7: native project ops exist, but the actual WebUI entry still chooses wasmBridge', async () => {
  const [main, app, picker, nativeReadme, nativeMain, projectOps, operations] = await Promise.all([
    read(WEBUI, 'src/main.ts'),
    read(WEBUI, 'src/App.svelte'),
    read(WEBUI, 'src/lib/project/picker.ts'),
    read(WEBUI, 'src-tauri/README.md'),
    read(WEBUI, 'src-tauri/src/main.rs'),
    read(ROOT, 'src/app/ops/project.rs'),
    catalog(),
  ]);
  assert.match(main, /import \{ wasmBridge \} from '\.\/lib\/bridge\/wasm'/);
  assert.match(main, /const bridge = wasmBridge\(\)/);
  assert.match(app, /'__TAURI__' in window \? 'native' : 'browser'/);
  assert.match(picker, /newProject: native,[\s\S]*recentList: native/);
  assert.match(nativeReadme, /画面はまだ `wasmBridge\(\)` を固定で使う/);
  assert.match(nativeMain, /fn run\(/);
  assert.match(nativeMain, /novnow_native_bridge::NativeSession/);
  assert.match(projectOps, /cfg\(target_arch = "wasm32"\)[\s\S]*no_fs/);
  assert.deepEqual(operation(operations, 'project.new').arguments.required, ['path']);
  assert.ok('title' in operation(operations, 'project.new').arguments.properties);
  assert.deepEqual(operation(operations, 'project.recent_list').arguments.properties, {});
});

test('§9.6: native external polling and its behavioral tests exist, but no WebUI Bridge method calls it', async () => {
  const [types, main, app, nativeReadme, nativeMain, externalTests, coverage] = await Promise.all([
    read(WEBUI, 'src/lib/bridge/types.ts'),
    read(WEBUI, 'src/main.ts'),
    read(WEBUI, 'src/App.svelte'),
    read(WEBUI, 'src-tauri/README.md'),
    read(WEBUI, 'src-tauri/src/main.rs'),
    read(WEBUI, 'src-tauri/bridge/tests/external.rs'),
    read(WEBUI, 'src/lib/coverage.ts'),
  ]);
  assert.match(nativeMain, /fn poll_external\(/);
  assert.match(nativeReadme, /画面が \*\*1 秒ごと\*\*に呼ぶ/);
  assert.match(externalTests, /fn an_external_edit_is_reloaded_when_clean/);
  assert.match(externalTests, /fn an_external_edit_is_a_conflict_when_dirty/);
  assert.doesNotMatch(types, /pollExternal|poll_external/);
  assert.match(main, /const bridge = wasmBridge\(\)/);
  assert.doesNotMatch(app, /pollExternal|poll_external/);
  assert.match(coverage, /Tauri native の poll_external \/ 衝突処理.*wasmBridge 固定/s);
});

test('§9.8.3: timeout and multi-file WorkspaceEdit exist in the first UI/core, not in the second screen API', async () => {
  const [feature, firstUi, workbench, firstUiTest, editing, coverage, operations] = await Promise.all([
    read(ROOT, 'src/core/features/search.rs'),
    read(ROOT, 'src/ui/workspaces/regex_search.rs'),
    read(ROOT, 'src/app/workbench.rs'),
    read(ROOT, 'tests/stage_two.rs'),
    read(ROOT, 'src/app/ops/editing.rs'),
    read(WEBUI, 'src/lib/coverage.ts'),
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
  assert.match(editing, /pub fn replace_file\([\s\S]*?let \(path, text, start\) = editing_document\(session\)/);
  assert.deepEqual(Object.keys(operation(operations, 'search.project').returns.properties).sort(), ['count', 'hits', 'query']);
  assert.deepEqual(Object.keys(operation(operations, 'search.replace_file').returns.properties).sort(), ['count', 'path', 'save']);
  assert.match(coverage, /第1画面の search\.run は1秒 timeout\/partial hits を実装/);
});

test('§10.0: the fixed-shortcut branch switches only workspace; B8 reference Nav is a different route', async () => {
  const [app, keys, reference, coverage, operations] = await Promise.all([
    read(WEBUI, 'src/App.svelte'),
    read(WEBUI, 'src/lib/keyboard/keys.ts'),
    read(ROOT, 'src/core/features/reference.rs'),
    read(WEBUI, 'src/lib/coverage.ts'),
    catalog(),
  ]);
  const at = app.indexOf("const keep = keepPayload(found)");
  const next = app.indexOf("// `Command` の名前は `session.command` が受ける", at);
  assert.ok(at >= 0 && next > at, 'fixed-switch route is present');
  const fixedRoute = app.slice(at, next);
  assert.match(fixedRoute, /workspace: String\(keep\.workspace\)/);
  assert.doesNotMatch(fixedRoute, /reference\.pane|castFocusIndex|materialFocusId/);
  assert.match(keys, /return digit \? \{ workspace: KEEP_WORKSPACES\[digit\] \} : null/);
  assert.ok(reference.includes('Character(usize)'));
  assert.ok(reference.includes('OpenMaterial(String)'));
  assert.ok(reference.includes('AddMaterial(String)'));
  assert.deepEqual(Object.keys(operation(operations, 'session.workspace_switch').arguments.properties), ['workspace']);
  assert.deepEqual(Object.keys(operation(operations, 'session.command').arguments.properties), ['name']);
  assert.match(coverage, /B8 の参照Navクリック経路は別経路/);
});

test('監査後も SPEC_GAPS の4項目を「提供済み」に移していない', async () => {
  const coverage = await read(WEBUI, 'src/lib/coverage.ts');
  const sections = [...coverage.matchAll(/\{ section: '(9\.7|9\.6|9\.8\.3|10\.0)'/g)].map((match) => match[1]);
  assert.deepEqual(sections.sort(), ['10.0', '9.6', '9.7', '9.8.3']);
});
