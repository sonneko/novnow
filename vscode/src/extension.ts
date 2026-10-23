import * as vscode from "vscode";
import { stat } from "node:fs/promises";
import { join } from "node:path";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";
import { resolveBinary } from "./binary";
import { NovnowCommands } from "./commands";

let client: LanguageClient | undefined;
let starting: Promise<LanguageClient | undefined> | undefined;

async function projectFolder(): Promise<vscode.WorkspaceFolder | undefined> {
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    try {
      if ((await stat(join(folder.uri.fsPath, "project.json"))).isFile()) return folder;
    } catch {
      // This workspace folder is not a novnow project; do not create anything.
    }
  }
  return undefined;
}

async function startServer(context: vscode.ExtensionContext): Promise<LanguageClient | undefined> {
  if (client) return client;
  if (starting) return starting;
  starting = (async () => {
    const folder = await projectFolder();
    if (!folder) return undefined;
    const executable = await resolveBinary(context);
    if (!executable) return undefined;

    const extraArgs = vscode.workspace
      .getConfiguration("novnow")
      .get<string[]>("extraArgs", []);
    const serverOptions: ServerOptions = {
      command: executable,
      args: [...extraArgs, "lsp", folder.uri.fsPath],
    };
    const clientOptions: LanguageClientOptions = {
      documentSelector: [
        {
          scheme: "file",
          language: "novnow",
          pattern: `${folder.uri.fsPath.replaceAll("\\", "/")}/**/*`,
        },
      ],
      workspaceFolder: folder,
      outputChannel: vscode.window.createOutputChannel("novnow Language Server"),
    };
    const created = new LanguageClient(
      "novnow",
      "novnow Language Server",
      serverOptions,
      clientOptions,
    );
    await created.start();
    client = created;
    context.subscriptions.push(created);
    return created;
  })();
  try {
    return await starting;
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    void vscode.window.showErrorMessage(`novnow language server could not start: ${detail}`);
    return undefined;
  } finally {
    starting = undefined;
  }
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const output = vscode.window.createOutputChannel("novnow");
  context.subscriptions.push(output);
  const commands = new NovnowCommands(
    output,
    () => resolveBinary(context),
    async () => (await projectFolder())?.uri.fsPath,
  );
  context.subscriptions.push(
    vscode.commands.registerCommand("novnow.check", () => commands.check()),
    vscode.commands.registerCommand("novnow.export", () => commands.exportText()),
    vscode.commands.registerCommand("novnow.newProject", () => commands.newProject()),
    vscode.commands.registerCommand("novnow.restartServer", async () => {
      if (client) {
        const previous = client;
        client = undefined;
        await previous.stop();
      }
      await startServer(context);
    }),
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (!event.affectsConfiguration("novnow")) return;
      if (client) {
        const previous = client;
        client = undefined;
        await previous.stop();
      }
      await startServer(context);
    }),
  );
  await startServer(context);
}

export function isLanguageServerActive(): boolean {
  return client !== undefined;
}

export async function deactivate(): Promise<void> {
  if (client) {
    const previous = client;
    client = undefined;
    await previous.stop();
  }
}
