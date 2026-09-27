//! The development producer retains compiler processes, never unchecked app results.
use super::{bake_in, compile_bytecode, BakeMode, Baked, Scratch, Seed, Tools};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};

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
    /// Capture and validate a fresh generation. A refusal never returns stale
    /// output. `seed` supplies the Rust-owned first-frame values this
    /// producer cannot compute (LLP 1027.002 §5 step 0).
    pub fn bake(&mut self, app: &Path, seed: Option<&Seed>) -> Result<Baked, String> {
        bake_in(
            app,
            &self.tools,
            &self.stage.0,
            &mut self.previous,
            BakeMode::Development {
                compiler: self.compiler.as_mut(),
            },
            None,
            seed,
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
        let mut child = exact_bake::bun()
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
import { readFileSync, writeFileSync, realpathSync, rmSync } from 'node:fs';
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
  const { configure } = await import(resolve(stage,'__exact_config.mjs'));
  configure(stage, true);
  const checking=check().then(()=>null,error=>error);
  let failed;
  try {
  const bundle=await rolldown({cwd:stage,input:resolve(stage,'__exact_entry.ts'),platform:'neutral',
    tsconfig:config,
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

// Shared by the one-shot and resident compilers. Read only captured configuration;
// the producer still owns checking policy and the ambient library surface.
pub(super) const CONFIG: &str = r#"
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve, relative, dirname, sep, isAbsolute } from 'node:path';
export function configure(stage, incremental=false) {
  const inside = (path) => {
    if (path !== stage && !path.startsWith(stage+sep)) throw new Error('tsconfig: path outside captured app: '+path);
    return path;
  };
  const mapping=resolve(stage,'__exact_paths.json');
  const roots=existsSync(mapping) ? JSON.parse(readFileSync(mapping,'utf8')) : {app:stage,mounts:[]};
  const within=(path,root)=>path===root || path.startsWith(root+sep);
  const origins=[...roots.mounts.map(([name,path])=>[resolve(stage,name),path]),[stage,roots.app]];
  const source = path => {
    const [at,from]=origins.find(([at])=>within(path,at));
    return resolve(from,relative(at,path));
  };
  const captured = path => {
    const match=[...origins].sort((a,b)=>b[1].length-a[1].length).find(([,from])=>within(path,from));
    if (!match) throw new Error('tsconfig: path outside captured app: '+path);
    return inside(resolve(match[0],relative(match[1],path)));
  };
  const local = path => './'+relative(stage,captured(path)).split(sep).join('/');
  const object = (value, label) => {
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('tsconfig: '+label+' must be an object');
    return value;
  };
  const chain = new Set();
  function read(path) {
    inside(path);
    if (chain.has(path)) throw new Error('tsconfig: cyclic extends: '+path);
    chain.add(path);
    const json=object(Bun.JSONC.parse(readFileSync(path,'utf8')),path), dir=source(dirname(path));
    let inherited={};
    for (const parent of json.extends === undefined ? [] : Array.isArray(json.extends) ? json.extends : [json.extends]) {
      if (typeof parent !== 'string' || !parent.startsWith('.')) throw new Error('tsconfig: extends must name a captured relative config');
      let target=captured(resolve(dir,parent));
      if (!existsSync(target) && !target.endsWith('.json')) target+='.json';
      inherited={...inherited,...read(target)};
    }
    const options=object(json.compilerOptions ?? {},'compilerOptions');
    if (options.baseUrl !== undefined) {
      if (typeof options.baseUrl !== 'string') throw new Error('tsconfig: baseUrl must be a string');
      inherited.base=resolve(dir,options.baseUrl);
      captured(inherited.base);
    }
    if (options.paths !== undefined) inherited.paths={map:object(options.paths,'paths'),dir};
    chain.delete(path);
    return inherited;
  }
  const entry=resolve(stage,'tsconfig.json');
  const {base,paths:declared}=existsSync(entry) ? read(entry) : {};
  const paths={};
  for (const [key,values] of Object.entries(declared?.map ?? {})) {
    if (key.split('*').length>2 || !Array.isArray(values) || !values.length || values.some(v=>typeof v!=='string' || v.split('*').length>2 || isAbsolute(v))) {
      throw new Error('tsconfig: paths.'+key+' must name relative paths with at most one wildcard');
    }
    paths[key]=values.map(value=>local(resolve(base ?? declared.dir,value)));
    // TypeScript 7 removed baseUrl. Lower its fallback to paths, shared with
    // Rolldown, so existing editor configuration retains the same resolution.
    if (base) paths[key].push(local(resolve(base,key)));
  }
  if (base && !paths['*']) paths['*']=[local(resolve(base,'*'))];
  const config={compilerOptions:{noEmit:true,strict:true,target:'ES2020',module:'ESNext',moduleResolution:'bundler',lib:['ES2020','WebWorker'],paths,
    ...(incremental ? {incremental:true,tsBuildInfoFile:resolve(stage,'__exact_build.tsbuildinfo')} : {})},files:['__exact_entry.ts']};
  const path=resolve(stage,'__exact_tsconfig.json'), bytes=JSON.stringify(config);
  if (!existsSync(path) || readFileSync(path,'utf8')!==bytes) writeFileSync(path,bytes);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile_once;

    #[test]
    fn tsconfig_changes_resolve_alike_in_both_compilers_without_loading_hermes() {
        let stage = Scratch::new(&std::env::temp_dir()).unwrap();
        let write = |name: &str, bytes: &str| std::fs::write(stage.0.join(name), bytes).unwrap();
        std::fs::create_dir(stage.0.join("lib")).unwrap();
        std::fs::create_dir(stage.0.join("config")).unwrap();
        write("lib/word.ts", "export const word = 'mapped';");
        write("__exact_entry.ts", "export { word } from '@/word';");
        write("__exact_config.mjs", CONFIG);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut compiler = Compiler::new(&root, &stage.0).unwrap();
        let tools = Tools::default();
        let mut resident = || {
            compiler.input.as_mut().unwrap().write_all(b"{}\n").unwrap();
            let first = compiler.reply()?;
            if first["phase"] != "bundled" {
                return Err(first["error"].to_string());
            }
            let checked = compiler.reply()?;
            if checked["ok"] != true {
                return Err(checked["error"].to_string());
            }
            std::fs::read_to_string(stage.0.join("app.js")).map_err(|e| e.to_string())
        };
        for config in [
            r#"{"compilerOptions":{"paths":{"@/*":["./missing/*","./lib/*"]}}}"#,
            r#"{"compilerOptions":{"baseUrl":"lib","paths":{"@/word":["word.ts"]}}}"#,
            r#"{"extends":"./config/base.json"}"#,
        ] {
            write(
                "config/base.json",
                r#"{
                // Paths without baseUrl are relative to this config.
                "compilerOptions":{"paths":{"@/*":["../lib/*"]}},
            }"#,
            );
            write("tsconfig.json", config);
            compile_once(&stage.0, &tools).unwrap();
            let one = std::fs::read_to_string(stage.0.join("app.js")).unwrap();
            assert!(one.contains("mapped"));
            assert_eq!(resident().unwrap(), one);
        }
        // The resident process must reread extended configs as well as the root.
        write("lib/next.ts", "export const word = 'changed';");
        write(
            "config/base.json",
            r#"{"compilerOptions":{"paths":{"@/word":["../lib/next.ts"]}}}"#,
        );
        assert!(resident().unwrap().contains("changed"));
        for config in [
            "{}",
            r#"{"compilerOptions":{"paths":{"@/*":["../uncaptured/*"]}}}"#,
            r#"{"compilerOptions":{"baseUrl":"../uncaptured"}}"#,
            r#"{"compilerOptions":{"paths":{"@/*":"./lib/*"}}}"#,
            r#"{"extends":"../uncaptured.json"}"#,
            r#"{"extends":"./tsconfig.json"}"#,
        ] {
            write("tsconfig.json", config);
            assert!(compile_once(&stage.0, &tools).is_err(), "{config}");
            assert!(resident().is_err(), "{config}");
        }
        // Editor paths into an external directory resolve to its declared mount.
        write(
            "__exact_paths.json",
            &serde_json::json!({
                "app": stage.0.join("original/app"),
                "mounts": [["lib", stage.0.join("original/shared")]]
            })
            .to_string(),
        );
        write(
            "tsconfig.json",
            r#"{"compilerOptions":{"paths":{"@/*":["../shared/*"]}}}"#,
        );
        compile_once(&stage.0, &tools).unwrap();
        assert!(resident().unwrap().contains("mapped"));
        std::fs::remove_file(stage.0.join("__exact_paths.json")).unwrap();
        // baseUrl also resolves bare imports without any paths entry.
        write("tsconfig.json", r#"{"compilerOptions":{"baseUrl":"lib"}}"#);
        write("__exact_entry.ts", "export { word } from 'word';");
        compile_once(&stage.0, &tools).unwrap();
        assert!(resident().unwrap().contains("mapped"));
    }
}
