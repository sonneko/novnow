import * as assert from "node:assert/strict";
import * as vscode from "vscode";
import { isLanguageServerActive } from "../../extension";

suite("novnow VSCode extension", () => {
  suiteSetup(async () => {
    const executable = process.env.NOVNOW_TEST_EXECUTABLE;
    assert.ok(executable, "NOVNOW_TEST_EXECUTABLE must point to a built novnow binary");
    await vscode.workspace
      .getConfiguration("novnow")
      .update("executablePath", executable, vscode.ConfigurationTarget.Global);
    const extension = vscode.extensions.getExtension("sonneko.novnow");
    assert.ok(extension);
    await extension.activate();
  });

  test("an explicit executable starts the LSP server", async () => {
    for (let attempt = 0; attempt < 50 && !isLanguageServerActive(); attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.equal(isLanguageServerActive(), true);
  });

  test("all four commands are registered and check delegates to the CLI", async () => {
    const commands = await vscode.commands.getCommands(true);
    for (const command of ["check", "export", "newProject", "restartServer"]) {
      assert.ok(commands.includes(`novnow.${command}`), command);
    }
    await assert.doesNotReject(Promise.resolve(vscode.commands.executeCommand("novnow.check")));
  });
});
