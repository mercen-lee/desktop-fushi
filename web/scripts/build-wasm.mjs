import { existsSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const here = dirname(fileURLToPath(import.meta.url));
const webRoot = resolve(here, "..");
const repoRoot = resolve(webRoot, "..");
const outDir = resolve(webRoot, "src", "generated", "wasm");
const wasmPath = resolve(
  repoRoot,
  "target",
  "wasm32-unknown-unknown",
  "release",
  "desktop_fushi.wasm",
);

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? repoRoot,
    env: {
      ...process.env,
      RUSTFLAGS: [
        process.env.RUSTFLAGS,
        "--cfg=web_sys_unstable_apis",
      ]
        .filter(Boolean)
        .join(" "),
    },
    stdio: "inherit",
    shell: process.platform === "win32",
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

if (existsSync(outDir)) {
  rmSync(outDir, { recursive: true, force: true });
}

run("rustup", ["target", "add", "wasm32-unknown-unknown"]);
run("cargo", [
  "build",
  "--release",
  "--target",
  "wasm32-unknown-unknown",
  "--features",
  "web",
  "--lib",
]);
run("wasm-bindgen", [
  wasmPath,
  "--target",
  "web",
  "--out-dir",
  outDir,
  "--out-name",
  "desktop_fushi",
]);
