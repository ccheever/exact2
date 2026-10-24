// @ref examples/t3-code/mobile/docs/1130.000-mobile-app-layout.decision.md#decision
// Viewer sources stay authored/vendor inputs. IIFE assets are generated before capture.
const VIEWERS: &str = r#"
import { rolldown } from 'rolldown';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
const manifest = process.argv[2] === '--manifest';
if (!process.argv[1] || process.argv.length !== (manifest ? 3 : 2)) throw Error('Expected mobile app directory and optional --manifest');
const app = resolve(process.argv[1]);
if (!manifest) {
  const generated = Bun.spawnSync([process.execPath, resolve(app, '../tools/generate/generate.mjs')], {stdout:'inherit', stderr:'inherit'});
  if (generated.exitCode !== 0) throw Error('Generate pinned app sources before the mobile bake');
}
const source = resolve(app, 'browser-mobile-renderer');
const assets = resolve(app, 'assets/browser-mobile');
try { await readFile(resolve(source, 'node_modules/effect/package.json')); }
catch { throw Error('Install the pinned viewer library first: cd examples/t3-code/mobile/browser-mobile-renderer && bun install --frozen-lockfile'); }
const files = [];
for (const [entry,name,file] of [['preview-stream.browser.ts','T3PreviewStream','preview.js'],['device-stream.browser.ts','T3DeviceStream','device.js']]) {
  const bundle = await rolldown({input:resolve(source,entry),transform:{target:'es2022'}});
  const result = await bundle.generate({format:'iife',name,minify:true});
  await bundle.close();
  const code = result.output.find(x=>x.type==='chunk')?.code;
  if (!code) throw Error('Viewer bundle emitted no script');
  const document = await import(resolve(source, entry.replace('.browser.ts','-document.ts')));
  const html = (name === 'T3PreviewStream' ? document.previewStreamDocument : document.deviceStreamDocument)('__T3_NATIVE_CONFIGURATION__',code);
  if (typeof html !== 'string') throw Error('Viewer document emitted no HTML');
  files.push({path:resolve(assets,file),text:code,log:file}, {path:resolve(assets,file.replace('.js','.html')),text:html});
}
if (manifest) {
  console.log(JSON.stringify({
    files:files.map(({path,text})=>({path,bytes:Buffer.byteLength(text),sha256:createHash('sha256').update(text).digest('hex')})),
    stale:{scan:false,remove:[]},
  }));
} else {
  await mkdir(assets, {recursive:true});
  for (const {path,text,log} of files) {
    if (await readFile(path,'utf8').catch(()=>null) !== text) await writeFile(path,text);
    if (log) console.log(log+' '+Buffer.byteLength(text)+' '+createHash('sha256').update(text).digest('hex'));
  }
}
"#;

fn main() {
    for entry in std::fs::read_dir("../browser-mobile-renderer").expect("viewer sources") {
        let path = entry.expect("viewer input").path();
        if path.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    println!("cargo:rerun-if-changed=../../../../package.json");
    println!("cargo:rerun-if-changed=../../../../bun.lock");
    println!("cargo:rerun-if-env-changed=EXACT_BUN");
    let app = std::path::Path::new("..")
        .canonicalize()
        .expect("mobile app directory");
    let repository = app.ancestors().nth(3).expect("repository");
    // Bun's stdout initialization can set O_NONBLOCK on an inherited pipe.
    // Keep its descriptors separate from Cargo's later watch-line output.
    let output =
        std::process::Command::new(std::env::var_os("EXACT_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", VIEWERS])
            .arg(&app)
            .current_dir(repository)
            .output()
            .expect("run pinned Bun for mobile viewers");
    std::io::Write::write_all(&mut std::io::stdout(), &output.stdout)
        .expect("write mobile viewer output");
    std::io::Write::write_all(&mut std::io::stderr(), &output.stderr)
        .expect("write mobile viewer diagnostics");
    assert!(
        output.status.success(),
        "bundle mobile viewers before the bake"
    );
    exact_js_bake::build(&app, "ios").expect("bake T3 Code for iOS");
}
