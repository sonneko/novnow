const assert = require('node:assert/strict');
const test = require('node:test');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '../..');
const manifest = require('../package.json');
const grammar = require('../syntaxes/novnow.tmLanguage.json');

test('every LSP token has a valid theme scope, enabled only for novnow', () => {
  const core = fs.readFileSync(path.join(root, 'src/core/features/semantic_tokens.rs'), 'utf8');
  const kinds = [...core.matchAll(/Self::\w+ => "([\w.]+)"/g)].map(m => m[1].replaceAll('.', '-'));
  const contribution = manifest.contributes;
  assert.equal(kinds.length, 21);
  assert.deepEqual(contribution.configurationDefaults, {
    '[novnow]': { 'editor.semanticHighlighting.enabled': true },
  });
  const scopes = contribution.semanticTokenScopes.find(s => s.language === 'novnow').scopes;
  assert.deepEqual(Object.keys(scopes).sort(), kinds.sort());
  const declared = new Set(['comment', ...contribution.semanticTokenTypes.map(t => t.id)]);
  for (const kind of kinds) {
    assert.match(kind, /^[\w-]+$/);
    assert.ok(declared.has(kind), kind);
    assert.ok(scopes[kind].length > 0, kind);
  }
});

test('TextMate never guesses the dictionary-dependent end of an apostrophe reference', () => {
  const patterns = grammar.patterns.map(p => new RegExp(p.match, 'gu'));
  for (const text of ["'星図が出てきた。", "'{星図}を読む。", "'星図:通常を読む。", "'未知の名前を読む。"] ) {
    const matches = patterns.flatMap(re => [...text.matchAll(re)]);
    assert.deepEqual(matches.map(m => m[0]), [text.startsWith("'{") ? "'{" : "'"]);
  }
});
