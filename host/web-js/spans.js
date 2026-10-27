// A device a call names, a send answered after a span, and a list whose
// changes are events: forms of the declared substitute (LLP 1133.002 §6) a
// module with devices of its own declares (LLP 1133.022 D10 item 3, §9), as
// the runner's `modules/spans.rs` answers them:
//
// - `of` (`{source, list, key, current, missing, lacks, needs}`): a call
//   names an entry of a sibling source's list by its `key` option, or, leaving
//   it out, the one the source's `current` names when it arrives. No such
//   entry fails `missing` (`NotFoundError`); an option value not in the
//   entry's `lacks` list field, or an entry whose `needs` field is not true,
//   fails `NotSupportedError`.
// - `answer`, `after`, `preempts`: a send answers `answer` after the sum of
//   the options `after` names, ms on the session's clock; one that declares
//   `preempts` first ends each running span of its extension on the same
//   entry, which answers `preempts`.
// - `played` (`{state, by}`): each call kept in `state.extensions.<module>.<state>`
//   with its options, origin and, once answered, `result`; with `by`, the
//   latest per value of that option.
// - `changes` on a source (`{list, key, added, removed}`): a preferred value
//   that changes the list raises `added` and `removed` with each entry.
import { clock, time } from "./rt.js";

/** `m`: the modules runtime's `{sibling, valueOf, route, raise, say, extensionOf}`. */
export function spans(m) {
  const running = [], played = new Map();
  let refs = 0;
  const same = (a, b) => (typeof a === "number" && typeof b === "number") ? a === b : JSON.stringify(a) === JSON.stringify(b);
  const ms = (sub, args) => (sub.after ?? []).reduce((t, o) => t + (typeof args?.[o] === "number" ? args[o] : 0), 0);
  const entryOf = (row, of, args) => {
    const source = m.sibling(row, of.source), key = of.key ?? "index", wanted = args?.[key];
    const list = source ? m.valueOf(source)?.[of.list] : null;
    return wanted == null || !Array.isArray(list) ? null : list.find(e => same(e?.[key], wanted)) ?? null;
  };
  const play = (row, args, origin, ref, result) => {
    const p = row.sub.played;
    if (!p?.state) return;
    const entry = Object.fromEntries(Object.entries(args ?? {}).filter(([, v]) => v != null || p.by));
    entry.origin = origin;
    if (ref != null) { entry.ref = ref; entry.result = result ?? null; } else if (result !== undefined) entry.result = result;
    const byState = played.get(m.extensionOf(row)) ?? played.set(m.extensionOf(row), new Map()).get(m.extensionOf(row));
    const list = (byState.get(p.state) ?? []).filter(e => !p.by || !same(e[p.by], args?.[p.by]));
    list.push(entry);
    byState.set(p.state, list);
  };
  const end = (s, json) => {
    running.splice(running.indexOf(s), 1);
    const result = json?.result ?? json;
    for (const byState of played.values()) for (const list of byState.values()) for (const e of list) if (e.ref === s.ref) e.result = result;
    m.say(`${s.row.key}: ${typeof result === "string" ? result : "answered"}`);
    s.resolve(json);
  };
  const due = () => { time(); for (const s of running.filter(s => s.deadline <= clock.now).sort((a, b) => a.deadline - b.deadline || a.ref - b.ref)) end(s, s.row.sub.answer); };
  const preempt = (row, args) => {
    const key = row.sub.of?.key ?? "index";
    for (const s of running.filter(s => m.extensionOf(s.row) === m.extensionOf(row) && same(s.args?.[key], args?.[key]))) end(s, row.sub.preempts);
  };
  return {
    /** A call's `of` key left out: the source's `current` now, written into `args`. */
    resolveOf(row, args) {
      const of = row.sub.of, key = of?.key ?? "index";
      if (!of || m.route(row) !== "substitute" || args?.[key] != null) return;
      const source = m.sibling(row, of.source), current = source ? m.valueOf(source)?.[of.current ?? "current"] : null;
      if (current != null) args[key] = current;
    },
    /** What `of` decides of a call: null when it names an entry that can do what it asks. */
    refusal(row, args) {
      const of = row.sub.of;
      if (!of) return null;
      const entry = entryOf(row, of, args);
      if (!entry) return { error: of.missing ?? "NotFoundError" };
      for (const [option, field] of Object.entries(of.lacks ?? {})) {
        if (typeof args?.[option] === "string" && !(entry[field] ?? []).includes(args[option])) return { error: "NotSupportedError" };
      }
      return of.needs && entry[of.needs] !== true ? { error: "NotSupportedError" } : null;
    },
    /** A send's `of`, then its declared `answer`: `{now}`, `{span: ms}`, or null when it declares neither. */
    decide(row, send, args) {
      const refused = this.refusal(row, args);
      if (refused) return { now: refused };
      if (!send || !row.sub.answer) return null;
      const t = ms(row.sub, args);
      return t > 0 ? { span: t } : { now: row.sub.answer };
    },
    /** A call `d` admitted: it ends what it preempts, and is kept where it is `played`; a span's promise. */
    admitted(row, args, origin, d) {
      if (m.route(row) !== "substitute" || (d.now !== undefined && d.now?.error) || (d.now === undefined && !d.span)) return null;
      if (row.sub.preempts) preempt(row, args);
      if (!d.span) { play(row, args, origin, null, d.now?.result ?? d.now); return null; }
      const ref = ++refs;
      play(row, args, origin, ref);
      return new Promise(resolve => {
        running.push({ row, args, ref, deadline: clock.now + d.span, resolve });
        if (!clock.agent) setTimeout(due, d.span);
      });
    },
    /** An effect (a send answering nothing) the substitute did. */
    effected(row, args, origin) { if (m.route(row) === "substitute") play(row, args, origin, null); },
    due,
    /** The running spans' count, which a settle does not wait on as requests, and the soonest end. */
    count: () => running.length,
    next: () => running.length ? Math.min(...running.map(s => s.deadline)) : null,
    /** What `played` keeps, by extension, as `state.extensions.<module>` fields. */
    state(extension, e) { for (const [state, list] of played.get(extension) ?? []) e[state] = list.map(x => ({ ...x })); },
    /** Source `row`'s preferred value went from `before` to `after`: its `changes` raise their events. */
    changed(row, before, after) {
      const c = row.sub.changes;
      if (!c?.list || !c.key) return;
      const entries = v => Array.isArray(v?.[c.list]) ? v[c.list] : [];
      const was = entries(before), is = entries(after), absent = (from, e) => !from.some(f => same(f?.[c.key], e?.[c.key]));
      for (const e of was.filter(e => absent(is, e))) if (c.removed) m.raise(`${m.extensionOf(row)}.${c.removed}`, [e], "device");
      for (const e of is.filter(e => absent(was, e))) if (c.added) m.raise(`${m.extensionOf(row)}.${c.added}`, [e], "device");
    },
  };
}

/** Set each `path` of a declared `fill` where `v` leaves it out: `"now"` is `now()`; `{table, by}` the entry of the
 * substitute's `tables.<table>` that the filled object's field `by` names, filled after the plain fillers so `by`
 * can be one; a part `list[]` fills each entry of that list. A table filler with `on`, a list of targets, fills only
 * on those, as the runner's `fill_all` does. */
export function fillAll(sub, v, now) {
  const fills = Object.entries(sub.fill ?? {}), tabled = f => f && typeof f === "object" && typeof f.table === "string";
  const here = f => !Array.isArray(f?.on) || f.on.includes("web");
  for (const pass of [false, true]) {
    for (const [path, filler] of fills.filter(([, f]) => tabled(f) === pass && here(f))) {
      fillWith(v, path.split("."), object => filler === "now" ? now() : pass ? structuredClone(sub.tables?.[filler.table]?.[object?.[filler.by]]) : structuredClone(filler));
    }
  }
  return v;
}

function fillWith(v, parts, made) {
  if (v === null || typeof v !== "object" || Array.isArray(v) || !parts.length) return;
  const [part, ...rest] = parts;
  if (part.endsWith("[]")) {
    for (const entry of Array.isArray(v[part.slice(0, -2)]) ? v[part.slice(0, -2)] : []) fillWith(entry, rest, made);
    return;
  }
  if (rest.length) return fillWith(v[part] ??= {}, rest, made);
  if (v[part] === undefined) { const filler = made(v); if (filler !== undefined) v[part] = filler; }
}
