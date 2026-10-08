import type { Storage } from "../../../vendor/ibex/crates/ibex2/src/bindings/storage";
type Store = { get(name:string):string|null; set(name:string,value:string):void; forget(name:string):void };
const appId = "dev.exact.storage-test";
const grants = "fs.read app:/data\nfs.write app:/data\nfs.read doc:/\nfs.write doc:/\nsqlite.open app:/data/notes.db\nnet.fetch https://example.test\nsecret.keep session\n";

// Work queued behind whatever the module started last, the way an app serializes its
// database operations. The second answer's storage calls run in a later microtask.
let tail: Promise<unknown> = Promise.resolve();
let saves: Promise<unknown> = Promise.resolve(), saveError = "";
let shared: Promise<{text:string}>;

// Background work (LLP 1097): the last save started and not awaited, and
// the last failure a background step reported to the app.
let saving: Promise<unknown> = Promise.resolve(), lastError = "";
const bytes = (value: string) => new Uint8Array(Array.from(value).map(c => c.charCodeAt(0)));

// Writes an answer starts and does not await (kanban F22): the answer is
// given before they land, and they must land all the same.
function answer(source:string, args:unknown[], store:Store, storage:Storage, native:any) {
  const op = String(args[0]), value = String(args[1]);
  if (op === "shared-live") {
    shared = fetch("https://example.test/shared").then(async () => {
      await storage.fs.atomicWriteFile(storage.fs.directories.data + "/shared", new Uint8Array([1]));
      store.set("session", value);
      return {text: value};
    });
    return shared;
  }
  if (op === "shared-wait") return shared.then(() => ({text: "waited"}));
  const song = storage.fs.directories.data + "/song";
  // The summary's edit: answered from memory, saved unawaited, in the answer.
  if (op === "save") {
    saving = storage.fs.atomicWriteFile(song, bytes(value)).catch((e) => { lastError = String(e.code); });
    return {text:"saved " + value};
  }
  // The same save chained behind the last one: issued when that lands.
  if (op === "save-chained") {
    saving = saving.then(() => storage.fs.atomicWriteFile(song, bytes(value))).catch((e) => { lastError = String(e.code); });
    return {text:"saved " + value};
  }
  // An answer that awaits background work: waits with the module.
  if (op === "await-saving") return saving.then(() => ({text:"after " + lastError}));
  // 258 writes at once: one in flight, 256 waiting, the 258th refused.
  if (op === "flood") {
    const codes: Promise<string>[] = [];
    for (let i = 0; i < 258; i++) codes.push(storage.fs.writeFile(storage.fs.directories.data + "/flood", bytes(String(i))).then(() => "ok", (e) => String(e.code)));
    return codes[257].then((code) => ({text:code}));
  }
  // What a background step may not do: fetch, or call the native module.
  if (op === "save-then-fetch") {
    storage.fs.atomicWriteFile(song, bytes(value)).then(() => fetch("https://example.test/later")).catch((e) => { lastError = e.message; });
    return {text:"saved " + value};
  }
  if (op === "save-then-native") {
    storage.fs.atomicWriteFile(song, bytes(value)).then(() => native.call({value})).catch((e) => { lastError = e.message; });
    return {text:"saved " + value};
  }
  // A background write that fails: its promise rejects, as on the web.
  if (op === "save-bad") {
    storage.fs.writeFile(storage.fs.directories.data + "/absent/file", bytes(value)).catch((e) => { lastError = String(e.code); });
    return {text:"saved " + value};
  }
  if (op === "reject") { Promise.reject(new Error("lost " + value)); return {text:"answered"}; }
  // A database opened by the answer and left open by its chain, which throws (LLP 1097 D7).
  if (op === "open-leak") {
    storage.sqlite.open("app:/data/notes.db").then(() => { throw new Error("leaked " + value); });
    return {text:"answered"};
  }
  if (op === "last-error") return {text:lastError};
  // Each part appended to one log, awaited in turn: two such answers
  // interleave at their awaits, as two async calls do on the web (D4.5).
  if (op === "append") {
    return (async () => {
      for (const part of value.split(",")) await storage.fs.appendFile(storage.fs.directories.data + "/log", bytes(part + ";"));
      return {text:value};
    })();
  }
  if (op === "unawaited") {
    storage.fs.atomicWriteFile(storage.fs.directories.data + "/unawaited", new Uint8Array(Array.from(value).map(c=>c.charCodeAt(0))));
    return {text:"answered"};
  }
  // drums R10: a value given at once, its save chained behind a promise
  // already resolved, so the write begins in the checkpoint after the call.
  if (op === "deferred") {
    saves = saves.then(() => storage.fs.atomicWriteFile(storage.fs.directories.data + "/deferred", new Uint8Array(Array.from(value).map(c=>c.charCodeAt(0)))))
      .catch((e) => { saveError = String(e); });
    return {text:"answered" + saveError};
  }
  if (op === "queued" || op === "queued-sync") {
    tail = tail.then(async () => {
      await storage.fs.mkdir(storage.fs.directories.data + "/queued");
      await storage.fs.atomicWriteFile(storage.fs.directories.data + "/queued/file", new Uint8Array(Array.from(value).map(c=>c.charCodeAt(0))));
    });
    return op === "queued-sync" ? {text:"answered"} : Promise.resolve({text:"answered"});
  }
  return work(source, args, store, storage, native);
}

async function work(_source:string, args:unknown[], store:Store, storage:Storage, native:{available:boolean; call(request:Record<string,unknown>):Record<string,unknown>; later(request:Record<string,unknown>):Promise<Record<string,unknown>>}|null) {
  const op = String(args[0]), value = String(args[1]);
  if (op === 'later') {
    if (!native?.available) return {text: 'no native module'};
    try { return {text:String((await native!.later({value})).text)}; }
    catch(error:any) { return {text:'refused: ' + error.message}; }
  }
  if (op === 'native' || op === 'native-fetch') {
    if (!native?.available) return {text: 'no native module'};
    try {
      if(op === 'native-fetch') await fetch('https://example.test/native');
      return {text:String(native!.call({value}).text)};
    } catch(error:any) { return {text:error.message}; }
  }
  if (op === "placeholder") return {text: ""};
  // A folder the person chose (LLP 1069.010 D1), `value` its `doc:` path:
  // listed, read, written beside, and refused past what it holds.
  if (op === "doc") {
    const out: string[] = [], text = (b: ArrayBuffer) => String.fromCharCode(...new Uint8Array(b));
    for (const name of await storage.fs.readdir(value)) {
      const stat = await storage.fs.stat(value + "/" + name);
      out.push(name + (stat.isDirectory ? "/" + stat.size : "=" + text(await storage.fs.readFile(value + "/" + name))));
    }
    await storage.fs.writeFile(value + "/new.txt", new Uint8Array([104, 105]));
    out.push("new=" + text(await storage.fs.readFile(value + "/new.txt")));
    await storage.fs.rm(value + "/new.txt");
    const steps: (() => Promise<unknown>)[] = [
      () => storage.fs.readFile(value + "/absent"),
      () => storage.fs.readFile(value + "/../escape"),
      () => storage.fs.readFile(value + "/sub"),
      () => storage.fs.readdir(value + "/a.txt"),
      () => storage.fs.rm(value + "/sub"),
      () => storage.fs.rm(value),
      () => storage.fs.rename(value + "/a.txt", value + "/b.txt"),
      () => storage.fs.readFile("doc:/999999/a.txt"),
      () => storage.fs.readFile("app:/data/note"),
    ];
    for (const step of steps) {
      try { await step(); out.push("ok"); } catch (e: any) { out.push(e.kind + " " + e.code); }
    }
    return {text: out.join(" ")};
  }
  // Ledger's shape (ledger F12): every answer queued on one chain, a listing
  // that reads, and a save that refuses bad input before touching storage.
  if (op === "count" || op === "invalid") {
    const run = tail.then(async () => {
      if (op === "invalid") return {text: "invalid"};
      const db = await storage.sqlite.open("app:/data/notes.db");
      try {
        await db.execute("CREATE TABLE IF NOT EXISTS notes (body TEXT UNIQUE)");
        const rows = await db.query("SELECT count(*) FROM notes");
        return {text: value + ":" + String(rows.rows[0][0])};
      } finally { await db.close(); }
    });
    tail = run.catch(() => {});
    return run;
  }
  if (op === "serial") {
    const run = tail.then(async () => {
      const db = await storage.sqlite.open("app:/data/notes.db");
      try {
        await db.execute("CREATE TABLE IF NOT EXISTS notes (body TEXT UNIQUE)");
        await db.execute("INSERT OR IGNORE INTO notes VALUES (?)", [value]);
        const rows = await db.query("SELECT count(*) FROM notes");
        return {text: value + ":" + String(rows.rows[0][0])};
      } finally { await db.close(); }
    });
    tail = run.catch(() => {});
    return run;
  }
  const path = storage.fs.directories.data + "/note";
  // LLP 1027.005's fixture: file identity selects the cached status; supplied
  // time decides whether that answer needs fetching again, without ambient time.
  if (op === "status") {
    let saved: {text:string; expires:number};
    try {
      const bytes = await storage.fs.readFile(storage.fs.directories.data + "/status-" + value);
      saved = JSON.parse(String.fromCharCode(...new Uint8Array(bytes)));
    } catch (_) { return {text:"empty"}; }
    const minute = Number(args[2]);
    if (minute <= saved.expires) return {text:saved.text};
    const result = await fetch("https://example.test/status/" + value + "?minute=" + minute);
    return {text:await result.text()};
  }
  if (op === "ordered") {
    const ordered = storage.fs.directories.data + "/ordered";
    let before = "";
    try { before = String.fromCharCode(...new Uint8Array(await storage.fs.readFile(ordered))); } catch (_) {}
    await storage.fs.atomicWriteFile(ordered, new Uint8Array(Array.from(before + value).map(c => c.charCodeAt(0))));
    store.set("session", value);
    return {text: value};
  }
  if (op === "file" || op === "file-kept") {
    await storage.fs.atomicWriteFile(path, new Uint8Array(Array.from(value).map(c=>c.charCodeAt(0))));
    const bytes = new Uint8Array(await storage.fs.readFile(path));
    if (op === "file-kept") store.set("session", value);
    return { text: String.fromCharCode(...bytes) };
  }
  if (op === "library") {
    try { return {text:String.fromCharCode(...new Uint8Array(await storage.fs.readFile(path)))}; }
    catch (_) { return {text:"empty"}; }
  }
  // Refusals carry a stable code beside their message (kanban F28).
  if (op === "codes") {
    const data = storage.fs.directories.data, out: string[] = [];
    const steps: (() => Promise<unknown>)[] = [
      () => storage.fs.readFile(data + "/absent"),
      () => storage.fs.writeFile("app:/cache/no", new Uint8Array([1])),
      () => storage.fs.readdir(data + "/note"),
      () => storage.fs.mkdir(data + "/full").then(() => storage.fs.writeFile(data + "/full/x", new Uint8Array([1]))).then(() => storage.fs.rm(data + "/full")),
      () => storage.sqlite.open("app:/data/other.db"),
    ];
    for (const step of steps) {
      try { await step(); out.push("ok"); } catch (e:any) { out.push(e.kind + " " + e.code + " " + e.message); }
    }
    return {text: out.join("\n")};
  }
  // `fs.compressImage` (LLP 1069.002 A1): the record, then each refusal's
  // name and code, in one answer.
  if (op === "compress") {
    const data = storage.fs.directories.data, out: string[] = [];
    try {
      const r = await storage.fs.compressImage(data + "/in.jpg", data + "/out.jpg", { maxDimension: Number(value), maxBytes: 2_000_000 });
      const back = await storage.fs.readFile(r.path);
      out.push([r.path, r.type, r.size, r.width, r.height, back.byteLength].join(" "));
    } catch (e:any) { out.push(e.name + " " + e.code + " " + e.message); }
    const steps: (() => Promise<unknown>)[] = [
      () => storage.fs.compressImage(data + "/in.jpg", "app:/tmp/out.jpg", { maxDimension: 10, maxBytes: 10 }),
      () => storage.fs.compressImage(data + "/absent.jpg", data + "/x.jpg", { maxDimension: 10, maxBytes: 10000 }),
      () => storage.fs.compressImage(data + "/note", data + "/x.jpg", { maxDimension: 10, maxBytes: 10000 }),
      () => storage.fs.compressImage(data + "/in.jpg", data + "/x.jpg", { maxDimension: 64, maxBytes: 1 }),
      () => storage.fs.compressImage(data + "/in.jpg", "doc:/1/x.jpg", { maxDimension: 10, maxBytes: 10 }),
      () => storage.fs.compressImage(data + "/in.jpg", data + "/x.jpg", { maxDimension: 0, maxBytes: 10 }),
      () => (storage.fs.compressImage as any)(data + "/in.jpg", data + "/x.jpg"),
    ];
    for (const step of steps) {
      try { await step(); out.push("ok"); } catch (e:any) { out.push(e.name + " " + e.code + " " + e.message); }
    }
    return {text: out.join("\n")};
  }
  // A long read (files F18: a folder's preview walking its tree), one
  // storage step after another in one answer.
  if (op === "walk") {
    let steps = 0;
    for (let i = 0; i < 24; i++) { await storage.fs.readdir(storage.fs.directories.data); steps++; }
    return {text: value + ":" + steps};
  }
  // An independent read, on no chain (minesweeper F10).
  if (op === "peek") {
    try { return {text: value + ":" + String.fromCharCode(...new Uint8Array(await storage.fs.readFile(storage.fs.directories.data + "/note")))}; }
    catch (_) { return {text: value + ":empty"}; }
  }
  if (op === "read-at") return {text:String.fromCharCode(...new Uint8Array(await storage.fs.readFile(storage.fs.directories.data + "/" + value)))};
  if (op === "read") return {text:String.fromCharCode(...new Uint8Array(await storage.fs.readFile(path)))};
  if (op === "refused") {
    try { await storage.fs.writeFile("app:/cache/no", new Uint8Array([1])); }
    catch (e) { return {text:"denied"}; }
    return {text:"leaked"};
  }
  if (op === "sqlite-refused") {
    try { await storage.sqlite.open("app:/data/notes-bob.db"); }
    catch (e:any) { return {text:e.code + " " + e.message}; }
    return {text:"leaked"};
  }
  if (op === "bake") {
    try { await storage.fs.readFile(path); }
    catch (e:any) { return {text:e.kind + ":" + e.code + ":" + e.message}; }
    return {text:"read at bake"};
  }
  if (op === "fetch") {
    await storage.fs.atomicWriteFile(path + value, new Uint8Array([1]));
    const response = await fetch("https://example.test/" + value);
    const text = await response.text();
    await storage.fs.atomicWriteFile(path + value, new Uint8Array([2]));
    store.set("session", value);
    return {text:value + ":" + text};
  }
  const db = await storage.sqlite.open("app:/data/notes.db");
  try {
    await db.execute("CREATE TABLE IF NOT EXISTS notes (body TEXT UNIQUE)");
    if (op === "add") {
      const insert = await db.prepare("INSERT INTO notes VALUES (?)");
      try { await insert.execute([value]); } finally { await insert.close(); }
    }
    if (op === "rollback") {
      try { await db.transaction([{sql:"INSERT INTO notes VALUES (?)",params:[value]}, {sql:"INSERT INTO notes VALUES (?)",params:[value]}]); }
      catch (_) { /* verify the rolled-back value is absent below */ }
    }
    if (op === "types") {
      const rows = await db.query("SELECT ?, ?, ?, ?", [9223372036854775807n, -9223372036854775808n, 1.25, new Uint8Array([0,255])]);
      const [max,min,real,blob]=rows.rows[0];
      if(typeof max!=="bigint"||typeof min!=="bigint"||!(blob instanceof Uint8Array))throw new Error("SQL value types changed");
      return {text:[max,min,real,Array.from(blob).join(",")].join("/")};
    }
    if (op === "sql-refusals") {
      const kinds=[];
      for(const sql of ["INSERT INTO notes VALUES ('write from query')", "ATTACH DATABASE '/tmp/escape' AS other", "PRAGMA writable_schema=ON", "SELECT 1; SELECT 2"]) {
        try { await db.query(sql); kinds.push("allowed"); } catch(e:any) { kinds.push(e.kind); }
      }
      return {text:kinds.join("/")};
    }
    const rows = await db.query("SELECT body FROM notes ORDER BY body");
    return {text: rows.rows.map(row=>String(row[0])).join(",")};
  } finally { await db.close(); }
}
(globalThis as any).exact = {abi:1, appId, grants, answer};
