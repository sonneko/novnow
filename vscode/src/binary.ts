import * as vscode from "vscode";
import { chmod, mkdir, rename, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { downloadBuffer, DownloadOptions } from "./download";
import { findOnPath, isExecutable } from "./executable";
import { runProcess } from "./process";
import {
  executableName,
  releaseUrl,
  safeArchiveEntry,
  targetFor,
  verifyArchiveChecksum,
} from "./release";

const INSTALL_GUIDE = "https://novnow.sonneko.com/ja/manual/install";

interface ExtensionManifest {
  version: string;
  novnow?: { artifact?: Record<string, string> };
}

function manifest(context: vscode.ExtensionContext): ExtensionManifest {
  return context.extension.packageJSON as ExtensionManifest;
}

function selectedVersion(context: vscode.ExtensionContext): string {
  const configured = vscode.workspace
    .getConfiguration("novnow")
    .get<string>("downloadVersion", "")
    .trim();
  return configured || manifest(context).version;
}

function cachedBinary(context: vscode.ExtensionContext, version: string, target: string): string {
  return join(
    context.globalStorageUri.fsPath,
    "bin",
    version,
    target,
    executableName(target),
  );
}

function downloadOptions(): DownloadOptions {
  const http = vscode.workspace.getConfiguration("http");
  const support = http.get<string>("proxySupport", "override");
  const options: DownloadOptions = {
    strictSSL: http.get<boolean>("proxyStrictSSL", true),
  };
  if (support !== "off") {
    const proxy = http.get<string>("proxy", "").trim();
    const authorization = http.get<string>("proxyAuthorization", "").trim();
    if (proxy) options.proxy = proxy;
    if (authorization) options.proxyAuthorization = authorization;
  }
  return options;
}

async function openInstallGuide(): Promise<void> {
  await vscode.env.openExternal(vscode.Uri.parse(INSTALL_GUIDE));
}

async function reportUnavailable(message: string): Promise<void> {
  const choice = await vscode.window.showErrorMessage(
    `${message} Install novnow manually or set novnow.executablePath.`,
    "Open installation guide",
  );
  if (choice) await openInstallGuide();
}

async function extractExecutable(
  archive: Buffer,
  archivePath: string,
  temporaryDirectory: string,
  target: string,
): Promise<string> {
  await writeFile(archivePath, archive, { flag: "wx" });
  const listing = await runProcess("tar", ["-tf", archivePath]);
  if (listing.code !== 0) throw new Error(`Cannot list the downloaded archive: ${listing.stderr}`);
  const names = listing.stdout.split(/\r?\n/).filter(Boolean);
  if (names.some((name) => !safeArchiveEntry(name))) {
    throw new Error("The downloaded archive contains an unsafe path");
  }
  const executable = executableName(target);
  if (!names.includes(executable)) {
    throw new Error(`The downloaded archive does not contain ${executable}`);
  }
  await mkdir(temporaryDirectory, { recursive: true });
  const extracted = await runProcess("tar", [
    "-xf",
    archivePath,
    "-C",
    temporaryDirectory,
    executable,
  ]);
  if (extracted.code !== 0) {
    throw new Error(`Cannot extract the downloaded archive: ${extracted.stderr}`);
  }
  return join(temporaryDirectory, executable);
}

async function downloadBinary(
  context: vscode.ExtensionContext,
  version: string,
  target: string,
): Promise<string> {
  const artifact = manifest(context).novnow?.artifact?.[target];
  if (!artifact) throw new Error(`No official artifact is registered for ${target}`);
  const destination = cachedBinary(context, version, target);
  const temporary = join(
    context.globalStorageUri.fsPath,
    "bin",
    `.download-${version}-${target}-${process.pid}`,
  );
  await rm(temporary, { recursive: true, force: true });
  await mkdir(temporary, { recursive: true });
  try {
    const options = downloadOptions();
    const checksums = await downloadBuffer(releaseUrl(version, "SHA256SUMS"), options);
    const archive = await downloadBuffer(releaseUrl(version, artifact), options);
    verifyArchiveChecksum(archive, checksums.toString("utf8"), artifact);

    const archivePath = join(temporary, artifact);
    const extracted = await extractExecutable(archive, archivePath, temporary, target);
    if (target !== "x86_64-pc-windows-msvc") await chmod(extracted, 0o755);
    await mkdir(dirname(destination), { recursive: true });
    if (await isExecutable(destination)) return destination;
    await rename(extracted, destination);
    return destination;
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

/**
 * Resolve in the specification's fixed order: explicit path, PATH, extension
 * cache, then (when enabled) the official release.
 */
export async function resolveBinary(context: vscode.ExtensionContext): Promise<string | undefined> {
  const configuration = vscode.workspace.getConfiguration("novnow");
  const explicit = configuration.get<string>("executablePath", "").trim();
  if (explicit) {
    const expanded = explicit.startsWith("~/")
      ? join(process.env.HOME ?? "", explicit.slice(2))
      : explicit;
    const absolute = resolve(expanded);
    if (await isExecutable(absolute)) return absolute;
  }

  const onPath = await findOnPath(process.env.PATH);
  if (onPath) return onPath;

  const version = selectedVersion(context);
  const target = targetFor(process.platform, process.arch);
  const cached = cachedBinary(context, version, target);
  if (await isExecutable(cached)) return cached;

  if (!configuration.get<boolean>("autoDownload", true)) {
    await reportUnavailable("novnow was not found and automatic download is disabled.");
    return undefined;
  }
  const failureKey = `novnow.downloadFailed.${version}.${target}`;
  if (context.globalState.get<boolean>(failureKey, false)) {
    await reportUnavailable("A previous novnow download failed; automatic retry is paused.");
    return undefined;
  }

  try {
    const downloaded = await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: `Downloading novnow ${version} (${target})`,
        cancellable: false,
      },
      async (progress) => {
        progress.report({ message: "Fetching SHA256SUMS and the nogui archive" });
        return downloadBinary(context, version, target);
      },
    );
    await context.globalState.update(failureKey, false);
    void vscode.window.showInformationMessage(`novnow ${version} is ready.`);
    return downloaded;
  } catch (error) {
    await context.globalState.update(failureKey, true);
    const detail = error instanceof Error ? error.message : String(error);
    await reportUnavailable(`novnow download failed: ${detail}`);
    return undefined;
  }
}
