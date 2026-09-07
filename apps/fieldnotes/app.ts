import type { Answer, Sources, Storage, Database, Result } from './app.contract.d.ts';

type Library = Result<'library'>;
type Note = Library['notes'][number];
type Saved = Result<'saveNote'>;
type Status = Result<'backupNotes'>;

export const appId = 'com.exact.fieldnotes';
export const grants = 'sqlite.open app:/data/fieldnotes.db\nfs.read app:/data/backups\nfs.write app:/data/backups';
const backupPath = 'app:/data/backups/fieldnotes.json';
let revision = 0;
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
    await db.execute('CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, body TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0)');
    return db;
  } catch (error) { await db.close(); throw error; }
}
async function withDatabase<T>(storage: Storage, work: (db: Database) => Promise<T>): Promise<T> {
  const db = await database(storage);
  try { return await work(db); } finally { await db.close(); }
}
async function notes(db: Database): Promise<Note[]> {
  const result = await db.query('SELECT id, title, body, pinned FROM notes ORDER BY pinned DESC, id DESC');
  return result.rows.map(row => ({id:String(row[0]),title:String(row[1]),body:String(row[2]),pinned:row[3] === 1n,
    excerpt:String(row[2]).replace(/\s+/g,' ').slice(0,100) || 'An empty page.'}));
}
function id(value: string): bigint {
  if (!/^[1-9][0-9]*$/.test(value)) throw new Error('That note is not available.');
  const n = BigInt(value); if(n > 9223372036854775807n) throw new Error('Invalid note ID.'); return n;
}
function validate(title: string, body: string): void {
  if (title.length > 160) throw new Error('Keep titles under 160 characters.');
  if (body.length > 20000) throw new Error('Keep each note under 20,000 characters.');
}
// Portable UTF-8: these app-local helpers also work in the lean native VM.
function encode(text: string): Uint8Array {
  const encoded = encodeURIComponent(text), bytes: number[] = [];
  for(let i=0;i<encoded.length;i++) {
    if(encoded[i]==='%'){bytes.push(parseInt(encoded.slice(i+1,i+3),16));i+=2;}
    else bytes.push(encoded.charCodeAt(i));
  }
  return new Uint8Array(bytes);
}
function decode(bytes: ArrayBuffer): string {
  return decodeURIComponent(Array.from(new Uint8Array(bytes), b=>'%'+b.toString(16).padStart(2,'0')).join(''));
}
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
    const all = await withDatabase(storage,notes), needle=query.toLowerCase();
    return {notes:all.filter(n=>(n.title+'\n'+n.body).toLowerCase().includes(needle)),total:all.length,message:'',ready:true};
  } catch (error) {
    const why=message(error);
    return {notes:[],total:0,message:why.includes('unsupported by this host')||why.includes('during bake') ? 'Opening your notebook…' : 'Could not open your notebook: '+why,ready:false};
  }
}
async function saveNote(noteId:string,title:string,body:string,pinned:boolean,version:number,storage:Storage): Promise<Saved> {
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
    return {id:savedId,version,title,body,pinned,revision:++revision,message:'Saved on this device.',failed:false};
  } catch(error) {return {id:noteId,version,title,body,pinned,revision:++revision,message:message(error),failed:true};}
}
async function service(source:string,args:unknown[],storage:Storage): Promise<Status> {
  try {
    let text='',notice='';
    if(source==='backupNotes') {
      const all=await withDatabase(storage,notes);
      text=JSON.stringify({version:1,notes:all.map(({id,title,body,pinned})=>({id,title,body,pinned}))},null,2);
      if(text.length>4*1024*1024)throw new Error('This backup exceeds 4 MB. Split or remove large notes before backing up.');
      await storage.fs.mkdir('app:/data/backups');
      await storage.fs.atomicWriteFile(backupPath,encode(text));
      notice=`Backup saved with ${all.length} notes.`;
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
    return {revision:++revision,message:notice,failed:false,backupText:text};
  } catch(error) {return {revision:++revision,message:message(error),failed:true,backupText:''};}
}
const sources: Sources = {
  library: ([query], _store, storage) => serial(() => library(query, storage)),
  saveNote: (args, _store, storage) => serial(() => saveNote(...args, storage)),
  backupNotes: (args, _store, storage) => serial(() => service('backupNotes', args, storage)),
  readBackup: (args, _store, storage) => serial(() => service('readBackup', args, storage)),
  restoreNotes: (args, _store, storage) => serial(() => service('restoreNotes', args, storage)),
  deleteNote: (args, _store, storage) => serial(() => service('deleteNote', args, storage)),
};
export const answer: Answer = (source,args,store,storage) => sources[source](args,store,storage);
