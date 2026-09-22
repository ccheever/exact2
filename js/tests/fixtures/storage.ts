import type { Storage } from "../../../vendor/ibex2/src/bindings/storage";
type Store = { get(name:string):string|null; set(name:string,value:string):void; forget(name:string):void };
const appId = "dev.exact.storage-test";
const grants = "fs.read app:/data\nfs.write app:/data\nsqlite.open app:/data/notes.db\nnet.fetch https://example.test\nsecret.keep session\n";

async function answer(_source:string, args:unknown[], store:Store, storage:Storage, native?:{call(request:Record<string,unknown>):Record<string,unknown>}|null) {
  const op = String(args[0]), value = String(args[1]);
  if (op === 'native' || op === 'native-fetch') {
    try {
      if(op === 'native-fetch') await fetch('https://example.test/native');
      return {text:String(native!.call({value}).text)};
    } catch(error:any) { return {text:error.message}; }
  }
  const path = storage.fs.directories.data + "/note";
  if (op === "file") {
    await storage.fs.atomicWriteFile(path, new Uint8Array(Array.from(value).map(c=>c.charCodeAt(0))));
    const bytes = new Uint8Array(await storage.fs.readFile(path));
    return { text: String.fromCharCode(...bytes) };
  }
  if (op === "library") {
    try { return {text:String.fromCharCode(...new Uint8Array(await storage.fs.readFile(path)))}; }
    catch (_) { return {text:"empty"}; }
  }
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
