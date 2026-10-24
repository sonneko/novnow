import { constants } from "node:fs";
import { access, stat } from "node:fs/promises";
import { delimiter, join } from "node:path";

export async function isExecutable(path: string, platform = process.platform): Promise<boolean> {
  try {
    const info = await stat(path);
    if (!info.isFile()) return false;
    await access(path, platform === "win32" ? constants.F_OK : constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

export function pathCandidates(
  pathValue: string | undefined,
  platform = process.platform,
  pathExtension = process.env.PATHEXT,
): string[] {
  const names = platform === "win32"
    ? (pathExtension ?? ".COM;.EXE;.BAT;.CMD").split(";")
    : [""];
  const separator = platform === "win32" ? ";" : delimiter;
  const directories = (pathValue ?? "").split(separator).filter(Boolean);
  return directories.flatMap((directory) =>
    names.map((extension) => join(directory, `novnow${extension.toLowerCase()}`)),
  );
}

export async function findOnPath(
  pathValue: string | undefined,
  platform = process.platform,
  pathExtension?: string,
): Promise<string | undefined> {
  for (const candidate of pathCandidates(pathValue, platform, pathExtension)) {
    if (await isExecutable(candidate, platform)) return candidate;
  }
  return undefined;
}
