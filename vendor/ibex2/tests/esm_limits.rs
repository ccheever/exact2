#![cfg(all(feature = "hermes", feature = "loader"))]
//! Probing what the ESM lowering does and does not preserve.
use ibex2::engine::hermes::{DynamicCode, Hermes};
use ibex2::loader::{ModuleGrants, Root};
use std::path::PathBuf;

struct P(PathBuf);
impl P {
    fn new(n: &str) -> Self {
        let d = std::env::temp_dir().join(format!("ibex2-lim-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Self(d)
    }
    fn f(&self, n: &str, s: &str) -> &Self {
        std::fs::write(self.0.join(n), s).unwrap();
        self
    }
    fn run(&self) -> (Vec<String>, Option<String>) {
        let mut rt = Hermes::new(DynamicCode::Closed).unwrap();
        rt.install_stdlib();
        rt.install_bindings().unwrap();
        rt.set_loader(Root::Declared(self.0.clone()), ModuleGrants::none())
            .expect("loader");
        let e = rt.run_entry("./index.js").err().map(|e| e.to_string());
        rt.run_to_quiescence(std::time::Duration::from_secs(5));
        (
            rt.drain_console().into_iter().map(|r| r.message).collect(),
            e,
        )
    }
}
impl Drop for P {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore]
fn probe_limits() {
    // 1. Import hoisting: real ESM allows use before the import statement.
    let p = P::new("hoist");
    p.f(
        "index.js",
        "console.log('got', typeof value);\nimport { value } from './d.js';\n",
    )
    .f("d.js", "export const value = 1;");
    println!("  import hoisting        -> {:?}", p.run());

    // 2. Named imports: live or snapshot?
    let p = P::new("liveimport");
    p.f(
        "index.js",
        "import { n, bump } from './c.js';\nimport * as ns from './c.js';\nbump();\nconsole.log('named', n, 'namespace', ns.n);",
    )
    .f("c.js", "export let n = 0;\nexport function bump() { n += 1; }");
    println!("  named vs namespace     -> {:?}", p.run());

    // 3. Cycles.
    let p = P::new("cycle");
    p.f(
        "index.js",
        "import { a } from './a.js';\nconsole.log('a is', a);",
    )
    .f(
        "a.js",
        "import { b } from './b.js';\nexport const a = 'A' + b;",
    )
    .f(
        "b.js",
        "import { a } from './a.js';\nexport const b = 'B' + typeof a;",
    );
    println!("  cycle                  -> {:?}", p.run());

    // 4. Top-level await.
    let p = P::new("tla");
    p.f(
        "index.js",
        "const v = await Promise.resolve(1);\nconsole.log('tla', v);",
    );
    println!("  top-level await        -> {:?}", p.run());

    // 5. import.meta
    let p = P::new("meta");
    p.f("index.js", "console.log('meta', typeof import.meta);");
    println!("  import.meta            -> {:?}", p.run());

    // 6. dynamic import
    let p = P::new("dyn");
    p.f("index.js", "import('./d.js').then(m => console.log('dyn', m.v), e => console.log('dyn failed:', e.message));")
        .f("d.js", "export const v = 1;");
    println!("  dynamic import()       -> {:?}", p.run());
}
