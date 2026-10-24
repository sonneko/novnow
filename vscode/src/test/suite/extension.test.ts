import * as assert from "node:assert/strict";
import * as vscode from "vscode";
import type { NovnowExtensionApi } from "../../extension";

suite("novnow VSCode extension", () => {
  let api!: NovnowExtensionApi;

  suiteSetup(async () => {
    const executable = process.env.NOVNOW_TEST_EXECUTABLE;
    assert.ok(executable, "NOVNOW_TEST_EXECUTABLE must point to a built novnow binary");
    await vscode.workspace
      .getConfiguration("novnow")
      .update("executablePath", executable, vscode.ConfigurationTarget.Global);
    const extension = vscode.extensions.getExtension("sonneko.novnow");
    assert.ok(extension);
    // Use the activated bundle's exports. Importing extension.ts here would create
    // a second module instance whose LSP client is never started by VSCode.
    api = (await extension.activate()) as NovnowExtensionApi;
  });

  test("an explicit executable starts the LSP server", () => {
    assert.equal(api.isLanguageServerActive(), true);
  });

  test("all four commands are registered and check delegates to the CLI", async () => {
    const commands = await vscode.commands.getCommands(true);
    for (const command of ["check", "export", "newProject", "restartServer"]) {
      assert.ok(commands.includes(`novnow.${command}`), command);
    }
    await assert.doesNotReject(Promise.resolve(vscode.commands.executeCommand("novnow.check")));
  });
});
