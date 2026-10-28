// An arrival in the declared substitute on the JS target (LLP 1133.035 D16
// item 1, §10), as the runner's `modules/arrival.rs`: an agent word that
// stands for something arriving. The interface's `"substitute": {"arrival":
// …}` names the field word, the fixtures' word, and the names an arrival
// reaches. Setting the field to a fixture tag's id brings that tag into the
// field: a waiting write or lock settles against it (no reading is raised),
// else each waiting scan hears `reading` (or `readingerror` for a tag with
// no NDEF) and each `once` scan answers `read`. A newer write replaces the
// waiting one; leaving the foreground fails a write and suspends a scan
// (the web's `readerUI` is `app`).

const textual = t => t === "text" || t === "url" || t === "absolute-url";

const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const base64 = bytes => {
  let s = "";
  for (let i = 0; i < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | ((bytes[i + 1] ?? 0) << 8) | (bytes[i + 2] ?? 0);
    s += B64[(n >> 18) & 63] + B64[(n >> 12) & 63] + (i + 1 < bytes.length ? B64[(n >> 6) & 63] : "=") + (i + 2 < bytes.length ? B64[n & 63] : "=");
  }
  return s;
};
const unbase64 = text => {
  const s = String(text).replace(/=+$/, ""), out = [];
  let bits = 0, n = 0;
  for (const c of s) { const v = B64.indexOf(c); if (v < 0) continue; n = (n << 6) | v; bits += 6; if (bits >= 8) { bits -= 8; out.push((n >> bits) & 255); } }
  return out;
};
const bytesOf = r => typeof r.data === "string" ? [...new TextEncoder().encode(r.data)] : typeof r.data?.base64 === "string" ? unbase64(r.data.base64) : [];

/** A fixture record as Contract reads it (`NDEFRecordInfo`). */
const recordInfo = r => {
  const type = r.recordType ?? "empty";
  const text = textual(type) ? (typeof r.data === "string" ? r.data : null)
    : type === "smart-poster" ? (r.records ?? []).find(x => x.recordType === "url")?.data ?? null : null;
  const info = { recordType: type, text, size: bytesOf(r).length, mediaType: r.mediaType ?? null, id: r.id ?? null, encoding: r.encoding ?? null, lang: r.lang ?? null };
  if (type === "text" && info.encoding == null) info.encoding = "utf-8";
  return info;
};
/** A fixture record as data code reads it (`NDEFRecord`). */
// A record's own records go one level deep (`NDEFInnerRecord`).
const withBytes = r => ({ ...recordInfo(r), data: { base64: base64(bytesOf(r)) } });
const fullRecord = r => ({ ...withBytes(r), records: Array.isArray(r.records) ? r.records.map(withBytes) : null });
const fixtureRecord = r => {
  const out = { recordType: r.recordType, data: r.data ?? null };
  for (const f of ["mediaType", "id", "encoding", "lang"]) if (r[f] != null) out[f] = r[f];
  return out;
};
const nfcTag = t => ({ id: t.id, serialNumber: t.serialNumber ?? "" });

/** Why a write's message cannot be encoded (Web NFC's "create NDEF record"), or null. */
const messageChecked = message => {
  const records = Array.isArray(message?.records) ? message.records : [];
  if (!records.length) return "a message has a record";
  for (const r of records) {
    const type = r.recordType ?? "";
    if (typeof r.data === "string" && !textual(type)) return `a \`${type}\` record's data is bytes`;
    if (typeof r.data === "string" && type !== "text" && !r.data.includes(":")) return `\`${r.data}\` is not a URL`;
    if (r.data == null && type !== "empty") return `a \`${type}\` record has data`;
  }
  return null;
};

/** The settings a claim's reader runs with (D6), a word left out being the default. */
const settings = args => JSON.stringify({
  polling: args?.apple?.polling ?? null, technologies: args?.android?.technologies ?? null,
  skipNdefCheck: args?.android?.skipNdefCheck === true, presenceCheckDelay: args?.android?.presenceCheckDelay ?? null,
  platformSounds: args?.android?.platformSounds !== false,
});

/**
 * `rows`: every name's row ({key, send, sub}); `held`: the substitute's held calls ({row, args, waits, ticket});
 * `preferred`: the agent's values by key; `release(h, json)`; `raise(key, args)`, with the device's origin; `say`.
 */
export function arrivals({ rows, held, preferred, release, raise, say, valueOf }) {
  const extensionOf = row => row.key.slice(0, row.key.indexOf("."));
  const declaredOf = row => row.key.slice(row.key.indexOf(".") + 1);
  const arrivalOf = row => row?.sub?.arrival && typeof row.sub.arrival === "object" ? row.sub.arrival : null;
  const byExtension = new Map();
  for (const row of rows()) { const a = arrivalOf(row); if (a && !byExtension.has(extensionOf(row))) byExtension.set(extensionOf(row), a); }
  const readings = new Map(), suspended = new Set();
  let last = 0;
  const fixtures = (extension, a) => { const v = preferred.get(`${extension}.${a.from}`); return Array.isArray(v) ? v : []; };
  const claims = (extension, names) => held.filter(h => h.waits === "value" && extensionOf(h.row) === extension && names.includes(declaredOf(h.row)));

  /** A tag arrives in `extension`'s field. */
  const arrive = (extension, a, id) => {
    const tag = fixtures(extension, a).find(t => t.id === id);
    if (!tag) return;
    say(`${extension}: tag \`${id}\` arrived`);
    const writes = claims(extension, [a.write, a.lock]);
    const write = writes.pop();
    if (write) {
      for (const c of writes) release(c, { error: "AbortError" });
      const answer = written(extension, a, tag, write.args ?? {}, declaredOf(write.row) === a.lock);
      say(`${extension}.${declaredOf(write.row)}: ${answer.error ?? "written"}`);
      return release(write, answer);
    }
    const scans = claims(extension, [a.scan]).filter(c => !suspended.has(c));
    if (!scans.length) return say(`${extension}: tag \`${id}\` read by no scan`);
    if (tag.ndef === false) return raise(`${extension}.${a.readingerror}`, [nfcTag(tag)]);
    const readingId = `r${++last}`;
    const kept = readings.get(extension) ?? readings.set(extension, []).get(extension);
    kept.push([readingId, structuredClone(tag)]);
    while (kept.length > (a.hold ?? 16)) kept.shift();
    raise(`${extension}.${a.reading}`, [{ ...nfcTag(tag), id: readingId, apple: null, android: null, records: (tag.records ?? []).map(recordInfo) }]);
    for (const c of scans.filter(c => c.args?.once === true)) release(c, { reason: "read", reading: readingId, message: null });
  };

  /** D7's steps by the fixture's `write`; on success the fixture takes the records, or is locked. */
  const written = (extension, a, tag, args, lock) => {
    const fail = (error, message = null) => ({ error, message });
    if (!lock && args.overwrite === false && (tag.records ?? []).length) return fail("NotAllowedError", "overwrite");
    if (tag.ndef === false) return fail("NotSupportedError");
    switch (tag.write ?? "ok") {
      case "read-only": return fail("NetworkError", "read-only");
      case "too-small": if (!lock) return fail("NetworkError", "too-small"); break;
      case "lost": return fail("NetworkError", "lost");
      case "not-ndef": return fail("NotSupportedError");
    }
    const tags = fixtures(extension, a).map(t => t.id !== tag.id ? t : lock ? { ...t, write: "read-only" } : { ...t, records: (args.message?.records ?? []).map(fixtureRecord) });
    preferred.set(`${extension}.${a.from}`, tags);
    return { serialNumber: tag.serialNumber ?? "" };
  };

  return {
    /** What an arrival's own names decide now; undefined where the ticket decides. */
    decide(row, send, args = {}) {
      const a = arrivalOf(row);
      if (!a) return undefined;
      const extension = extensionOf(row), declared = declaredOf(row);
      if (typeof row.sub.kept === "string") {
        const id = args[row.sub.kept];
        const tag = (readings.get(extension) ?? []).find(([r]) => r === id)?.[1];
        return { now: tag ? (tag.records ?? []).map(fullRecord) : { error: "NotFoundError", message: String(id ?? "") } };
      }
      if (!send || ![a.scan, a.write, a.lock].includes(declared)) return undefined;
      if (declared === a.write) { const why = messageChecked(args.message); if (why) return { now: { error: "TypeError", message: why } }; }
      const mine = settings(args), writes = declared !== a.scan;
      const busy = claims(extension, [a.scan, a.write, a.lock]).some(c => {
        const theirs = settings(c.args);
        const hidden = (writes && JSON.parse(theirs).skipNdefCheck) || (declaredOf(c.row) !== a.scan && JSON.parse(mine).skipNdefCheck);
        return theirs !== mine || hidden;
      });
      return busy ? { now: { error: "InvalidStateError", message: "busy" } } : { hold: "value" };
    },
    /** A claim of `row` was just held: a newer write replaces the waiting one. */
    claimed(row) {
      const a = arrivalOf(row);
      if (!a || ![a.write, a.lock].includes(declaredOf(row))) return;
      const writes = claims(extensionOf(row), [a.write, a.lock]);
      writes.pop();
      for (const c of writes) { say(`${extensionOf(row)}.${declaredOf(c.row)}: replaced by a newer write`); release(c, { error: "AbortError" }); }
    },
    /** A send that `stops` the session's claims of its names; false for any other. */
    stops(row) {
      if (!Array.isArray(row.sub.stops)) return false;
      const extension = extensionOf(row);
      for (const c of claims(extension, row.sub.stops)) {
        say(`${extension}.${declaredOf(c.row)}: stopped by ${declaredOf(row)}`);
        suspended.delete(c);
        release(c, c.row.sub.stopped ?? { error: "AbortError" });
      }
      return true;
    },
    /** Why `prefer extension key=value` is refused for a field word, or null. */
    refusal(key, v, group) {
      const extension = key.slice(0, key.indexOf(".")), word = key.slice(key.indexOf(".") + 1), a = byExtension.get(extension);
      if (!a || a.field !== word || v === "clear") return null;
      const from = `${extension}.${a.from}`, tags = Object.hasOwn(group, from) ? group[from] : fixtures(extension, a);
      return (Array.isArray(tags) ? tags : []).some(t => t.id === v) ? null : `prefer extension: no tag \`${v}\` in \`${from}\``;
    },
    /** `key` moved from `before` to `after`: an arrival where it is a field word naming a tag. */
    moved(key, before, after) {
      const extension = key.slice(0, key.indexOf(".")), word = key.slice(key.indexOf(".") + 1), a = byExtension.get(extension);
      if (!a || a.field !== word || before === after) return;
      if (typeof after === "string") arrive(extension, a, after); else say(`${key}: the tag left the field`);
    },
    /** The page left the foreground or came back (D6's table, with the web's `readerUI` `app`). */
    visibility(hidden) {
      for (const [extension, a] of byExtension) {
        if (hidden) {
          for (const c of claims(extension, [a.write, a.lock])) { say(`${extension}.${declaredOf(c.row)}: aborted, the app left the foreground`); release(c, { error: "AbortError", message: "background" }); }
          const status = rows().find(r => r.key === `${extension}.${a.status}`);
          const system = status && valueOf(status)?.readerUI === "system";
          for (const c of claims(extension, [a.scan])) {
            if (system) release(c, { reason: "interrupted", reading: null, message: "background" });
            else { say(`${extension}.${declaredOf(c.row)}: suspended`); suspended.add(c); }
          }
        } else if (suspended.size) {
          suspended.clear();
          say(`${extension}: scans resumed`);
          const field = preferred.get(`${extension}.${a.field}`);
          if (typeof field === "string") arrive(extension, a, field);
        }
      }
    },
    /** A ticket's taps besides `cancel` (`taps`: a scan's `timeout`). */
    taps: row => Object.keys(row.sub.taps ?? {}),
    /** `state.extensions.<extension>`'s arrival. */
    state(extension, e) {
      const a = byExtension.get(extension);
      if (!a) return;
      e.tags = fixtures(extension, a);
      e.field = preferred.get(`${extension}.${a.field}`) ?? null;
      e.scans = claims(extension, [a.scan]).map(c => ({ ticket: c.ticket, origin: "app", once: c.args?.once === true, suspended: suspended.has(c) }));
      const w = claims(extension, [a.write, a.lock]).at(-1);
      e.write = w ? { ticket: w.ticket, name: declaredOf(w.row), origin: "app" } : null;
      e.readings = (readings.get(extension) ?? []).map(([id, tag]) => ({ id, tag: tag.id }));
    },
  };
}
