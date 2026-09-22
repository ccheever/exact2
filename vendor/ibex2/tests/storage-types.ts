import type { Storage, SQLValue } from "../src/bindings/storage";

// Type-check the app-facing contract independently of a JS engine.
async function persist(storage: Storage): Promise<string> {
  const bytes = new Uint8Array([1, 2]);
  await storage.fs.atomicWriteFile(storage.fs.directories.data + "/note", bytes);
  const contents: ArrayBuffer = await storage.fs.readFile("app:/data/note");
  const names: string[] = await storage.fs.readdir("app:/data");
  const file: boolean = (await storage.fs.stat("app:/data/note")).isFile;
  const db = await storage.sqlite.open("app:/data/app.db");
  const insert = await db.prepare("INSERT INTO notes VALUES (?)");
  const rowid: bigint = (await insert.execute(["text"])).lastInsertRowid;
  const value: SQLValue = (await db.query("SELECT 1")).rows[0][0];
  await db.transaction([{ sql: "INSERT INTO notes VALUES (?)", params: [bytes] }]);
  await insert.close();
  await db.close();
  return `${contents.byteLength}:${names.length}:${file}:${rowid}:${value}`;
}
void persist;

function invalid(storage: Storage) {
  // @ts-expect-error strings are not byte buffers
  storage.fs.writeFile("app:/data/note", "text");
  // @ts-expect-error arbitrary objects cannot be bound as SQL parameters
  storage.sqlite.open("app:/data/app.db").then(db => db.query("SELECT ?", [{}]));
  // @ts-expect-error no ambient synchronous file access
  storage.fs.readFileSync("app:/data/note");
}
void invalid;
