import * as vscode from "vscode";
import { join } from "node:path";
import { runProcess } from "./process";

export class NovnowCommands {
  constructor(
    private readonly output: vscode.OutputChannel,
    private readonly binary: () => Promise<string | undefined>,
    private readonly projectRoot: () => Promise<string | undefined>,
  ) {}

  private async run(args: string[]): Promise<number | undefined> {
    const executable = await this.binary();
    if (!executable) return undefined;
    this.output.appendLine(`$ ${executable} ${args.join(" ")}`);
    try {
      const result = await runProcess(executable, args);
      if (result.stdout) this.output.append(result.stdout);
      if (result.stderr) this.output.append(result.stderr);
      return result.code;
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      void vscode.window.showErrorMessage(`Could not run novnow: ${detail}`);
      return undefined;
    }
  }

  async check(): Promise<void> {
    const root = await this.projectRoot();
    if (!root) {
      void vscode.window.showErrorMessage("Open a novnow project folder first.");
      return;
    }
    this.output.show(true);
    const code = await this.run(["check", root]);
    if (code === 0) void vscode.window.showInformationMessage("novnow: project check passed.");
    if (code === 5) void vscode.window.showWarningMessage("novnow: the project has error diagnostics.");
    if (code !== undefined && code !== 0 && code !== 5) {
      void vscode.window.showErrorMessage(`novnow check exited with code ${code}.`);
    }
  }

  async exportText(): Promise<void> {
    const root = await this.projectRoot();
    if (!root) {
      void vscode.window.showErrorMessage("Open a novnow project folder first.");
      return;
    }
    const destination = await vscode.window.showSaveDialog({
      defaultUri: vscode.Uri.file(join(root, "manuscript.txt")),
      filters: { Text: ["txt"], Markdown: ["md"] },
      title: "Export text through novnow",
    });
    if (!destination) return;
    const code = await this.run(["export", root, "--out", destination.fsPath]);
    if (code === 0) {
      void vscode.window.showInformationMessage(`Exported: ${destination.fsPath}`);
    } else if (code !== undefined) {
      void vscode.window.showErrorMessage(`novnow export exited with code ${code}.`);
    }
  }

  async newProject(): Promise<void> {
    const parents = await vscode.window.showOpenDialog({
      canSelectFiles: false,
      canSelectFolders: true,
      canSelectMany: false,
      openLabel: "Select parent folder",
      title: "Select the parent for a new novnow project",
    });
    const parent = parents?.[0];
    if (!parent) return;
    const name = await vscode.window.showInputBox({
      prompt: "New project folder name",
      validateInput: (value) =>
        !value.trim() || value.includes("/") || value.includes("\\") || value === "." || value === ".."
          ? "Enter one folder name, not a path"
          : undefined,
    });
    if (!name) return;
    const destination = join(parent.fsPath, name.trim());
    const code = await this.run(["new", destination]);
    if (code === 0) {
      void vscode.window.showInformationMessage(`Created novnow project: ${destination}`);
    } else if (code !== undefined) {
      void vscode.window.showErrorMessage(`novnow new exited with code ${code}.`);
    }
  }
}
