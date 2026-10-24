const test = require("node:test");
const assert = require("node:assert/strict");
const { runProcess } = require("../out/unit/process.js");

test("CLI arguments are passed without shell interpretation", async () => {
  const script = "console.log(process.argv.slice(1).join('|'));process.exit(7)";
  const result = await runProcess(process.execPath, ["-e", script, "a;touch nope", "two words"]);
  assert.equal(result.code, 7);
  assert.equal(result.stdout.trim(), "a;touch nope|two words");
});
