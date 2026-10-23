import * as esbuild from "esbuild";

const watch = process.argv.includes("--watch");
const unit = process.argv.includes("--unit");
const options = {
  entryPoints: ["src/extension.ts"],
  outfile: "dist/extension.js",
  bundle: true,
  external: ["vscode"],
  format: "cjs",
  platform: "node",
  target: "node18",
  sourcemap: true,
  minify: !watch,
  logLevel: "info",
};

if (watch) {
  const context = await esbuild.context(options);
  await context.watch();
} else if (unit) {
  await esbuild.build({
    entryPoints: ["src/release.ts", "src/executable.ts", "src/process.ts"],
    outdir: "out/unit",
    bundle: true,
    format: "cjs",
    platform: "node",
    target: "node18",
    sourcemap: false,
    logLevel: "info",
  });
} else {
  await esbuild.build(options);
}
