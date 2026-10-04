import type { Storage } from "../../../vendor/ibex2/src/bindings/storage";
type Store = { get(name:string):string|null; set(name:string,value:string):void; forget(name:string):void };
const appId = "dev.exact.storage-test";
const grants = "fs.read app:/data\nfs.write app:/data\nsqlite.open app:/data/notes.db\nnet.fetch https://example.test\nsecret.keep session\n";

// Work queued behind whatever the module started last, the way an app serializes its
// database operations. The second answer's storage calls run in a later microtask.
let tail: Promise<unknown> = Promise.resolve();

// Writes an answer starts and does not await (kanban F22): the answer is
// given before they land, and they must land all the same.
function answer(source:string, args:unknown[], store:Store, storage:Storage, native:any) {
  const op = String(args[0]), value = String(args[1]);
  if (op === "unawaited") {
    storage.fs.atomicWriteFile(storage.fs.directories.data + "/unawaited", new Uint8Array(Array.from(value).map(c=>c.charCodeAt(0))));
    return {text:"answered"};
  }
  if (op === "queued") {
    tail = tail.then(async () => {
      await storage.fs.mkdir(storage.fs.directories.data + "/queued");
      await storage.fs.atomicWriteFile(storage.fs.directories.data + "/queued/file", new Uint8Array(Array.from(value).map(c=>c.charCodeAt(0))));
    });
    return Promise.resolve({text:"answered"});
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
  if (op === "file") {
    await storage.fs.atomicWriteFile(path, new Uint8Array(Array.from(value).map(c=>c.charCodeAt(0))));
    const bytes = new Uint8Array(await storage.fs.readFile(path));
    return { text: String.fromCharCode(...bytes) };
  }
  if (op === "library") {
    try { return {text:String.fromCharCode(...new Uint8Array(await storage.fs.readFile(path)))}; }
    catch (_) { return {text:"empty"}; }
  }
  if (op === "read-at") return {text:String.fromCharCode(...new Uint8Array(await storage.fs.readFile(storage.fs.directories.data + "/" + value)))};
  if (op === "read") return {text:String.fromCharCode(...new Uint8Array(await storage.fs.readFile(path)))};
  if (op === "refused") {
    try { await storage.fs.writeFile("app:/cache/no", new Uint8Array([1])); }
    catch (e) { return {text:"denied"}; }
    return {text:"leaked"};
  }
  if (op === "bake") {
    try { await storage.fs.readFile(path); }
    catch (e:any) { return {text:e.kind + ":" + e.message}; }
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
