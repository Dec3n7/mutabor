// Downloads the external tools listed in tools/manifest.json for the current
// Rust target, verifies their SHA-256 and places them where Tauri expects
// sidecars: src-tauri/binaries/<name>-<target-triple>[.exe].
//
// Optional engines are downloaded by the app at runtime; `--optional` fetches
// them here too, for the tests.
//
// Usage: npm run tools [-- --force] [-- --optional]

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, createReadStream, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const manifest = JSON.parse(readFileSync(join(root, "tools/manifest.json"), "utf8"));
const cache = join(root, ".cache/tools");
const outDir = join(root, "src-tauri/binaries");
const force = process.argv.includes("--force");
const optional = process.argv.includes("--optional");

const triple = execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (\S+)$/m)[1];
const exe = triple.includes("windows") ? ".exe" : "";

function sha256(path) {
  return new Promise((ok, fail) => {
    const h = createHash("sha256");
    createReadStream(path).on("data", (d) => h.update(d)).on("end", () => ok(h.digest("hex"))).on("error", fail);
  });
}

async function download(url, path) {
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  writeFileSync(path, Buffer.from(await res.arrayBuffer()));
}

/** Finds `rel` (e.g. "bin/ffmpeg.exe") under dir, ignoring the archive's top folder. */
function find(dir, rel) {
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry);
    if (existsSync(join(p, rel))) return join(p, rel);
    if (statSync(p).isDirectory()) {
      const hit = find(p, rel);
      if (hit) return hit;
    }
  }
}

for (const [name, tool] of Object.entries(manifest)) {
  if (name.startsWith("$") || name === "planned" || (tool.optional && !optional)) continue;
  const build = tool.builds[triple];
  if (!build) {
    console.warn(`${name}: no build listed for ${triple}, skipped`);
    continue;
  }
  const targets = Object.entries(build.files).map(([rel, as]) => [rel, join(outDir, `${as}-${triple}${exe}`)]);
  if (!force && targets.every(([, out]) => existsSync(out))) {
    console.log(`${name} ${tool.version}: already in place`);
    continue;
  }

  mkdirSync(cache, { recursive: true });
  const archive = join(cache, build.url.split("/").pop());
  if (!existsSync(archive) || force) {
    console.log(`${name} ${tool.version}: downloading ${build.url}`);
    await download(build.url, archive);
  }
  const actual = await sha256(archive);
  if (actual !== build.sha256) {
    rmSync(archive);
    throw new Error(`${name}: SHA-256 mismatch\n  expected ${build.sha256}\n  got      ${actual}`);
  }
  console.log(`${name}: SHA-256 verified`);

  const unpacked = join(cache, `${name}-${tool.version}-${triple}`);
  rmSync(unpacked, { recursive: true, force: true });
  mkdirSync(unpacked, { recursive: true });
  // Windows' own bsdtar reads zip; a GNU tar from Git Bash on PATH would not.
  const tar = process.platform === "win32" ? join(process.env.SystemRoot, "System32", "tar.exe") : "tar";
  execFileSync(tar, ["-xf", archive, "-C", unpacked]);

  mkdirSync(outDir, { recursive: true });
  for (const [rel, out] of targets) {
    const src = find(unpacked, rel);
    if (!src) throw new Error(`${name}: ${rel} not found in ${archive}`);
    copyFileSync(src, out);
    console.log(`  -> ${out}`);
  }
  rmSync(unpacked, { recursive: true, force: true });
}
