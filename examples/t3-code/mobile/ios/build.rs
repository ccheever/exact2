// @ref llp/1106.000-mobile-app-layout.decision.md#decision
// Viewer sources stay authored/vendor inputs. IIFE assets are generated before capture.
const VIEWERS: &str = r#"
import { rolldown } from 'rolldown';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
const app = resolve(process.argv[1]);
const source = resolve(app, 'browser-mobile-renderer');
const assets = resolve(app, 'assets/browser-mobile');
try { await readFile(resolve(source, 'node_modules/effect/package.json')); }
catch { throw Error('Install the pinned viewer library first: cd examples/t3-code/mobile/browser-mobile-renderer && bun install --frozen-lockfile'); }
await mkdir(assets, {recursive:true});
for (const [entry,name,file] of [['preview-stream.browser.ts','T3PreviewStream','preview.js'],['device-stream.browser.ts','T3DeviceStream','device.js']]) {
  const bundle = await rolldown({input:resolve(source,entry),transform:{target:'es2022'}});
  const result = await bundle.generate({format:'iife',name,minify:true});
  await bundle.close();
  const code = result.output.find(x=>x.type==='chunk')?.code;
  if (!code) throw Error('Viewer bundle emitted no script');
  const path = resolve(assets,file);
  if (await readFile(path,'utf8').catch(()=>null) !== code) await writeFile(path,code);
  console.log(file+' '+Buffer.byteLength(code)+' '+createHash('sha256').update(code).digest('hex'));
  const document = await import(resolve(source, entry.replace('.browser.ts','-document.ts')));
  const html = (name === 'T3PreviewStream' ? document.previewStreamDocument : document.deviceStreamDocument)('__T3_NATIVE_CONFIGURATION__',code);
  const htmlPath = resolve(assets,file.replace('.js','.html'));
  if (await readFile(htmlPath,'utf8').catch(()=>null) !== html) await writeFile(htmlPath,html);
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
    let status =
        std::process::Command::new(std::env::var_os("EXACT_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", VIEWERS])
            .arg(&app)
            .current_dir(repository)
            .status()
            .expect("run pinned Bun for mobile viewers");
    assert!(status.success(), "bundle mobile viewers before the bake");
    exact_js_bake::build(&app, "ios").expect("bake T3 Code for iOS");
}
