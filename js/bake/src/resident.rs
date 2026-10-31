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
        if exact_js::ENGINE_LINKED {
            tools.check_engine()?;
        }
        let stage = Scratch::new(&std::env::temp_dir())?;
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let standard = |tool: &Path, name: &str| {
            tool.canonicalize().ok() == super::package_tool(&root, name).canonicalize().ok()
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
import { parseSync } from 'rolldown/utils';
import { realpathSync, rmSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';
const stage=realpathSync(process.argv[1]);
const libraries=resolve(dirname(fileURLToPath(import.meta.resolve(`@typescript/typescript-${process.platform}-${process.arch}/package.json`))),'lib');
const tsc=resolve(libraries,'tsc');
const config=resolve(stage,'__exact_tsconfig.json');
// The native builder owns dependency and diagnostic invalidation, including
// globals and standard libraries. Its cache never comes from the app capture.
async function compile() {
  // Generated output is not a captured input. Remove it before resolution,
  // so an app's ./app.js import follows the same TS substitution as one-shot.
  rmSync(resolve(stage,'app.js'),{force:true});
  const { configure, check, dataBundle } = await import(resolve(stage,'__exact_config.mjs'));
  configure(stage, true, parseSync);
  const checking=check(stage,tsc,libraries).then(()=>null,error=>error);
  let failed;
  try {
  const bundle=await rolldown({...dataBundle(stage),cwd:stage,input:resolve(stage,'__exact_entry.ts'),
    tsconfig:config});
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

/// The one TypeScript configuration and its check (`typescript.mjs`), shared
/// by the one-shot and resident compilers and by the web build. It reads only
/// captured configuration; the producer still owns the stage.
pub(super) const CONFIG: &str = include_str!("typescript.mjs");

/// The module binding's generator, which `CONFIG` imports from beside it.
pub(super) const BINDING: &str = include_str!("__exact_binding.mjs");

/// The settings' reader (`import.meta.env`), which `CONFIG` imports from beside it.
pub(super) const ENV: &str = include_str!("__exact_env.mjs");

/// The target a module binding answers `supports` for (LLP 1133.006 D7), as
/// an implementation's `targets` names it: Cargo's `CARGO_CFG_TARGET_OS` in
/// an app's build script, else the OS of `TARGET`'s triple, which is all a
/// bake run outside Cargo (`exact-js-bake --cargo`) is told. None where
/// neither says, as in a development loop that serves several targets.
pub(super) fn binding_target() -> Option<String> {
    let os = std::env::var("CARGO_CFG_TARGET_OS").ok();
    os.or_else(|| target_os(&std::env::var("TARGET").ok()?).map(str::to_owned))
}

/// The OS of a Rust target triple, in Cargo's `target_os` words, for the
/// targets an implementation names.
fn target_os(triple: &str) -> Option<&'static str> {
    [
        ("-apple-tvos", "tvos"),
        ("-apple-ios", "ios"),
        ("-apple-darwin", "macos"),
        ("-android", "android"),
        ("-windows", "windows"),
        ("-linux", "linux"),
    ]
    .into_iter()
    .find(|(part, _)| triple.contains(part))
    .map(|(_, os)| os)
}

/// `CONFIG` and the modules it imports written into `stage`, each only when changed.
pub(super) fn write_config(stage: &Path) -> Result<(), String> {
    super::write_changed(&stage.join("__exact_config.mjs"), CONFIG.as_bytes())?;
    super::write_changed(&stage.join("__exact_binding.mjs"), BINDING.as_bytes())?;
    super::write_changed(&stage.join("__exact_env.mjs"), ENV.as_bytes())
}

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
        write("__exact_binding.mjs", BINDING);
        write("__exact_env.mjs", ENV);
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
                "app": stage.0.join("original").join("app"),
                "mounts": [["lib", stage.0.join("original").join("shared")]]
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
        // Both producers inject from the immutable capture, even after a reused
        // stage's app.ts was injected during an earlier build.
        let source = "export const origin='http://127.0.0.1:4400'; export const grants=`net.fetch ${origin}`;";
        write("app.ts", source);
        write(
            "__exact_backend_source.json",
            &serde_json::to_string(source).unwrap(),
        );
        write(
            "__exact_entry.ts",
            "export { origin, grants } from './app';",
        );
        std::fs::create_dir(stage.0.join(".exact")).unwrap();
        write(".exact/backend-origin", "http://127.0.0.1:4412");
        assert!(resident().unwrap().contains("4412"));
        let warning = || {
            serde_json::from_str::<Option<String>>(
                &std::fs::read_to_string(stage.0.join("__exact_backend_warning.json")).unwrap(),
            )
            .unwrap()
        };
        assert!(warning()
            .unwrap()
            .contains("overriding the app.ts origin literal"));
        compile_once(&stage.0, &tools).unwrap();
        assert!(std::fs::read_to_string(stage.0.join("app.js"))
            .unwrap()
            .contains("4412"));
        std::fs::remove_file(stage.0.join(".exact/backend-origin")).unwrap();
        assert!(resident().unwrap().contains("4400"));
        assert_eq!(warning(), None);
        compile_once(&stage.0, &tools).unwrap();
        assert!(std::fs::read_to_string(stage.0.join("app.js"))
            .unwrap()
            .contains("4400"));
    }
}
