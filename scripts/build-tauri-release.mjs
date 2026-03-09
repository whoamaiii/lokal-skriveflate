import { spawn } from "node:child_process";
import { copyFile, mkdir, readdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const tauriRoot = path.join(repoRoot, "src-tauri");
const stagedLibDir = path.join(
  tauriRoot,
  "resources",
  "embedded-runtime",
  "staged",
  "lib",
);
const appBundleLibDir = path.join(
  tauriRoot,
  "target",
  "release",
  "bundle",
  "macos",
  "Lokal Skriveflate.app",
  "Contents",
  "lib",
);

async function pathExists(candidate) {
  try {
    await stat(candidate);
    return true;
  } catch {
    return false;
  }
}

async function copyStagedDylibsIntoBundle() {
  if (!(await pathExists(stagedLibDir))) {
    console.log("No staged llama.cpp dylibs found; skipping bundle lib sync.");
    return;
  }

  if (!(await pathExists(path.dirname(appBundleLibDir)))) {
    console.log("No macOS app bundle found yet; skipping bundle lib sync.");
    return;
  }

  await mkdir(appBundleLibDir, { recursive: true });
  const entries = await readdir(stagedLibDir);
  for (const entry of entries) {
    if (!entry.endsWith(".dylib")) {
      continue;
    }

    await copyFile(
      path.join(stagedLibDir, entry),
      path.join(appBundleLibDir, entry),
    );
  }

  console.log(`Synced staged llama.cpp dylibs into ${appBundleLibDir}`);
}

function runTauriBuild(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      path.join(repoRoot, "node_modules", ".bin", "tauri"),
      ["build", ...args],
      {
        cwd: repoRoot,
        stdio: "inherit",
        shell: false,
      },
    );

    child.on("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`tauri build exited with code ${code ?? "unknown"}`));
      }
    });

    child.on("error", reject);
  });
}

async function main() {
  const args = process.argv.slice(2);
  await runTauriBuild(args);
  await copyStagedDylibsIntoBundle();
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
