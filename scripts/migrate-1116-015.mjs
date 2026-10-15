#!/usr/bin/env bun
// LLP 1116.015, run once over a checkout (or an app with `--root <dir>`), then deleted after the open
// branches have taken it (T6):
//   D1  a view's `when` is `if`, `else when` is `else if`, `each` is `for`;
//   D2  a roster call on its subject is a method call (`trim(toLowerCase(s))` is `s.toLowerCase().trim()`,
//       `length(xs)` is `xs.length`), and `list<T>` is `T[]`.
//   bun scripts/migrate-1116-015.mjs [--root <dir>] [--dry] [--phase d1|d2]
// On a branch, run it before merging main: main's tests and docs quote the old spelling on purpose
// (the refusals' own cases), and a second pass would rewrite them.
import { execSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const args = process.argv.slice(2);
const root = args.includes("--root") ? args[args.indexOf("--root") + 1] : process.cwd();
const dry = args.includes("--dry");
const phase = args.includes("--phase") ? args[args.indexOf("--phase") + 1] : "all";

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
const write = (f, path, before, after) => {
  if (after === before) return;
  changedFiles++;
  changedLines += after.split("\n").filter((l, k) => l !== before.split("\n")[k]).length;
  if (dry) console.log(f);
  else writeFileSync(path, after);
};

function d1() {
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
}

// D2 reads the same Markdown and fixtures; in code, the string literals of files that hold Contract.
const D2_KEEP = new Set(["syntax-method-form", "syntax-list-type", "syntax-length-field"]);
const D2_SKIP = new Set(["contract/syntax/src/idioms.rs", "plan/build.rs", "plan/tests/format.rs", "contract/types/src/lib.rs", "contract/types/src/routes.rs"]);
function d2() {
  for (const f of files) {
    const ext = f.slice(f.lastIndexOf(".") + 1);
    if (f.startsWith("vendor/") || f.startsWith("llp/") || f === "scripts/migrate-1116-015.mjs") continue;
    // Refusals that quote the old spelling, and the roster table's internal type names.
    if (D2_SKIP.has(f)) continue;
    const path = join(root, f);
    let before;
    try { before = readFileSync(path, "utf8"); } catch { continue; }
    let after = before;
    if (ext === "contract") after = rewrite(before);
    else if (ext === "md" && MD_DIRS.some((d) => f === d || f.startsWith(d))) after = d2Markdown(before);
    else if (ext === "txt" && /^== /m.test(before)) after = d2Fixtures(before);
    else if ((ext === "rs" || ext === "swift") && /component |\bview\b|list</.test(before)) after = rewriteLiterals(before, ext === "swift");
    else if (f === "game/new.mjs") after = rewrite(before);
    write(f, path, before, after);
  }
}
function d2Markdown(src) {
  let fence = null;
  return src.split("\n").map((line) => {
    const m = line.match(/^\s*(```+|~~~+)(.*)$/);
    if (m) { fence = fence ? null : { skip: FENCE_SKIP.test(m[2].trim()) && !/^text\b/.test(m[2].trim()) }; return line; }
    if (fence) return fence.skip ? line : rewrite(line);
    // inline code in prose
    return line.replace(/`([^`\n]+)`/g, (whole, code) => (/[(<]/.test(code) ? "`" + rewrite(code) + "`" : whole));
  }).join("\n");
}
function d2Fixtures(src) {
  let keep = false;
  return src.split("\n").map((line) => {
    const m = line.match(/^== (\S+)/);
    if (m) keep = D2_KEEP.has(m[1]);
    return keep || line.startsWith("#") || line.startsWith("==") ? line : rewrite(line);
  }).join("\n");
}

// `rewriteContract(text, esc)` rewrites Contract text; with `esc`, the text is the inside of a
// Rust or Swift "…" literal, where Contract's quotes are `\"` and `\n` is a newline.
const METHODS = new Set([
  "isEmpty", "toString", "includes", "startsWith", "endsWith", "trim", "toLowerCase", "slice",
  "concat", "indexOf", "split", "replaceAll", "at", "map", "filter", "join", "toFixed",
]);
const PROPERTIES = new Set(["length"]);

// Tokens: {k: "id"|"num"|"str"|"tpl"|"p"|"ws", t: text, parts?: for tpl}
function lex(s, esc) {
  const out = [];
  let i = 0;
  const n = s.length;
  while (i < n) {
    const c = s[i];
    // whitespace, including a literal's escaped newline/tab and line continuation
    if (/\s/.test(c)) { let j = i; while (j < n && /\s/.test(s[j])) j++; out.push({ k: "ws", t: s.slice(i, j) }); i = j; continue; }
    if (esc && c === "\\" && (s[i + 1] === "n" || s[i + 1] === "t")) { out.push({ k: "ws", t: s.slice(i, i + 2) }); i += 2; continue; }
    if (esc && c === "\\" && s[i + 1] === "\n") { out.push({ k: "ws", t: s.slice(i, i + 2) }); i += 2; continue; }
    // a Contract string
    if ((!esc && c === '"') || (esc && c === "\\" && s[i + 1] === '"')) {
      let j = i + (esc ? 2 : 1);
      while (j < n) {
        if (esc) {
          if (s[j] === "\\" && s[j + 1] === "\\" && s[j + 2] === "\\" && s[j + 3] === '"') { j += 4; continue; }
          if (s[j] === "\\" && s[j + 1] === '"') { j += 2; break; }
          if (s[j] === "\\") { j += 2; continue; }
        } else {
          if (s[j] === "\\") { j += 2; continue; }
          if (s[j] === '"') { j++; break; }
        }
        j++;
      }
      out.push({ k: "str", t: s.slice(i, j) }); i = j; continue;
    }
    // a template: `…${expr}…`
    if (c === "`") {
      let j = i + 1;
      const parts = []; let lit = "`";
      while (j < n && s[j] !== "`") {
        if (s[j] === "\\") { lit += s.slice(j, j + 2); j += 2; continue; }
        if (s[j] === "$" && s[j + 1] === "{") {
          // find the matching brace, skipping nested strings and templates
          let d = 0, k = j + 1;
          for (; k < n; k++) {
            const ch = s[k];
            if (ch === "{") d++;
            else if (ch === "}") { d--; if (d === 0) break; }
            else if (ch === "`") { k = skipTemplate(s, k); }
            else if (ch === '"' && !esc) { k = skipString(s, k); }
          }
          parts.push({ lit: lit + "${" }); lit = "";
          parts.push({ expr: s.slice(j + 2, k) });
          lit = "}"; j = k + 1; continue;
        }
        lit += s[j]; j++;
      }
      // An unterminated template (a fence's "```" in a Rust string) is left as it is.
      if (j >= n) { out.push({ k: "ws", t: s.slice(i) }); i = n; continue; }
      lit += "`"; parts.push({ lit });
      out.push({ k: "tpl", t: s.slice(i, j + 1), parts }); i = j + 1; continue;
    }
    if (/[A-Za-z_]/.test(c)) { let j = i; while (j < n && /[\w]/.test(s[j])) j++; out.push({ k: "id", t: s.slice(i, j) }); i = j; continue; }
    if (/[0-9]/.test(c)) { let j = i; while (j < n && /[0-9.eE_]/.test(s[j]) && !(s[j] === "." && !/[0-9]/.test(s[j + 1] ?? ""))) j++; out.push({ k: "num", t: s.slice(i, j) }); i = j; continue; }
    if (c === "/" && s[i + 1] === "/") { let j = i; while (j < n && s[j] !== "\n" && !(esc && s[j] === "\\" && s[j + 1] === "n")) j++; out.push({ k: "ws", t: s.slice(i, j) }); i = j; continue; }
    const two = s.slice(i, i + 2);
    if (["=>", "==", "!=", "<=", ">=", "&&", "||", "${", "{{", "}}"].includes(two)) { out.push({ k: "p", t: two }); i += 2; continue; }
    out.push({ k: "p", t: c }); i++;
  }
  return out;
}
function skipTemplate(s, k) { let j = k + 1; while (j < s.length && s[j] !== "`") { if (s[j] === "\\") j++; j++; } return j; }
function skipString(s, k) { let j = k + 1; while (j < s.length && s[j] !== '"') { if (s[j] === "\\") j++; j++; } return j; }

const OPEN = { "(": ")", "[": "]", "{": "}", "${": "}", "{{": "}}" };

// Index of the token closing the bracket at `i`.
function close(toks, i) {
  let d = 0;
  for (let j = i; j < toks.length; j++) {
    const t = toks[j].t;
    if (toks[j].k === "p" && (t === "(" || t === "[" || t === "{" || t === "${" || t === "{{")) d++;
    else if (toks[j].k === "p" && (t === ")" || t === "]" || t === "}" || t === "}}")) { d--; if (d === 0) return j; }
  }
  return -1;
}

// Whether `toks` is one postfix expression (a name, literal, call, list or parenthesis, then `.x` / `.f(…)`).
function atomic(toks) {
  const t = toks.filter((x) => x.k !== "ws" || false);
  if (!toks.length) return false;
  // whitespace at depth 0 means more than one term
  let i = 0;
  const seq = toks;
  // strip outer whitespace
  let a = 0, b = seq.length;
  while (a < b && seq[a].k === "ws") a++;
  while (b > a && seq[b - 1].k === "ws") b--;
  if (a >= b) return false;
  i = a;
  const first = seq[i];
  if (first.k === "num") return false; // `(2).toFixed(1)`
  if (first.k === "id" && ["not", "none", "some", "true", "false", "match"].includes(first.t)) return ["none", "true", "false"].includes(first.t) && b - a === 1;
  if (first.k === "id" || first.k === "str" || first.k === "tpl") i++;
  else if (first.k === "p" && (first.t === "(" || first.t === "[")) { const j = close(seq, i); if (j < 0) return false; i = j + 1; }
  else return false;
  while (i < b) {
    const x = seq[i];
    if (x.k === "p" && x.t === "(") { const j = close(seq, i); if (j < 0) return false; i = j + 1; continue; }
    if (x.k === "p" && x.t === "." && seq[i + 1]?.k === "id") { i += 2; continue; }
    return false;
  }
  return true;
}

function render(toks, esc) {
  return toks.map((x) => (x.k === "tpl" ? x.parts.map((p) => (p.expr !== undefined ? rewrite(p.expr, esc) : p.lit)).join("") : x.t)).join("");
}

// Split the tokens strictly inside a call's parentheses at depth-0 commas.
function splitArgs(toks) {
  const args = [];
  let d = 0, cur = [];
  for (const x of toks) {
    if (x.k === "p" && ["(", "[", "{", "${", "{{"].includes(x.t)) d++;
    if (x.k === "p" && [")", "]", "}", "}}"].includes(x.t)) d--;
    if (d === 0 && x.k === "p" && x.t === ",") { args.push(cur); cur = []; continue; }
    cur.push(x);
  }
  args.push(cur);
  return args;
}
const trimWs = (toks) => { let a = 0, b = toks.length; while (a < b && toks[a].k === "ws") a++; while (b > a && toks[b - 1].k === "ws") b--; return toks.slice(a, b); };

function rewrite(text, esc = false) {
  const toks = lex(text, esc);
  let out = "";
  for (let i = 0; i < toks.length; i++) {
    const x = toks[i];
    const prev = (() => { for (let j = i - 1; j >= 0; j--) if (toks[j].k !== "ws") return toks[j]; return null; })();
    const prevRaw = toks[i - 1];
    const isDef = prev && prev.k === "id" && (prev.t === "fn" || prev.t === "action");
    const afterDot = prevRaw && prevRaw.k === "p" && prevRaw.t === ".";
    if (x.k === "id" && (METHODS.has(x.t) || PROPERTIES.has(x.t)) && !isDef && !afterDot && toks[i + 1]?.k === "p" && toks[i + 1].t === "(") {
      const end = close(toks, i + 1);
      if (end > 0) {
        const args = splitArgs(toks.slice(i + 2, end));
        const recvToks = trimWs(args[0]);
        const named = recvToks[0]?.k === "id" && recvToks[1]?.k === "p" && recvToks[1].t === "=";
        // One-argument forms exist only for these; a quote is JavaScript's, not Contract's.
        const unary = PROPERTIES.has(x.t) || ["trim", "toLowerCase", "toString", "isEmpty"].includes(x.t);
        const js = toks.slice(i + 2, end).some((t) => t.k === "p" && t.t === "'");
        if (recvToks.length && !named && !js && (unary ? args.length === 1 : args.length >= 2)) {
          let recv = rewrite(render(recvToks, esc), esc);
          if (!atomic(lex(recv, esc))) recv = `(${recv})`;
          if (PROPERTIES.has(x.t)) out += `${recv}.${x.t}`;
          else {
            const restRaw = args.length > 1 ? render(toks.slice(i + 2, end), esc).slice(render(args[0], esc).length + 1).replace(/^\s+/, "") : "";
            out += `${recv}.${x.t}(${rewrite(restRaw, esc)})`;
          }
          i = end;
          continue;
        }
      }
    }
    // `list<T>` is `T[]`.
    if (x.k === "id" && x.t === "list" && !afterDot && toks[i + 1]?.k === "p" && toks[i + 1].t === "<") {
      let d = 0, end = -1;
      for (let j = i + 1; j < toks.length; j++) {
        if (toks[j].k === "p" && toks[j].t === "<") d++;
        else if (toks[j].k === "p" && toks[j].t === ">") { d--; if (d === 0) { end = j; break; } }
        else if (toks[j].k === "ws" && /\n/.test(toks[j].t)) break;
      }
      if (end > 0) {
        const inner = rewrite(render(toks.slice(i + 2, end), esc), esc).trim();
        if (/^[\w<>\[\], ]+$/.test(inner)) { out += `${inner}[]`; i = end; continue; }
      }
    }
    out += x.k === "tpl" ? render([x], esc) : x.t;
  }
  return out;
}

// The string literals of Rust or Swift source, each rewritten as Contract; code and comments untouched.
function rewriteLiterals(src, swift = false) {
  let out = "", i = 0;
  const n = src.length;
  while (i < n) {
    const c = src[i];
    if (c === "/" && src[i + 1] === "/") { const j = src.indexOf("\n", i); const e = j < 0 ? n : j; out += src.slice(i, e); i = e; continue; }
    if (c === "/" && src[i + 1] === "*") { const j = src.indexOf("*/", i + 2); const e = j < 0 ? n : j + 2; out += src.slice(i, e); i = e; continue; }
    if (!swift && c === "'") {
      // a char literal ('x', '\'', '"') or a lifetime ('a)
      const m = src.slice(i).match(/^'(\\.|[^\\'])'/);
      if (m) { out += m[0]; i += m[0].length; continue; }
      out += c; i++; continue;
    }
    // Rust raw string r"…", r#"…"#
    if (!swift && c === "r" && !/[\w]/.test(src[i - 1] ?? "") && /^r#*"/.test(src.slice(i, i + 8))) {
      const hashes = src.slice(i + 1).match(/^#*/)[0];
      const start = i + 2 + hashes.length;
      const end = src.indexOf('"' + hashes, start);
      if (end < 0) { out += src.slice(i); break; }
      out += src.slice(i, start) + rewrite(src.slice(start, end), false) + '"' + hashes;
      i = end + 1 + hashes.length; continue;
    }
    // Swift multi-line """…"""
    if (swift && src.startsWith('"""', i)) {
      const end = src.indexOf('"""', i + 3);
      if (end < 0) { out += src.slice(i); break; }
      out += '"""' + rewrite(src.slice(i + 3, end), false) + '"""';
      i = end + 3; continue;
    }
    if (c === '"') {
      let j = i + 1;
      while (j < n && src[j] !== '"') { if (src[j] === "\\") j++; j++; }
      // A quote this reader cannot close (Swift's #"…"#, a quote inside an interpolation) ends it: the rest stays as is.
      if (j >= n) { out += src.slice(i); break; }
      out += '"' + rewrite(src.slice(i + 1, j), true) + '"';
      i = j + 1; continue;
    }
    out += c; i++;
  }
  return out;
}

if (phase === "all" || phase === "d1") d1();
if (phase === "all" || phase === "d2") d2();
console.log(`${dry ? "would change" : "changed"} ${changedLines} lines in ${changedFiles} files`);
