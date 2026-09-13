import type { Database, SQLCommand, Storage } from './app.contract.d.ts';
import { MemoryStore, type Row, type SchemaJson, type Store, type StoreTx } from './vendor/snapback4/store';

// Exact's SQLite capability offers atomic statement batches, without keeping a
// transaction open across JS callbacks. The upstream indexed memory store gives
// the interpreter read-your-writes; its successful writes commit in one batch.
// A refusal or failed commit reloads the last durable image before another call.
export class ReplicaStore implements Store {
  private memory: MemoryStore;
  private constructor(private db: Database, public schema: SchemaJson) {
    this.memory = new MemoryStore(schema);
  }
  static async open(storage: Storage, path: string, schema: SchemaJson): Promise<ReplicaStore> {
    const db = await storage.sqlite.open(path);
    try {
      await db.transaction([
        {sql:'CREATE TABLE IF NOT EXISTS replica_rows (t TEXT NOT NULL,id TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(t,id)) WITHOUT ROWID'},
        {sql:'CREATE TABLE IF NOT EXISTS replica_meta (key TEXT PRIMARY KEY,value TEXT NOT NULL)'},
        {sql:'CREATE TABLE IF NOT EXISTS replica_side (name TEXT NOT NULL,key TEXT NOT NULL,value TEXT NOT NULL,PRIMARY KEY(name,key)) WITHOUT ROWID'},
        {sql:"INSERT OR IGNORE INTO replica_meta VALUES ('exact:device',json_quote(lower(hex(randomblob(16)))))"},
      ]);
      const store = new ReplicaStore(db,schema); await store.reload(); return store;
    } catch(error) {await db.close();throw error;}
  }
  setSchema(schema: SchemaJson) { this.schema=schema;this.memory.setSchema(schema); }
  private async reload(): Promise<void> {
    const memory = new MemoryStore(this.schema);
    const rows = await this.db.query('SELECT t,data FROM replica_rows');
    const meta = await this.db.query('SELECT key,value FROM replica_meta');
    const sides = await this.db.query('SELECT name,key,value FROM replica_side');
    await memory.write(async tx => {
      for(const [table,data] of rows.rows) await tx.put(String(table),JSON.parse(String(data)) as Row);
      for(const [key,value] of meta.rows) await tx.setMeta(String(key),JSON.parse(String(value)));
      for(const [name,key,value] of sides.rows) await tx.side(String(name)).put(String(key),JSON.parse(String(value)));
    });
    this.memory=memory;
  }
  read<T>(body:(tx:StoreTx)=>Promise<T>):Promise<T> { return this.memory.read(body); }
  async write<T>(body:(tx:StoreTx)=>Promise<T>):Promise<T> {
    const commands:SQLCommand[]=[];
    try {
      const result=await this.memory.write(tx=>body({...tx,
        put:async(table,row)=>{
          await tx.put(table,row);
          commands.push({sql:'INSERT INTO replica_rows(t,id,data) VALUES(?,?,?) ON CONFLICT(t,id) DO UPDATE SET data=excluded.data',params:[table,row.id,JSON.stringify(row)]});
        },
        delete:async(table,id)=>{await tx.delete(table,id);commands.push({sql:'DELETE FROM replica_rows WHERE t=? AND id=?',params:[table,id]});},
        setMeta:async(key,value)=>{await tx.setMeta(key,value);commands.push({sql:'INSERT INTO replica_meta(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value',params:[key,JSON.stringify(value)]});},
        side:name=>{
          const side=tx.side(name);
          return {...side,
            put:async(key,value)=>{await side.put(key,value);commands.push({sql:'INSERT INTO replica_side(name,key,value) VALUES(?,?,?) ON CONFLICT(name,key) DO UPDATE SET value=excluded.value',params:[name,key,JSON.stringify(value)]});},
            delete:async key=>{await side.delete(key);commands.push({sql:'DELETE FROM replica_side WHERE name=? AND key=?',params:[name,key]});},
            clear:async()=>{await side.clear();commands.push({sql:'DELETE FROM replica_side WHERE name=?',params:[name]});},
          };
        },
      }));
      if(commands.length)await this.db.transaction(commands);
      return result;
    } catch(error) {await this.reload();throw error;}
  }
  async clearRows():Promise<void> {await this.db.execute('DELETE FROM replica_rows');await this.memory.clearRows();}
  close():Promise<void> {return this.db.close();}
}
