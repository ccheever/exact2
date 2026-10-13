#!/usr/bin/env bun
// LLP 1116.015 D1: a view's `when` is `if`, `else when` is `else if`, `each` is `for`. Run once over a
// checkout (or an app with `--root <dir>`), then deleted after the open branches have taken it (T6).
//   bun scripts/migrate-1116-015.mjs [--root <dir>] [--dry]
import { execSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const args = process.argv.slice(2);
const root = args.includes("--root") ? args[args.indexOf("--root") + 1] : process.cwd();
const dry = args.includes("--dry");

// A view word at the start of a line, or after an escaped newline in a string that holds Contract.
// Kotlin is not read; `each` needs its `x in` (or `x, i in`) to be the view's.
const WORD = String.raw`(else when|when(?=\s+[^\s=])|each(?=\s+[{a-z_][\w{}]*(?:\s*,\s*[{a-z_][\w{}]*)?\s+in\s))\b`;
const LINE = new RegExp(String.raw`^(\s*)` + WORD, "");
const ESCAPED = new RegExp(String.raw`(\\n\s*)` + WORD, "g");
const to = (w) => (w === "else when" ? "else if" : w === "when" ? "if" : "for");

// Markdown that is today's documentation; dated records (LLPs, issues, reviews) keep what they said.
const MD_DIRS = ["docs/", "contract/cli/src/vocab/", "semantics/README.md", "apps/", "templates/", "README.md", "game/"];
const FENCE_SKIP = /^(rust|rs|js|javascript|ts|typescript|swift|kotlin|kt|sh|bash|zsh|json|toml|lean|css|html|text|diff|console)\b/;
// Reject fixtures whose point is the old word.
const KEEP_FIXTURES = new Set(["syntax-view-keyword", "syntax-else-keyword"]);

const files = execSync("git ls-files", { cwd: root, encoding: "utf8", maxBuffer: 1 << 28 }).split("\n").filter(Boolean);
let changedFiles = 0, changedLines = 0;
for (const f of files) {
  const ext = f.slice(f.lastIndexOf(".") + 1);
  const md = ext === "md";
  if (md && !MD_DIRS.some((d) => f === d || f.startsWith(d))) continue;
  if (!md && !["contract", "rs", "swift", "mjs", "js", "ts", "txt"].includes(ext)) continue;
  if (f.startsWith("vendor/") || f.startsWith("llp/") || f === "scripts/migrate-1116-015.mjs") continue;
  const path = join(root, f);
  const src = readFileSync(path, "utf8");
  const lines = src.split("\n");
  let fence = null, fixture = null, n = 0;
  const out = lines.map((line) => {
    if (md) {
      const m = line.match(/^\s*(```+|~~~+)(.*)$/);
      if (m) {
        fence = fence ? null : { skip: FENCE_SKIP.test(m[2].trim()) };
        return line;
      }
      if (!fence || fence.skip) return line;
    }
    if (ext === "txt") {
      const m = line.match(/^== (\S+)/);
      if (m) fixture = m[1];
      if (KEEP_FIXTURES.has(fixture)) return line;
    }
    let next = line.replace(LINE, (_, ws, w) => ws + to(w));
    if (!md && ext !== "contract") next = next.replace(ESCAPED, (_, nl, w) => nl + to(w));
    if (next !== line) n++;
    return next;
  });
  if (n) {
    changedFiles++;
    changedLines += n;
    if (dry) console.log(`${n}\t${f}`);
    else writeFileSync(path, out.join("\n"));
  }
}
console.log(`${dry ? "would change" : "changed"} ${changedLines} lines in ${changedFiles} files`);
