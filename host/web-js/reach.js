// The system's ways into an app on the JS target (LLP 1133.023 §10), as the runner's
// `modules/declared/reach.rs`: under the agent the page keeps the icon's menu and the content index the host's
// stores would, and tells no platform anything.
//
// - `"shortcuts": "set" | "clear"` replaces the app's list: each item's `name` is not empty, its `url` an absolute app
//   path the routes match, its icons local, else `TypeError` and the list is kept. `"pin"` answers `requested`, or
//   `unsupported` while the agent's word `<extension>.pinSupported` is false.
// - `"index": "add" | "delete" | "set" | "entries"`: entries by `id`, of a `kind`, in the order the app set them; `set`
//   replaces one kind's and records the difference. At most 5,000 entries of at most 1,024 bytes each
//   (`QuotaExceededError`). A source declared `entries` answers them, asked again when they change.
import { routeMatches } from "./router.js";

const MOST = 5_000, TEXT = 1_024;
const canon = v => Array.isArray(v) ? `[${v.map(canon).join(",")}]` : v && typeof v === "object"
  ? `{${Object.keys(v).sort().map(k => `${JSON.stringify(k)}:${canon(v[k])}`).join(",")}}` : JSON.stringify(v ?? null);
const same = (a, b) => canon(a) === canon(b);
// Lowercased, with a Latin letter's marks dropped: the runner's `fold`.
const fold = t => t.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** `say(line)` journals; `announce(keys)` asks a source's readers again; `preferred(key)` an agent word's value. */
export function reach({ say, announce, preferred }) {
  let shortcuts = [], pins = [], index = [], last = null;
  const urlRefused = url => typeof url !== "string" || !url.startsWith("/") || url.startsWith("//") ? `url=${JSON.stringify(url)} is an absolute app path`
    : !routeMatches(url) ? `url=${JSON.stringify(url)} matches none of the routes` : null;
  const iconsRefused = icons => { const remote = (icons ?? []).map(i => i?.src).find(s => typeof s === "string" && s.includes("://") && !s.startsWith("app:/")); return remote ? `icon ${JSON.stringify(remote)} is a local image (an asset or an app:/ path)` : null; };
  const shortcutRefused = i => !i?.name ? "a shortcut's name= is not empty" : urlRefused(i.url) ?? iconsRefused(i.icons);
  const entryRefused = (e, kind) => !e?.id ? "an entry's id= is not empty" : !kind && !e.kind ? "an entry added alone names its kind="
    : new TextEncoder().encode(JSON.stringify(e)).length > TEXT ? `an entry holds at most ${TEXT} bytes of text` : urlRefused(e.url) ?? iconsRefused(e.icons);
  const refused = (row, code, why) => { say(`${row.key}: refused: ${code} (${why})`); return { error: code, message: why }; };
  const indexKeys = rows => rows.filter(r => r.sub.index === "entries").map(r => r.key);
  return {
    /** A call of `row` with `args`: null, `{error}`, or undefined for a name not the reach's. */
    effect(row, args, rows) {
      const op = row.sub.shortcuts;
      if (op && op !== "pin") {
        const items = op === "set" ? args.shortcuts ?? [] : [];
        const why = items.map(shortcutRefused).find(Boolean);
        if (why) return refused(row, "TypeError", why);
        say(`${row.key}: ${items.length} shortcuts`);
        shortcuts = items;
        return null;
      }
      if (!row.sub.index || row.sub.index === "entries") return undefined;
      let next = [...index], kind;
      const added = [], updated = [], deleted = [];
      if (row.sub.index === "add") {
        const e = args.description;
        const why = entryRefused(e, null);
        if (why) return refused(row, "TypeError", why);
        kind = e.kind;
        const at = next.findIndex(o => o.id === e.id);
        if (at < 0) { next.push(e); added.push(e.id); } else if (!same(next[at], e)) { next[at] = e; updated.push(e.id); }
      } else if (row.sub.index === "delete") {
        const gone = next.find(o => o.id === args.id);
        kind = gone?.kind ?? null;
        if (gone) { next = next.filter(o => o !== gone); deleted.push(args.id); }
      } else {
        kind = args.kind;
        if (!kind) return refused(row, "TypeError", "kind= is not empty");
        const entries = [];
        for (const raw of args.entries ?? []) {
          const e = { ...raw, kind };
          const why = entryRefused(e, kind);
          if (why) return refused(row, "TypeError", why);
          entries.push(e);
        }
        const mine = o => o.kind === kind, given = o => entries.some(e => e.id === o.id);
        for (const e of entries) { const o = next.find(o => o.id === e.id); if (!o) added.push(e.id); else if (!same(o, e)) updated.push(e.id); }
        for (const o of next.filter(mine)) if (!given(o)) deleted.push(o.id);
        // The kind's entries take the place of its first, in the order given (D9).
        const first = next.findIndex(mine), at = next.slice(0, first < 0 ? next.length : first).filter(o => !given(o)).length;
        const rest = next.filter(o => !mine(o) && !given(o));
        next = [...rest.slice(0, at), ...entries, ...rest.slice(at)];
      }
      if (next.length > MOST) return refused(row, "QuotaExceededError", `the index holds at most ${MOST} entries`);
      say(`${row.key}: ${added.length} added, ${updated.length} updated, ${deleted.length} deleted`);
      index = next;
      last = { kind, added, updated, deleted };
      const keys = indexKeys(rows);
      if (keys.length) announce(keys);
      return null;
    },
    /** `requestPinShortcut` (D11). */
    pin(row, args) {
      const why = shortcutRefused(args.shortcut);
      if (why) return refused(row, "TypeError", why);
      if (preferred(`${row.key.split(".")[0]}.pinSupported`) === false) { say(`${row.key}: unsupported`); return { state: "unsupported" }; }
      pins.push(args.shortcut);
      say(`${row.key}: requested`);
      return { state: "requested" };
    },
    /** A source declared `"index": "entries"`: every entry; undefined for any other. */
    value: row => row.sub.index === "entries" ? index.map(e => ({ ...e })) : undefined,
    /** The entries of `kind` whose title or description holds `matching`, folded (D9). */
    entities: (kind, matching) => index.filter(e => e.kind === kind && (matching == null
      || [e.title, e.description].some(t => typeof t === "string" && fold(t).includes(fold(matching))))),
    /** The reach's fields of an extension's `state.extensions` entry, from its rows. */
    state(rows, e) {
      if (rows.some(r => r.sub.shortcuts)) Object.assign(e, { manifest: rows.find(r => r.sub.manifest)?.sub.manifest ?? [], shortcuts, pinRequests: pins });
      if (rows.some(r => r.sub.index)) {
        const counts = {};
        for (const x of index) counts[x.kind] = (counts[x.kind] ?? 0) + 1;
        Object.assign(e, { counts, last });
      }
    },
  };
}
