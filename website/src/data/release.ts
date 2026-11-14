// Published version: intentionally independent of development Cargo.toml.
// Update only after verifying the corresponding public release and assets.
export const version = "0.1.1";
export const releaseUrl = `https://github.com/sonneko/novnow/releases/tag/v${version}`;
export const assetUrl = (name: string) => `https://github.com/sonneko/novnow/releases/download/v${version}/${name}`;
export const binaryTargets = [
  ["Windows · x64", "x86_64-pc-windows-msvc", ".zip"],
  ["macOS · Apple Silicon", "aarch64-apple-darwin", ".tar.gz"],
  ["macOS · Intel", "x86_64-apple-darwin", ".tar.gz"],
  ["Linux · x64", "x86_64-unknown-linux-gnu", ".tar.gz"],
  ["Linux · ARM64", "aarch64-unknown-linux-gnu", ".tar.gz"],
] as const;
