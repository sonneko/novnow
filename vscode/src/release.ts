import { createHash } from "node:crypto";

export const RELEASE_ROOT = "https://github.com/sonneko/novnow/releases/download";

export function targetFor(platform: string, arch: string): string {
  const key = `${platform}:${arch}`;
  const targets: Record<string, string> = {
    "linux:x64": "x86_64-unknown-linux-gnu",
    "linux:arm64": "aarch64-unknown-linux-gnu",
    "darwin:x64": "x86_64-apple-darwin",
    "darwin:arm64": "aarch64-apple-darwin",
    "win32:x64": "x86_64-pc-windows-msvc",
  };
  const target = targets[key];
  if (!target) {
    throw new Error(`novnow does not provide a nogui binary for ${platform}/${arch}`);
  }
  return target;
}

export function executableName(target: string): string {
  return target === "x86_64-pc-windows-msvc" ? "novnow.exe" : "novnow";
}

export function releaseUrl(version: string, artifact: string): string {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Invalid novnow release version: ${version}`);
  }
  return `${RELEASE_ROOT}/v${version}/${artifact}`;
}

export function parseChecksums(text: string): Map<string, string> {
  const checksums = new Map<string, string>();
  for (const line of text.split(/\r?\n/)) {
    if (!line || line.startsWith("#")) continue;
    const match = /^([0-9a-f]{64})  (.+)$/.exec(line);
    if (!match) throw new Error("The release SHA256SUMS contains an invalid line");
    const [, hash, name] = match;
    if (hash === undefined || name === undefined) continue;
    if (checksums.has(name)) throw new Error(`Duplicate SHA256SUMS entry: ${name}`);
    checksums.set(name, hash.toLowerCase());
  }
  return checksums;
}

export function verifyArchiveChecksum(
  archive: Buffer,
  checksumDocument: string,
  artifact: string,
): void {
  const expected = parseChecksums(checksumDocument).get(artifact);
  if (!expected) throw new Error(`SHA256SUMS has no entry for ${artifact}`);
  const actual = createHash("sha256").update(archive).digest("hex");
  if (actual !== expected) {
    throw new Error(`SHA-256 mismatch for ${artifact}; the partial download was discarded`);
  }
}

export function safeArchiveEntry(name: string): boolean {
  const normalized = name.replaceAll("\\", "/");
  return (
    normalized.length > 0 &&
    !normalized.startsWith("/") &&
    !/^[A-Za-z]:\//.test(normalized) &&
    !normalized.split("/").includes("..")
  );
}
