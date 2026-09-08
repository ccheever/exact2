//! The development producer retains compiler processes, never unchecked app results.
use super::{bake_in, compile_bytecode, Baked, Scratch, Tools};
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
    pub(super) fn compile(&mut self, stage: &Path, hermesc: &Path) -> Result<(), String> {
        let input = self.input.as_mut().ok_or("resident compiler is closed")?;
        input
            .write_all(b"{}\n")
            .and_then(|_| input.flush())
            .map_err(|e| format!("resident compiler: {e}"))?;
        let reply = self.reply()?;
        if reply["phase"] != "bundled" {
            return Err(reply["error"]
                .as_str()
                .unwrap_or("resident compiler refused before bundling")
                .to_owned());
        }
        // Compilation is independent of checking. Never inspect or execute
        // the bytecode until both succeed, and always consume the final reply
        // even if Hermes refuses, so the next request cannot read stale status.
        let bytecode = compile_bytecode(stage, hermesc);
        let checked = self.reply().and_then(|reply| {
            if reply["ok"] == true {
                Ok(())
            } else {
                Err(reply["error"]
                    .as_str()
                    .unwrap_or("resident type checker refused")
                    .to_owned())
            }
        });
        match (bytecode, checked) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(a), Err(b)) => Err(format!("{a}\n{b}")),
            (Err(error), _) | (_, Err(error)) => Err(error),
        }
    }
    fn reply(&mut self) -> Result<serde_json::Value, String> {
        let mut line = String::new();
        self.output
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&line)
            .map_err(|e| format!("resident compiler stopped or sent an invalid reply: {e}"))
    }
}
impl Drop for Compiler {
    fn drop(&mut self) {
        // EOF closes the owned bundler process after any compiler invocation finishes.
        drop(self.input.take());
        let _ = self.child.wait();
    }
}

const WORKER: &str = r#"
import { rolldown } from 'rolldown';
import { writeFileSync, realpathSync, rmSync } from 'node:fs';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { resolve, dirname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';
const stage=realpathSync(process.argv[1]);
const libraries=resolve(dirname(fileURLToPath(import.meta.resolve(`@typescript/typescript-${process.platform}-${process.arch}/package.json`))),'lib');
const tsc=resolve(libraries,'tsc'), execute=promisify(execFile);
// Short-lived native checks trade fewer GC cycles for a soft Go memory limit
// (not an RSS cap); explicit user settings always win.
const checkEnv={GOGC:'300',GOMEMLIMIT:'128MiB',GOMAXPROCS:'4',...process.env};
const allowed = path => { path=resolve(path);return path === stage || path.startsWith(stage+sep) || path === libraries || path.startsWith(libraries+sep); };
const config=resolve(stage,'__exact_tsconfig.json');
// The native builder owns dependency and diagnostic invalidation, including
// globals and standard libraries. Its cache never comes from the app capture.
writeFileSync(config,JSON.stringify({compilerOptions:{noEmit:true,strict:true,target:'ES2020',module:'ESNext',moduleResolution:'bundler',lib:['ES2020','WebWorker'],incremental:true,tsBuildInfoFile:resolve(stage,'__exact_build.tsbuildinfo')},files:['__exact_entry.ts']}));
async function check() {
  let files;
  try { ({stdout:files}=await execute(tsc,['--project',config,'--pretty','false','--listFiles'],{cwd:stage,env:checkEnv,encoding:'utf8',maxBuffer:4*1024*1024})); }
  catch(error) {
    const diagnostics=String(error.stdout??'').split(/\r?\n/).filter(line=>! /^(?:\/|[A-Za-z]:[\\/]).*\.[cm]?[jt]sx?$/.test(line)).join('\n');
    throw new Error(diagnostics+String(error.stderr??'') || String(error.message));
  }
  // Check the compiler's actual resolved graph, including type-only imports.
  // An external declaration must not influence an accepted app artifact.
  for(const path of files.trim().split(/\r?\n/)) {
    if(!path || !allowed(realpathSync(path)))throw new Error('module outside captured app: '+path);
  }
}
async function compile() {
  // Generated output is not a captured input. Remove it before resolution,
  // so an app's ./app.js import follows the same TS substitution as one-shot.
  rmSync(resolve(stage,'app.js'),{force:true});
  const checking=check().then(()=>null,error=>error);
  let failed;
  try {
  const bundle=await rolldown({cwd:stage,input:resolve(stage,'__exact_entry.ts'),platform:'neutral',
    plugins:[{name:'captured-sources',load(id){if(!id.startsWith(stage+sep))throw new Error('module outside captured app: '+id);return null;}}]});
  try { await bundle.write({file:resolve(stage,'app.js'),format:'iife',name:'exact'}); }
  finally { await bundle.close(); }
  process.stdout.write('{"phase":"bundled"}\n');
  } catch(error) { failed=error; }
  const typeError=await checking;
  if(failed || typeError)throw new Error([failed,typeError].filter(Boolean).map(e=>e.message??String(e)).join('\n'));
}
process.once('SIGTERM',()=>process.exit(0));
for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})) {
  try { JSON.parse(line);await compile();process.stdout.write('{"ok":true}\n'); }
  catch(error){process.stdout.write(JSON.stringify({ok:false,error:String(error?.message??error).slice(0,65536)})+'\n');}
}
"#;
