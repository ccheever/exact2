// A keyed list whose entries change, on the JS target (LLP 1133.033 D17 item 7, D21), as the runner's
// `modules/declared/update.rs`: a call the app places is listed, then its state moves as the app reports it
// (`"update"` on a send) and as the person acts on the system's screen (`tap @ref <choice>`). The forms are that
// file's: a list's `with`, `absent`, `stamp`, `refuse`, `ends`, `tap.choices`; a value in `set` is a literal,
// `{now: true}`, or `{from, map, absent}`.

/** `lists`: the declared lists by state; `rows()` the module rows; `now()` the agent clock's date; `changed(state)`
 * re-asks the sources that answer from a list; `raise(row, event, tap, fields)` raises an entry's event as an
 * activation, with the arguments `tap` declares of its `fields`. */
export function updates({ lists, rows, say, now, changed, raise }) {
  // What every row of list `state` declares of it, the first declaration of each key winning.
  const config = state => {
    const out = {};
    for (const row of rows()) if (row.sub.list?.state === state) for (const [k, v] of Object.entries(row.sub.list)) if (!(k in out)) out[k] = v;
    return out;
  };
  const matches = (fields, rule) => !!rule && typeof rule === "object"
    && Object.entries(rule).every(([field, want]) => Array.isArray(want) ? want.some(v => v === (fields[field] ?? null)) : want === (fields[field] ?? null));
  const prune = state => { const ends = config(state).ends; if (ends && lists.has(state)) lists.set(state, lists.get(state).filter(e => !matches(e.fields, ends))); };
  const valueOf = (value, args) => {
    if (!value || typeof value !== "object" || Array.isArray(value)) return [value];
    if (value.now === true) return [now()];
    if (typeof value.from !== "string") return [value];
    const given = args[value.from] ?? value.absent ?? null;
    if (!value.map) return [given];
    const word = typeof given === "string" ? given : JSON.stringify(given);
    return word in value.map ? valueOf(value.map[word], args) : [];
  };
  const setFields = (fields, set, args) => { for (const [field, v] of Object.entries(set ?? {})) { const got = valueOf(v, args); if (got.length) fields[field] = got[0]; } };
  return {
    /** The listing's own forms over the call's `fields` before it is listed: the refusal's code, or null. */
    listing(list, fields) {
      if (!["ends", "refuse", "with", "stamp", "absent"].some(k => k in list)) return null;
      const state = list.state ?? "listed";
      prune(state);
      if (list.refuse && list.key && (lists.get(state) ?? []).some(e => e.fields[list.key] === (fields[list.key] ?? null))) return list.refuse;
      for (const [k, v] of Object.entries(list.absent ?? {})) if (fields[k] == null) fields[k] = v;
      Object.assign(fields, list.with ?? {});
      if (list.stamp) fields[list.stamp] = now();
      return null;
    },
    /** `"update"` on a send: null, or `{error}`. */
    update(name, update, args) {
      const state = update.state ?? "listed", key = update.key ?? "", code = update.else ?? "NotFoundError", value = args[key] ?? null;
      const entry = (lists.get(state) ?? []).find(e => e.fields[key] === value);
      if (!entry) { say(`${name}: refused: ${code} (no entry ${JSON.stringify(value)})`); return { error: code }; }
      if (update.when && !matches(entry.fields, update.when)) { say(`${name}: refused: ${code} (${JSON.stringify(value)} is not ${JSON.stringify(update.when)})`); return { error: code }; }
      entry.fields = { ...entry.fields };
      setFields(entry.fields, update.set, args);
      say(`${name}: updated`);
      changed(state);
      return null;
    },
    /** Whether the entry's list declares choices: its tap then takes one. */
    chooses: entry => !!config(entry.row.sub.list.state).tap?.choices,
    /** `tap @ref <choice>`: the person's act on the system's screen; the entry stays listed. */
    choose(entry, choice) {
      const state = entry.row.sub.list.state, choices = config(state).tap?.choices;
      if (!choices) return { error: `@${entry.ref} takes no choice: tap @${entry.ref} alone` };
      const chosen = choices[choice];
      if (!chosen) return { error: `@${entry.ref} takes ${Object.keys(choices).join(", ")}, not \`${choice}\`` };
      if (chosen.when && !matches(entry.fields, chosen.when)) {
        const now = Object.keys(chosen.when).map(f => `its ${f} is ${JSON.stringify(entry.fields[f] ?? null)}`);
        return { error: `@${entry.ref}: \`${choice}\` is refused: ${now.join(", ")}` };
      }
      entry.fields = { ...entry.fields };
      setFields(entry.fields, chosen.set, {});
      say(`${state} @${entry.ref}: ${choice}`);
      changed(state);
      if (typeof chosen.event !== "string") return { ref: entry.ref, choice, delivery: "substituted" };
      raise(entry.row, chosen.event, chosen, entry.fields);
      return { ref: entry.ref, choice, event: chosen.event, delivery: "substituted" };
    },
  };
}
