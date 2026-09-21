import type { Answer, Sources, Storage, Database, Result } from './app.contract.d.ts';

type Library = Result<'library'>;
type Note = Library['notes'][number];
type Saved = Result<'saveNote'>;
type Status = Result<'backupNotes'>;
type Store = Parameters<Answer>[2];

export const appId = 'com.exact.fieldnotes';
export const grants = 'sqlite.open app:/data/fieldnotes.db\nfs.read app:/data/backups\nfs.write app:/data/backups\nsecret.keep fieldnotes.revision';
const backupPath = 'app:/data/backups/fieldnotes.json';
// One app-owned revision survives source/language replacement with the Store.
function nextRevision(store:Store):number {
  const raw=store.get('fieldnotes.revision');
  const previous=raw&&/^[0-9]+$/.test(raw)?Number(raw):0;
  const next=Number.isSafeInteger(previous)&&previous<Number.MAX_SAFE_INTEGER?previous+1:1;
  store.set('fieldnotes.revision',String(next));return next;
}
// Serialize whole app operations, not just individual database statements.
// A save and the resource refresh it triggers never contend for the same file.
let tail: Promise<unknown> = Promise.resolve();
function serial<T>(work: () => Promise<T>): Promise<T> {
  const result = tail.then(work); tail = result.catch(() => {}); return result;
}
function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
async function database(storage: Storage): Promise<Database> {
  const db = await storage.sqlite.open('app:/data/fieldnotes.db');
  try {
    // On web, even a no-op execute exports the complete database. Check each
    // opened file instead of caching readiness across replacement or deletion.
    const schema = await db.query("SELECT 1 FROM sqlite_schema WHERE type='table' AND name='notes'");
    if (!schema.rows.length) await db.execute('CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, body TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0)');
    return db;
  } catch (error) { await db.close(); throw error; }
}
async function withDatabase<T>(storage: Storage, work: (db: Database) => Promise<T>): Promise<T> {
  const db = await database(storage);
  try { return await work(db); } finally { await db.close(); }
}
function excerpt(body:string):string {
  for(let end=100;;end*=2) {
    const text=body.slice(0,end).replace(/\s+/g,' ');
    // Do not split an astral character at the UTF-16 preview boundary.
    if(text.length>=100||end>=body.length)return text.slice(0,100).replace(/[\uD800-\uDBFF]$/,'')||'An empty page.';
  }
}
// Full bodies stay within one small storage reply and leave the library as previews.
// Scan by immutable ID so another window pinning a note cannot duplicate it.
async function notes(db: Database, query: string): Promise<{notes:Note[];total:number}> {
  const needle=query.toLowerCase(), found:Note[]=[];
  let cursor=9223372036854775807n, total=0;
  for (;;) {
    const result=await db.query('SELECT id, title, body, pinned FROM notes WHERE id <= ? ORDER BY id DESC LIMIT 32',[cursor]);
    for(const row of result.rows) {
      const title=String(row[1]),body=String(row[2]);total++;
      if(!needle||(title+'\n'+body).toLowerCase().includes(needle)) {
        found.push({id:String(row[0]),title,pinned:row[3]===1n,excerpt:excerpt(body)});
      }
    }
    if(result.rows.length<32)break;
    cursor=(result.rows[result.rows.length-1][0] as bigint)-1n;
  }
  // Stable sort keeps descending IDs within each pin group, without Number rounding.
  found.sort((a,b)=>Number(b.pinned)-Number(a.pinned));
  return {notes:found,total};
}
async function openNote(noteId:string,version:number,storage:Storage):Promise<Result<'openNote'>> {
  const empty={version,title:'',body:'',pinned:false,message:'',ready:true};
  if(!noteId)return empty;
  try {
    return await withDatabase(storage,async db=>{
      const result=await db.query('SELECT title, body, pinned FROM notes WHERE id=?',[id(noteId)]);
      if(!result.rows.length)throw new Error('This note was deleted. Choose another note or start a new one.');
      const row=result.rows[0];
      return {version,title:String(row[0]),body:String(row[1]),pinned:row[2]===1n,message:'',ready:true};
    });
  } catch(error) {return {...empty,message:message(error),ready:false};}
}
function id(value: string): bigint {
  if (!/^[1-9][0-9]*$/.test(value)) throw new Error('That note is not available.');
  const n = BigInt(value); if(n > 9223372036854775807n) throw new Error('Invalid note ID.'); return n;
}
function validate(title: string, body: string): void {
  if (title.length > 160) throw new Error('Keep titles under 160 characters.');
  if (body.length > 20000) throw new Error('Keep each note under 20,000 characters.');
}
// @ref LLP 1027.001 D1 — standard UTF-8 on every executor
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
function encode(text: string): Uint8Array { return encoder.encode(text); }
function decode(bytes: ArrayBuffer): string { return decoder.decode(bytes); }
type BackupNote = {id:string;title:string;body:string;pinned:boolean};
function parseBackup(text: string): BackupNote[] {
  if(text.length>4*1024*1024)throw new Error('Backups must be smaller than 4 MB.');
  const value: unknown = JSON.parse(text);
  if(!value || typeof value!=='object' || !('version' in value) || value.version!==1 || !('notes' in value) || !Array.isArray(value.notes) || value.notes.length>1000)throw new Error('This is not a Fieldnotes backup.');
  const seen=new Set<string>();
  return value.notes.map((n:unknown)=>{
    if(!n || typeof n!=='object' || !('id' in n) || typeof n.id!=='string' || !('title' in n) || typeof n.title!=='string' || !('body' in n) || typeof n.body!=='string' || !('pinned' in n) || typeof n.pinned!=='boolean')throw new Error('The backup contains an invalid note.');
    id(n.id);validate(n.title,n.body);
    if(seen.has(n.id))throw new Error('The backup contains duplicate note IDs.');seen.add(n.id);
    return {id:n.id,title:n.title,body:n.body,pinned:n.pinned};
  });
}
async function library(query: string, storage: Storage): Promise<Library> {
  try {
    const result = await withDatabase(storage,db=>notes(db,query));
    return {...result,message:'',ready:true};
  } catch (error) {
    const why=message(error);
    return {notes:[],total:0,message:why.includes('unsupported by this host')||why.includes('during bake') ? 'Opening your notebook…' : 'Could not open your notebook: '+why,ready:false};
  }
}
async function saveNote(noteId:string,title:string,body:string,pinned:boolean,version:number,storage:Storage,store:Store): Promise<Saved> {
  try {
    validate(title,body);if(!title.trim()&&!body.trim())throw new Error('Write something before saving.');
    const savedId=await withDatabase(storage,async db=>{
      if(noteId){
        const result=await db.execute('UPDATE notes SET title=?,body=?,pinned=? WHERE id=?',[title.trim()||'Untitled note',body,pinned?1n:0n,id(noteId)]);
        if(result.changes!==1)throw new Error('This note was deleted. Start a new note to keep this draft.');
        return noteId;
      }
      const count=await db.query('SELECT count(*) FROM notes');
      if((count.rows[0][0] as bigint)>=1000n)throw new Error('This notebook holds up to 1,000 notes. Back up and remove some before adding more.');
      const result=await db.execute('INSERT INTO notes (title,body,pinned) VALUES (?,?,?)',[title.trim()||'Untitled note',body,pinned?1n:0n]);
      return String(result.lastInsertRowid);
    });
    return {id:savedId,version,title,body,pinned,revision:nextRevision(store),message:'Saved on this device.',failed:false};
  } catch(error) {return {id:noteId,version,title,body,pinned,revision:nextRevision(store),message:message(error),failed:true};}
}
async function service(source:string,args:unknown[],storage:Storage,store:Store): Promise<Status> {
  try {
    let text='',notice='';
    if(source==='backupNotes') {
      let count=0;
      text=await withDatabase(storage,async db=>{
        const parts:string[]=[];
        const prefix='{\n  "version": 1,\n  "notes": [\n', suffix='\n  ]\n}';
        let length=prefix.length+suffix.length;
        // One SQLite statement preserves a snapshot across other windows' writes.
        // A valid 4 Mi UTF-16 backup has at most 12 MiB of quoted UTF-8 text.
        const result=await db.query('WITH sized AS (SELECT id, title, body, pinned, SUM(length(CAST(json_quote(title) AS BLOB)) + length(CAST(json_quote(body) AS BLOB))) OVER () AS backup_bytes FROM notes) SELECT id, CASE WHEN backup_bytes <= 12582912 THEN title END, CASE WHEN backup_bytes <= 12582912 THEN body END, pinned FROM sized ORDER BY pinned DESC, id DESC');
        for(const row of result.rows){
          if(row[1]===null||row[2]===null)throw new Error('This backup exceeds 4 MB. Split or remove large notes before backing up.');
          // These fields are scalars: emit their final six-space indentation
          // directly, instead of scanning every body again to indent its lines.
          const note=JSON.stringify({id:String(row[0]),title:String(row[1]),body:String(row[2]),pinned:row[3]===1n},null,6);
          const part='    '+note.slice(0,-1)+'    }';
          length+=part.length+(count?2:0);
          if(length>4*1024*1024)throw new Error('This backup exceeds 4 MB. Split or remove large notes before backing up.');
          parts.push(part);count++;
        }
        return count?prefix+parts.join(',\n')+suffix:JSON.stringify({version:1,notes:[]},null,2);
      });
      await storage.fs.mkdir('app:/data/backups');
      await storage.fs.atomicWriteFile(backupPath,encode(text));
      notice=`Backup saved with ${count} notes.`;
    } else if(source==='readBackup') {
      text=decode(await storage.fs.readFile(backupPath));parseBackup(text);notice='Your saved backup is ready to copy.';
    } else if(source==='restoreNotes') {
      text=String(args[0]);if(!text.trim())text=decode(await storage.fs.readFile(backupPath));
      const restored=parseBackup(text);
      await withDatabase(storage, db=>db.transaction([{sql:'DELETE FROM notes'},...restored.map(n=>({sql:'INSERT INTO notes (id,title,body,pinned) VALUES (?,?,?,?)',params:[id(n.id),n.title,n.body,n.pinned?1n:0n]}))]));
      notice=`Restored ${restored.length} notes.`;
    } else if(source==='deleteNote') {
      await withDatabase(storage,db=>db.execute('DELETE FROM notes WHERE id=?',[id(String(args[0]))]));notice='Note deleted.';
    } else throw new Error('Unknown notebook action.');
    return {revision:nextRevision(store),message:notice,failed:false,backupText:text};
  } catch(error) {return {revision:nextRevision(store),message:message(error),failed:true,backupText:''};}
}
const sources: Sources = {
  library: ([query], _store, storage) => serial(() => library(query, storage)),
  // Native reads own their continuation; browser executors already hold a
  // whole storage-backed turn through close. Do not await another app call.
  openNote: ([noteId, version], _store, storage) => openNote(noteId, version, storage),
  saveNote: (args, store, storage) => serial(() => saveNote(...args, storage, store)),
  backupNotes: (args, store, storage) => serial(() => service('backupNotes', args, storage, store)),
  readBackup: (args, store, storage) => serial(() => service('readBackup', args, storage, store)),
  restoreNotes: (args, store, storage) => serial(() => service('restoreNotes', args, storage, store)),
  deleteNote: (args, store, storage) => serial(() => service('deleteNote', args, storage, store)),
};
export const answer: Answer = (source,args,store,storage) => sources[source](args,store,storage);
