#!/usr/bin/env bun
// Stages the embedded T3 server for the macOS bundle (app.json `commands.runtime`):
// the official T3 Code CLI release archive named by server-runtime/runtime-pin.json.
//
//   bun examples/t3-code/stage-runtime.mjs            download (or reuse the cache), verify, smoke, stage
//   bun examples/t3-code/stage-runtime.mjs --offline  the cache only; fails when it is missing or wrong
//   --no-smoke  skips step 4 (for a run under sandbox-exec, where the server's own spawns fail with
//               EPERM even under `(allow default)`); a staged runtime is normally smoked
//
// Verification, in order (T3 Code MIT, see LICENSE-T3; reference 1e2ecbd975: the release
// checksums of packages/shared/src/cliRelease.ts `parseChecksums`, scripts/install.sh's
// download-verify-extract, scripts/smoke-cli-archive.ts's start-and-probe):
//   1. the archive's size and SHA-256 equal the pin's, and the release's SHA256SUMS lists the
//      same hash for the asset. SHA256SUMS is unsigned, so the committed hash is the anchor;
//   2. the archive extracts into a scratch folder; every Mach-O file in it passes
//      `codesign --verify --strict` (the release signs them, docs/operations/release.md);
//   3. `t3 --version` with an empty environment prints the pinned version;
//   4. `t3 --bootstrap-fd 0` starts on a scratch T3 home and a lane port (16000-16999), answers
//      /.well-known/t3/environment with the pinned serverVersion, and exits on SIGTERM.
// Then the archive (its own bytes: the release's signatures and entitlements stay intact; a
// copied tree's Mach-O files would be re-signed by the bundle build) and runtime-manifest.json
// (every path's type, mode, size, SHA-256 or link target) go beside the pin. The bundle carries
// that folder as Contents/Resources/t3-runtime (app.json host.macos.resources, exact2 #103/#215).
//
// Reads only this example and the network (github.com release downloads). Never the reference
// checkout, ~/.t3 or port 3773.
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { closeSync, copyFileSync, existsSync, lstatSync, mkdirSync, mkdtempSync, openSync, readdirSync, readFileSync, readlinkSync, readSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const runtimeDir = join(here, "server-runtime");
// T3_RUNTIME_CACHE moves the download cache (package-app.mjs keeps one beside its export).
export const cacheDir = process.env.T3_RUNTIME_CACHE ? process.env.T3_RUNTIME_CACHE : join(here, ".runtime-cache");

/** `sha256sum` lines (`<hex>  <file>`, a leading `*` for binary mode): cliRelease.ts parseChecksums. */
export function parseChecksums(text) {
  const checksums = new Map();
  for (const line of text.split(/\r?\n/)) {
    const match = /^([0-9a-fA-F]{64})\s+\*?(\S.*)$/.exec(line.trim());
    if (match?.[1] !== undefined && match[2] !== undefined) checksums.set(match[2], match[1].toLowerCase());
  }
  return checksums;
}

export function sha256File(path) {
  const hash = createHash("sha256"), block = Buffer.alloc(1 << 20), fd = openSync(path, "r");
  try { let n; while ((n = readSync(fd, block, 0, block.length, null))) hash.update(block.subarray(0, n)); }
  finally { closeSync(fd); }
  return hash.digest("hex");
}

/** The three hashes must agree, and the size must be the pin's. Returns the failure text or null. */
export function checkArchive(pin, { size, sha256, checksums }) {
  const listed = parseChecksums(checksums).get(pin.asset);
  if (size !== pin.size) return `${pin.asset}: size ${size}, the pin says ${pin.size}`;
  if (listed === undefined) return `${pin.asset} is not listed in SHA256SUMS`;
  if (listed !== pin.sha256) return `SHA256SUMS lists ${listed} for ${pin.asset}; the pin says ${pin.sha256}`;
  if (sha256 !== pin.sha256) return `${pin.asset}: sha256 ${sha256}; the pin and SHA256SUMS say ${pin.sha256}`;
  return null;
}

/** Mach-O by magic (thin either byte order, universal), as host/apple/assets.mjs decides. */
export function isMachO(path) {
  const head = Buffer.alloc(8), fd = openSync(path, "r");
  try { if (readSync(fd, head, 0, 8, 0) < 8) return false; } finally { closeSync(fd); }
  const magic = head.readUInt32BE(0);
  if ([0xfeedface, 0xfeedfacf, 0xcefaedfe, 0xcffaedfe].includes(magic)) return true;
  return (magic === 0xcafebabe || magic === 0xcafebabf) && head.readUInt32BE(4) < 32;
}

/** Every path under `root`, sorted, with what the app checks after unpacking (T3LocalRuntime.swift). */
export function manifestOf(root) {
  const entries = [];
  const walk = (dir) => {
    for (const name of readdirSync(dir).sort()) {
      const path = join(dir, name), stat = lstatSync(path), rel = relative(root, path);
      const mode = stat.mode & 0o7777;
      if (stat.isSymbolicLink()) entries.push({ path: rel, type: "link", link: readlinkSync(path) });
      else if (stat.isDirectory()) { entries.push({ path: rel, type: "dir", mode }); walk(path); }
      else if (stat.isFile()) entries.push({ path: rel, type: "file", mode, size: stat.size, sha256: sha256File(path) });
      else throw new Error(`${rel}: not a file, folder or link`);
    }
  };
  walk(root);
  return entries;
}

async function download(url, to) {
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  const partial = `${to}.partial`;
  await Bun.write(partial, response);
  renameSync(partial, to);
}

const portFree = (port, host) => new Promise((done) => {
  const server = createServer();
  server.once("error", (error) => done(error.code === "EADDRNOTAVAIL"));
  server.listen({ host, port }, () => server.close(() => done(true)));
});

async function lanePort() {
  const fixed = Number(process.env.T3_STAGE_PORT ?? "");
  if (fixed) {
    if (fixed < 16000 || fixed > 16999) throw new Error("T3_STAGE_PORT must be in 16000-16999");
    return fixed;
  }
  for (let port = 16900; port <= 16999; port++) {
    let free = true;
    for (const host of ["127.0.0.1", "0.0.0.0", "::"]) if (!(await portFree(port, host))) { free = false; break; }
    if (free) return port;
  }
  throw new Error("no free lane port in 16900-16999");
}

/** Starts the staged `t3` once on a scratch home and a lane port, as the app will, and stops it. */
async function smoke(tree, pin) {
  const home = mkdtempSync(join(tmpdir(), "t3-stage-home-"));
  const port = await lanePort();
  const token = createHash("sha256").update(String(Math.random())).digest("hex").slice(0, 48);
  const envelope = { mode: "desktop", noBrowser: true, port, t3Home: home, host: "127.0.0.1", desktopBootstrapToken: token, tailscaleServeEnabled: false, tailscaleServePort: 443 };
  const child = spawn(join(tree, "t3"), ["--bootstrap-fd", "0"], {
    cwd: home, stdio: ["pipe", "pipe", "pipe"],
    env: { PATH: "/usr/bin:/bin:/usr/sbin:/sbin", HOME: home, T3CODE_TELEMETRY_ENABLED: "false" },
  });
  let output = "";
  child.stdout.on("data", (chunk) => { output += chunk; });
  child.stderr.on("data", (chunk) => { output += chunk; });
  const exited = new Promise((done) => child.once("exit", (code, signal) => done({ code, signal })));
  child.stdin.end(`${JSON.stringify(envelope)}\n`);
  const started = Date.now();
  let environment = null;
  while (Date.now() - started < 60_000 && child.exitCode === null) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/.well-known/t3/environment`, { signal: AbortSignal.timeout(1000) });
      if (response.ok) { environment = await response.json(); break; }
    } catch {}
    await Bun.sleep(100);
  }
  const readyMs = Date.now() - started;
  child.kill("SIGTERM");
  const timer = setTimeout(() => child.kill("SIGKILL"), 2000);
  const exit = await exited;
  clearTimeout(timer);
  rmSync(home, { recursive: true, force: true });
  if (!environment) throw new Error(`the staged server did not answer on port ${port} within 60 s:\n${output.slice(-4000)}`);
  if (environment.serverVersion !== pin.version) throw new Error(`the staged server reports ${environment.serverVersion}, the pin says ${pin.version}`);
  return { port, readyMs, exit };
}

export async function stage({ offline = false, smokeTest = true, log = console.log } = {}) {
  const pin = JSON.parse(readFileSync(join(runtimeDir, "runtime-pin.json"), "utf8"));
  mkdirSync(cacheDir, { recursive: true });
  const archive = join(cacheDir, pin.asset), sums = join(cacheDir, `SHA256SUMS-${pin.version}`);
  const check = () => existsSync(archive) && existsSync(sums)
    ? checkArchive(pin, { size: lstatSync(archive).size, sha256: sha256File(archive), checksums: readFileSync(sums, "utf8") })
    : `no ${pin.asset} and SHA256SUMS in ${relative(process.cwd(), cacheDir) || "."}`;
  let failure = check();
  if (failure && offline) throw new Error(`--offline: ${failure}`);
  if (failure) {
    log(`cache: ${failure}; downloading ${pin.url}`);
    await download(pin.checksumsUrl, sums);
    await download(pin.url, archive);
    failure = check();
    if (failure) { rmSync(archive, { force: true }); throw new Error(failure); }
  } else log(`cached ${pin.asset}`);
  log(`verified ${pin.asset} ${pin.size} bytes sha256 ${pin.sha256} (pin = SHA256SUMS = computed)`);

  const scratch = mkdtempSync(join(tmpdir(), "t3-stage-"));
  try {
    const tree = join(scratch, "tree");
    mkdirSync(tree);
    const tar = spawnSync("/usr/bin/tar", ["-xzf", archive, "-C", tree, "--strip-components=1"], { encoding: "utf8" });
    if (tar.status !== 0) throw new Error(`tar: ${tar.stderr}`);
    const manifest = manifestOf(tree);
    const machO = manifest.filter((entry) => entry.type === "file" && isMachO(join(tree, entry.path)));
    for (const entry of machO) {
      const verify = spawnSync("codesign", ["--verify", "--strict", join(tree, entry.path)], { encoding: "utf8" });
      if (verify.status !== 0) throw new Error(`codesign --verify --strict ${entry.path}: ${verify.stderr.trim()}`);
    }
    log(`codesign --verify --strict: ${machO.length} Mach-O files pass (${machO.map((entry) => entry.path).join(", ")})`);
    const t3 = manifest.find((entry) => entry.path === "t3");
    if (!t3 || t3.type !== "file" || (t3.mode & 0o777) !== 0o755) throw new Error("the archive's t3 is missing or not mode 0755");
    const version = spawnSync(join(tree, "t3"), ["--version"], { encoding: "utf8", env: {} });
    if (version.status !== 0 || version.stdout.trim() !== `t3 v${pin.version}`) throw new Error(`t3 --version: ${version.stdout}${version.stderr}`);
    log(`t3 --version (empty environment): ${version.stdout.trim()}`);
    if (smokeTest) {
      const smoked = await smoke(tree, pin);
      log(`smoke: ready on 127.0.0.1:${smoked.port} in ${smoked.readyMs} ms; SIGTERM exit ${JSON.stringify(smoked.exit)}`);
    } else log("smoke: skipped (--no-smoke)");

    const files = manifest.filter((entry) => entry.type === "file");
    const document = {
      version: pin.version, asset: pin.asset, sha256: pin.sha256, size: pin.size,
      root: `t3-${pin.version}-darwin-arm64`, stripComponents: 1,
      files: files.length, bytes: files.reduce((sum, entry) => sum + entry.size, 0),
      entries: manifest,
    };
    for (const name of readdirSync(runtimeDir)) if (name !== "runtime-pin.json") rmSync(join(runtimeDir, name), { recursive: true, force: true });
    copyFileSync(archive, join(runtimeDir, pin.asset));
    writeFileSync(join(runtimeDir, "runtime-manifest.json"), `${JSON.stringify(document)}\n`);
    log(`staged server-runtime/${pin.asset} and runtime-manifest.json (${document.entries.length} entries, ${files.length} files, ${document.bytes} bytes unpacked)`);
    return document;
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

if (import.meta.main) {
  stage({ offline: process.argv.includes("--offline"), smokeTest: !process.argv.includes("--no-smoke") }).catch((error) => {
    console.error(`stage-runtime: ${error.message}`);
    process.exit(1);
  });
}
