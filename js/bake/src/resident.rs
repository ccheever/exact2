//! The development producer retains compiler processes, never unchecked app results.
use super::{bake_in, Baked, Scratch, Tools};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// Sequential development bakes over one private captured graph. Every request
/// still runs strict diagnostics, bundling, HBC compilation and the normal bake.
/// Nonstandard compiler tools retain their existing one-shot invocation contract.
pub struct Producer {
    compiler: Option<Compiler>,
    tools: Tools,
    stage: Scratch,
    previous: BTreeMap<PathBuf, Vec<u8>>,
}
impl Producer {
    /// Start a producer using the requested tools. Compiler overrides are not ignored.
    pub fn new(tools: Tools) -> Result<Self, String> {
        let stage = Scratch::new(&std::env::temp_dir())?;
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let standard = |tool: &Path, name: &str| {
            tool.canonicalize().ok()
                == root
                    .join("node_modules/.bin")
                    .join(name)
                    .canonicalize()
                    .ok()
                && tool.exists()
        };
        let compiler = if standard(&tools.tsc, "tsc") && standard(&tools.rolldown, "rolldown") {
            Some(Compiler::new(&root, &stage.0)?)
        } else {
            None
        };
        Ok(Self {
            tools,
            stage,
            previous: BTreeMap::new(),
            compiler,
        })
    }
    /// Capture and validate a fresh generation. A refusal never returns stale output.
    pub fn bake(&mut self, app: &Path) -> Result<Baked, String> {
        bake_in(
            app,
            &self.tools,
            &self.stage.0,
            &mut self.previous,
            self.compiler.as_mut(),
        )
    }
}

pub(super) struct Compiler {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}
impl Compiler {
    fn new(root: &Path, stage: &Path) -> Result<Self, String> {
        let mut child = Command::new("node")
            .args(["--input-type=module", "-e", WORKER])
            .arg(stage)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("resident compiler: {e}"))?;
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().ok_or("compiler stdout unavailable")?);
        Ok(Self {
            child,
            input,
            output,
        })
    }
    pub(super) fn compile(&mut self, _stage: &Path) -> Result<(), String> {
        let input = self.input.as_mut().ok_or("resident compiler is closed")?;
        input
            .write_all(b"{}\n")
            .and_then(|_| input.flush())
            .map_err(|e| format!("resident compiler: {e}"))?;
        let mut line = String::new();
        self.output
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        let reply: serde_json::Value = serde_json::from_str(&line)
            .map_err(|e| format!("resident compiler stopped or sent an invalid reply: {e}"))?;
        if reply["ok"] == true {
            Ok(())
        } else {
            Err(reply["error"]
                .as_str()
                .unwrap_or("resident compiler refused")
                .to_owned())
        }
    }
}
impl Drop for Compiler {
    fn drop(&mut self) {
        // EOF closes the compiler API and its owned TS server before Node exits.
        drop(self.input.take());
        let _ = self.child.wait();
    }
}

const WORKER: &str = r#"
import { API } from 'typescript/unstable/sync';
import { rolldown } from 'rolldown';
import { readFileSync, writeFileSync, readdirSync, realpathSync } from 'node:fs';
import { resolve, dirname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';
const stage=realpathSync(process.argv[1]);
const libraries=resolve(dirname(fileURLToPath(import.meta.resolve(`@typescript/typescript-${process.platform}-${process.arch}/package.json`))),'lib');
const allowed = path => { path=resolve(path);return path === stage || path.startsWith(stage+sep) || path === libraries || path.startsWith(libraries+sep); };
const api=new API({cwd:stage,fs:{
  readFile:path=>allowed(path)?undefined:null,
  fileExists:path=>allowed(path)?undefined:false,
  directoryExists:path=>allowed(path)?undefined:false,
  getAccessibleEntries:path=>allowed(path)?undefined:{files:[],directories:[]},
}});
const config=resolve(stage,'__exact_tsconfig.json');
writeFileSync(config,JSON.stringify({compilerOptions:{noEmit:true,strict:true,target:'ES2020',module:'ESNext',moduleResolution:'bundler',lib:['ES2020','DOM']},files:['__exact_entry.ts']}));
let previous=new Map(), opened=false;
function files(at=stage,result=new Map()) {
  for(const entry of readdirSync(at,{withFileTypes:true})) {
    const path=resolve(at,entry.name);
    if(entry.isDirectory())files(path,result);
    else if(/\.(ts|json)$/.test(path))result.set(path,readFileSync(path,'utf8'));
  }
  return result;
}
function diagnostics(program) {
  return [...program.getConfigFileParsingDiagnostics(),...program.getProgramDiagnostics(),
    ...program.getSyntacticDiagnostics(),...program.getBindDiagnostics(),
    ...program.getGlobalDiagnostics(),...program.getSemanticDiagnostics()];
}
function explain(d) {
  return `${d.fileName??'TypeScript'}:${d.pos??0} TS${d.code}: ${d.text}` + (d.messageChain??[]).map(x=>'\n'+explain(x)).join('');
}
async function compile() {
  const current=files(), changes={created:[],changed:[],deleted:[]};
  for(const [path,text] of current) {
    if(!previous.has(path))changes.created.push(path);
    else if(previous.get(path)!==text)changes.changed.push(path);
  }
  for(const path of previous.keys())if(!current.has(path))changes.deleted.push(path);
  const snapshot=api.updateSnapshot(opened?{fileChanges:changes}:{openProjects:[config]});
  opened=true;previous=current;
  try {
    const project=snapshot.getProject(config);
    if(!project)throw new Error('TypeScript did not load the captured project');
    const errors=diagnostics(project.program);
    if(errors.length)throw new Error(errors.map(explain).join('\n'));
  } finally { snapshot.dispose(); }
  const bundle=await rolldown({cwd:stage,input:resolve(stage,'__exact_entry.ts'),platform:'neutral',
    plugins:[{name:'captured-sources',load(id){if(!id.startsWith(stage+sep))throw new Error('module outside captured app: '+id);return null;}}]});
  try { await bundle.write({file:resolve(stage,'app.js'),format:'iife',name:'exact'}); }
  finally { await bundle.close(); }
}
process.once('SIGTERM',()=>{api.close();process.exit(0);});
try {
  for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})) {
    try { JSON.parse(line);await compile();process.stdout.write('{"ok":true}\n'); }
    catch(error){process.stdout.write(JSON.stringify({ok:false,error:String(error?.message??error).slice(0,65536)})+'\n');}
  }
} finally { api.close(); }
"#;
