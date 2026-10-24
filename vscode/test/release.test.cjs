const test = require("node:test");
const assert = require("node:assert/strict");
const { createHash } = require("node:crypto");
const {
  targetFor,
  executableName,
  releaseUrl,
  parseChecksums,
  verifyArchiveChecksum,
  safeArchiveEntry,
} = require("../out/unit/release.js");

test("the five runner platforms map to the five release targets", () => {
  assert.equal(targetFor("linux", "x64"), "x86_64-unknown-linux-gnu");
  assert.equal(targetFor("linux", "arm64"), "aarch64-unknown-linux-gnu");
  assert.equal(targetFor("darwin", "x64"), "x86_64-apple-darwin");
  assert.equal(targetFor("darwin", "arm64"), "aarch64-apple-darwin");
  assert.equal(targetFor("win32", "x64"), "x86_64-pc-windows-msvc");
  assert.throws(() => targetFor("win32", "arm64"), /does not provide/);
  assert.equal(executableName("x86_64-pc-windows-msvc"), "novnow.exe");
});

test("release URLs are versioned and official", () => {
  assert.equal(
    releaseUrl("0.1.0", "SHA256SUMS"),
    "https://github.com/sonneko/novnow/releases/download/v0.1.0/SHA256SUMS",
  );
  assert.throws(() => releaseUrl("../other", "SHA256SUMS"), /Invalid/);
});

test("checksum mismatch rejects the archive", () => {
  const archive = Buffer.from("actual archive");
  const good = createHash("sha256").update(archive).digest("hex");
  const sums = `${good}  novnow-test.zip\n`;
  assert.equal(parseChecksums(sums).get("novnow-test.zip"), good);
  assert.doesNotThrow(() => verifyArchiveChecksum(archive, sums, "novnow-test.zip"));
  assert.throws(
    () => verifyArchiveChecksum(Buffer.from("tampered"), sums, "novnow-test.zip"),
    /SHA-256 mismatch/,
  );
  assert.throws(() => verifyArchiveChecksum(archive, "", "novnow-test.zip"), /no entry/);
});

test("archive extraction rejects traversal and absolute paths", () => {
  assert.equal(safeArchiveEntry("novnow"), true);
  assert.equal(safeArchiveEntry("novnow.app/Contents/MacOS/novnow"), true);
  assert.equal(safeArchiveEntry("../outside"), false);
  assert.equal(safeArchiveEntry("folder/../../outside"), false);
  assert.equal(safeArchiveEntry("/absolute"), false);
  assert.equal(safeArchiveEntry("C:/absolute"), false);
});
