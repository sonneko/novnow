import * as path from "node:path";
import { downloadAndUnzipVSCode, runTests } from "@vscode/test-electron";
import * as fs from "node:fs";
import { execFileSync } from "node:child_process";

async function main(): Promise<void> {
  const extensionDevelopmentPath = path.resolve(__dirname, "../..");
  const extensionTestsPath = path.resolve(__dirname, "./suite/index");
  const sampleProject = path.resolve(extensionDevelopmentPath, "../examples/sample-novel");
  const executable = process.env.NOVNOW_TEST_EXECUTABLE;
  if (!executable) throw new Error("NOVNOW_TEST_EXECUTABLE is required");
  process.env.PATH = `${path.dirname(executable)}${path.delimiter}${process.env.PATH ?? ""}`;
  let vscodeExecutablePath = await downloadAndUnzipVSCode();
  if (process.platform === "darwin" && !fs.existsSync(vscodeExecutablePath)) {
    // New VSCode bundles can rename CFBundleExecutable. Do not assume Electron.
    const directory = path.dirname(path.dirname(path.dirname(path.dirname(vscodeExecutablePath))));
    const bundle = fs.readdirSync(directory).find(name => name.endsWith(".app"));
    if (!bundle) throw new Error(`VSCode app bundle missing in ${directory}: ${fs.readdirSync(directory)}`);
    const contents = path.join(directory, bundle, "Contents");
    const executableName = execFileSync("/usr/libexec/PlistBuddy", ["-c", "Print :CFBundleExecutable", path.join(contents, "Info.plist")], { encoding: "utf8" }).trim();
    vscodeExecutablePath = path.join(contents, "MacOS", executableName);
    if (!fs.existsSync(vscodeExecutablePath)) throw new Error(`VSCode bundle executable missing: ${vscodeExecutablePath}`);
  }
  await runTests({
    vscodeExecutablePath,
    extensionDevelopmentPath,
    extensionTestsPath,
    launchArgs: [sampleProject, "--disable-extensions"],
  });
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
