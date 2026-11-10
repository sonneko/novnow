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

  test("an explicit executable starts the LSP server", async () => {
    // Changing executablePath can restart an extension that VSCode already
    // activated while opening the sample workspace. Wait for that restart.
    for (let attempt = 0; attempt < 50 && !api.isLanguageServerActive(); attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.equal(api.isLanguageServerActive(), true);
  });

  test("the language client exposes semantic tokens with registered theme scopes", async () => {
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder);
    const uri = vscode.Uri.joinPath(folder.uri, "contents", "01-01.cnov");
    const document = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(document);
    assert.equal(vscode.workspace.getConfiguration("editor", document).get("semanticHighlighting.enabled"), true);
    const legend = await vscode.commands.executeCommand<vscode.SemanticTokensLegend>(
      "vscode.provideDocumentSemanticTokensLegend", uri,
    );
    assert.ok(legend);
    assert.ok(legend.tokenTypes.includes("ref-word"));
    assert.ok(legend.tokenTypes.every(type => !type.includes(".")));
    const tokens = await vscode.commands.executeCommand<vscode.SemanticTokens>(
      "vscode.provideDocumentSemanticTokens", uri,
    );
    assert.ok(tokens && tokens.data.length > 0);
    assert.equal(tokens.data.length % 5, 0);
  });

  test("all four commands are registered and check delegates to the CLI", async () => {
    const commands = await vscode.commands.getCommands(true);
    for (const command of ["check", "export", "newProject", "restartServer"]) {
      assert.ok(commands.includes(`novnow.${command}`), command);
    }
    await assert.doesNotReject(Promise.resolve(vscode.commands.executeCommand("novnow.check")));
  });
});
