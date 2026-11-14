import test from 'node:test';
import assert from 'node:assert/strict';
import searchIndex, {
  buildIndex, decodeEntities, entryFor, INDEX_DIRECTORY, LANGUAGES, MAX_TEXT_CHARS,
  pageHeadings, pageLanguage, pageTitle, pageUrl, visibleText,
} from './search-index.mjs';

const page = (lang, body, extra = '') =>
  `<!DOCTYPE html><html lang="${lang}"><head><title>題 | NovNow</title>${extra}</head>`
  + `<body><header><nav>毎ページ同じ導線</nav></header><main>${body}</main>`
  + `<footer>© 2026</footer><script>console.log("本文ではない")</script></body></html>`;

test('本文は main の中だけで、導線・スクリプト・スタイル・図は入らない', () => {
  const html = page('ja', '<h1>保存</h1><p>一時ファイルと rename。</p><style>p{color:red}</style>'
    + '<svg><title>図</title></svg><nav>目次</nav>');
  const text = visibleText(html);
  assert.match(text, /^保存 一時ファイルと rename。$/);
  assert.ok(!text.includes('毎ページ同じ導線'));
  assert.ok(!text.includes('console.log'));
  assert.ok(!text.includes('color:red'));
  assert.ok(!text.includes('図'));
});

test('main が無いページは body から取り、実体参照は戻す', () => {
  const html = '<html lang="en"><body><p>Atomic &amp; safe&nbsp;writes</p><script>x()</script></body></html>';
  assert.equal(visibleText(html), 'Atomic & safe writes');
  assert.equal(decodeEntities('&#65;&#x42;&lt;tag&gt;'), 'AB<tag>');
});

test('題は title から取り、無ければ h1、サイト名の接尾辞は落とす', () => {
  assert.equal(pageTitle(page('ja', '<h1>見出し</h1>')), '題');
  assert.equal(pageTitle('<html><body><h1>診断一覧</h1></body></html>'), '診断一覧');
  assert.equal(pageTitle('<html><head><title>API リファレンス</title></head></html>'), 'API リファレンス');
});

test('節の見出しは h2 から集める', () => {
  const html = page('ja', '<h2>8.3 原子的な書き込み</h2><p>本文</p><h2><code>project.toml</code> の形式</h2>');
  assert.deepEqual(pageHeadings(html), ['8.3 原子的な書き込み', 'project.toml の形式']);
});

test('言語は html lang から決める（ディレクトリ名に依存しない）', () => {
  assert.equal(pageLanguage('<html lang="ja-JP">'), 'ja');
  assert.equal(pageLanguage("<html lang='en'>"), 'en');
  assert.equal(pageLanguage('<html>'), '');
});

test('公開 URL は base を付け、index.html はディレクトリの形にする', () => {
  assert.equal(pageUrl('ja/manual/start/index.html', '/novnow'), '/novnow/ja/manual/start/');
  assert.equal(pageUrl('guide/start.html', '/novnow'), '/novnow/guide/start.html');
  assert.equal(pageUrl('en/index.html', '/novnow/'), '/novnow/en/');
});

test('検索のページ自身・ルートのリダイレクト・アプリは索引に入れない', () => {
  assert.equal(entryFor('ja/search/index.html', page('ja', '<p>検索</p>'), ''), null);
  assert.equal(entryFor('en/search/index.html', page('en', '<p>Search</p>'), ''), null);
  assert.equal(entryFor('index.html', '<html lang="en"><body>redirect</body></html>', '/novnow'), null);
  assert.equal(entryFor('app/index.html', page('ja', '<p>アプリ</p>'), ''), null);
  assert.equal(entryFor('nightly-app/index.html', page('ja', '<p>アプリ</p>'), ''), null);
  assert.equal(entryFor('_astro/page.html', page('ja', '<p>資産</p>'), ''), null);
});

test('索引に無い言語と空のページは入れない', () => {
  assert.equal(entryFor('fr/index.html', page('fr', '<p>bonjour</p>'), '/novnow'), null);
  assert.equal(entryFor('ja/empty/index.html', '<html lang="ja"><body></body></html>', '/novnow'), null);
});

test('索引は言語別に分かれ、URL の順に並ぶ', () => {
  const index = buildIndex([
    { file: 'en/manual/start/index.html', html: page('en', '<p>Save the manuscript atomically.</p>') },
    { file: 'ja/manual/start/index.html', html: page('ja', '<p>本文を原子的に保存する。</p>') },
    { file: 'ja/api/index.html', html: page('ja', '<p>操作の一覧。</p>') },
    { file: 'ja/search/index.html', html: page('ja', '<p>検索</p>') },
  ], '/novnow');
  assert.deepEqual(Object.keys(index), LANGUAGES);
  assert.deepEqual(index.ja.map(entry => entry.url), ['/novnow/ja/api/', '/novnow/ja/manual/start/']);
  assert.deepEqual(index.en.map(entry => entry.url), ['/novnow/en/manual/start/']);
  assert.deepEqual(index.ja[0].lang, 'ja');
  assert.ok(index.ja[1].text.includes('原子的に保存する'));
});

test('長いページの本文は上限で切る（索引が肥大しない）', () => {
  const long = 'あ'.repeat(MAX_TEXT_CHARS + 5000);
  const entry = entryFor('ja/specifications/ui/index.html', page('ja', `<p>${long}</p>`), '/novnow');
  assert.equal(entry.text.length, MAX_TEXT_CHARS);
});

test('エントリの形は固定（url / lang / title / headings / text）', () => {
  const entry = entryFor('ja/manual/index.html', page('ja', '<h2>段階</h2><p>本文</p>'), '/novnow');
  assert.deepEqual(Object.keys(entry).sort(), ['headings', 'lang', 'text', 'title', 'url']);
  assert.deepEqual(entry.headings, ['段階']);
});

test('統合は設定確定とビルド完了の両方のフックを持つ', () => {
  const integration = searchIndex();
  assert.equal(integration.name, 'novnow-search-index');
  assert.deepEqual(Object.keys(integration.hooks).sort(), ['astro:build:done', 'astro:config:done']);
  assert.equal(INDEX_DIRECTORY, 'search-index');
});
