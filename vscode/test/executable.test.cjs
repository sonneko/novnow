const test = require("node:test");
const assert = require("node:assert/strict");
const { delimiter, join } = require("node:path");
const { pathCandidates } = require("../out/unit/executable.js");

test("PATH search is deterministic and uses novnow only", () => {
  const pathValue = ["/one", "/two"].join(delimiter);
  assert.deepEqual(pathCandidates(pathValue, "linux"), [
    join("/one", "novnow"),
    join("/two", "novnow"),
  ]);
  const windows = pathCandidates("C:\\one", "win32", ".EXE;.BAT");
  assert.equal(windows.length, 2);
  assert.match(windows[0], /novnow\.exe$/i);
});
