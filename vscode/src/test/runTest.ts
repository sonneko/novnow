import * as path from "node:path";
import { runTests } from "@vscode/test-electron";

async function main(): Promise<void> {
  const extensionDevelopmentPath = path.resolve(__dirname, "../..");
  const extensionTestsPath = path.resolve(__dirname, "./suite/index");
  const sampleProject = path.resolve(extensionDevelopmentPath, "../examples/sample-novel");
  const executable = process.env.NOVNOW_TEST_EXECUTABLE;
  if (!executable) throw new Error("NOVNOW_TEST_EXECUTABLE is required");
  process.env.PATH = `${path.dirname(executable)}${path.delimiter}${process.env.PATH ?? ""}`;
  await runTests({
    extensionDevelopmentPath,
    extensionTestsPath,
    launchArgs: [sampleProject, "--disable-extensions"],
  });
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
