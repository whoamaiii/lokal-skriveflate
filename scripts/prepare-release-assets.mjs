import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmod,
  copyFile,
  mkdir,
  readFile,
  realpath,
  stat,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const tauriRoot = path.join(repoRoot, "src-tauri");
const binariesDir = path.join(tauriRoot, "binaries");
const stagedRuntimeDir = path.join(
  tauriRoot,
  "resources",
  "embedded-runtime",
  "staged",
);
const stagedLibDir = path.join(stagedRuntimeDir, "lib");
const manifestPath = path.join(
  tauriRoot,
  "resources",
  "embedded-runtime",
  "manifest.json",
);
const packageJsonPath = path.join(repoRoot, "package.json");

const DEFAULT_MODEL = {
  id: "neurologg-q4_k_m",
  label: "Neurologg Q4_K_M",
  tier: "Standard",
  filename: "neurologg-q4_k_m.gguf",
};

function resolveFromPath(command) {
  const result = spawnSync("which", [command], {
    cwd: repoRoot,
    encoding: "utf8",
  });

  if (result.status !== 0) {
    return null;
  }

  const candidate = result.stdout.trim();
  return candidate ? candidate : null;
}

async function resolveExistingPath(candidate, label) {
  if (!candidate) {
    return null;
  }

  try {
    await stat(candidate);
    return await realpath(candidate);
  } catch {
    console.warn(`Skipping missing ${label} candidate: ${candidate}`);
    return null;
  }
}

async function sha256(filePath) {
  const hash = createHash("sha256");
  hash.update(await readFile(filePath));
  return hash.digest("hex");
}

async function ensureExecutable(filePath) {
  await chmod(filePath, 0o755);
}

async function ensureReadable(filePath) {
  await chmod(filePath, 0o644);
}

async function copy(source, target, executable = false) {
  await mkdir(path.dirname(target), { recursive: true });
  await copyFile(source, target);
  if (executable) {
    await ensureExecutable(target);
  } else {
    await ensureReadable(target);
  }
}

function dylibNamesFor(binaryPath) {
  const result = spawnSync("otool", ["-L", binaryPath], {
    cwd: repoRoot,
    encoding: "utf8",
  });

  if (result.status !== 0) {
    throw new Error(
      `Could not inspect dylib dependencies for ${binaryPath}: ${result.stderr || result.stdout}`,
    );
  }

  return result.stdout
    .split("\n")
    .slice(1)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => line.split(" ")[0])
    .filter((entry) => entry.startsWith("@rpath/"))
    .map((entry) => path.basename(entry));
}

async function stageLlamaDependencies(llamaBinaryPath) {
  const siblingLibDir = path.resolve(path.dirname(llamaBinaryPath), "..", "lib");
  const dylibNames = dylibNamesFor(llamaBinaryPath);

  try {
    await stat(siblingLibDir);
  } catch {
    console.warn(
      `No sibling lib directory found for ${llamaBinaryPath}; skipping staged dylib dependencies.`,
    );
    return [];
  }

  const staged = [];
  for (const dylibName of dylibNames) {
    const source = path.join(siblingLibDir, dylibName);
    const resolved = await resolveExistingPath(source, `llama dependency ${dylibName}`);
    if (!resolved) {
      throw new Error(
        `Missing required llama.cpp dylib dependency: ${dylibName} in ${siblingLibDir}`,
      );
    }

    const target = path.join(stagedLibDir, dylibName);
    await copy(resolved, target, false);
    staged.push(target);
  }

  return staged;
}

async function main() {
  const packageJson = JSON.parse(await readFile(packageJsonPath, "utf8"));
  const bundleVersion = packageJson.version ?? "0.1.0";

  const codexSource = await resolveExistingPath(
    process.env.LOKAL_RELEASE_CODEX_SOURCE ?? resolveFromPath("codex"),
    "Codex",
  );
  const llamaSource = await resolveExistingPath(
    process.env.LOKAL_RELEASE_LLAMA_SOURCE ?? resolveFromPath("llama-server"),
    "llama-server",
  );
  const modelSource = await resolveExistingPath(
    process.env.LOKAL_RELEASE_MODEL_SOURCE,
    "model",
  );

  if (!codexSource) {
    throw new Error(
      "Missing Codex source. Set LOKAL_RELEASE_CODEX_SOURCE or install codex locally.",
    );
  }

  if (!llamaSource) {
    throw new Error(
      "Missing llama-server source. Set LOKAL_RELEASE_LLAMA_SOURCE or install llama-server locally.",
    );
  }

  if (!modelSource) {
    throw new Error(
      "Missing model source. Set LOKAL_RELEASE_MODEL_SOURCE to the real GGUF file before staging release assets.",
    );
  }

  const codexTarget = path.join(binariesDir, "codex-aarch64-apple-darwin");
  const llamaSidecarTarget = path.join(
    binariesDir,
    "llama-server-aarch64-apple-darwin",
  );
  const stagedLlamaTarget = path.join(stagedRuntimeDir, "llama-server");
  const stagedModelTarget = path.join(stagedRuntimeDir, DEFAULT_MODEL.filename);

  await copy(codexSource, codexTarget, true);
  await copy(llamaSource, llamaSidecarTarget, true);
  await copy(llamaSource, stagedLlamaTarget, true);
  await copy(modelSource, stagedModelTarget, false);
  const stagedDylibs = await stageLlamaDependencies(llamaSource);

  const manifest = {
    schema_version: 1,
    bundle_version: bundleVersion,
    codex: {
      sidecar_name: "codex",
      sha256: await sha256(codexTarget),
      placeholder: false,
    },
    engine: {
      sidecar_name: "llama-server",
      resource: "embedded-runtime/staged/llama-server",
      target_name: "llama-server",
      sha256: await sha256(stagedLlamaTarget),
      placeholder: false,
    },
    models: [
      {
        ...DEFAULT_MODEL,
        resource: `embedded-runtime/staged/${DEFAULT_MODEL.filename}`,
        sha256: await sha256(stagedModelTarget),
        bundled: true,
        placeholder: false,
      },
    ],
  };

  await writeFile(`${manifestPath}`, `${JSON.stringify(manifest, null, 2)}\n`);

  console.log("Staged release assets:");
  console.log(`- Codex: ${codexTarget}`);
  console.log(`- llama-server sidecar: ${llamaSidecarTarget}`);
  console.log(`- llama-server resource: ${stagedLlamaTarget}`);
  if (stagedDylibs.length > 0) {
    console.log("- llama.cpp dylibs:");
    for (const dylib of stagedDylibs) {
      console.log(`  - ${dylib}`);
    }
  }
  console.log(`- model resource: ${stagedModelTarget}`);
  console.log(`- manifest: ${manifestPath}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
