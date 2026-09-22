//! The LLP 0062 §3 intrinsic-freeze measurement, kept reproducible.
#![cfg(feature = "hermes")]

#[test]
#[ignore]
fn measure_userland_harden() {
    use ibex2::engine::hermes::{DynamicCode, Hermes};
    use std::time::Instant;

    // A SES-style transitive freeze, in userland: walk property DESCRIPTORS so
    // getters are not invoked, iterate rather than recurse so deep graphs do
    // not hit a stack cap, and track visited so cycles terminate.
    const HARDEN: &str = r#"
      globalThis.__harden = function () {
        const seen = new Set();
        const queue = [globalThis];
        let frozen = 0;
        while (queue.length) {
          const obj = queue.pop();
          if (obj === null || (typeof obj !== 'object' && typeof obj !== 'function')) continue;
          if (seen.has(obj)) continue;
          seen.add(obj);
          try { Object.freeze(obj); frozen++; } catch (e) {}
          const names = Object.getOwnPropertyNames(obj);
          for (let i = 0; i < names.length; i++) {
            let d;
            try { d = Object.getOwnPropertyDescriptor(obj, names[i]); } catch (e) { continue; }
            if (!d) continue;
            if ('value' in d) queue.push(d.value);
            else { queue.push(d.get); queue.push(d.set); }
          }
          try { queue.push(Object.getPrototypeOf(obj)); } catch (e) {}
        }
        return frozen;
      };
    "#;

    let mut total = std::time::Duration::ZERO;
    let runs = 20;
    let mut count = String::new();
    for _ in 0..runs {
        let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
        rt.install_stdlib();
        rt.eval(HARDEN).unwrap();
        let t = Instant::now();
        count = rt.eval("String(__harden())").unwrap();
        total += t.elapsed();
    }
    println!("\n  userland harden of the whole global graph:");
    println!("    {count} objects frozen");
    println!("    {:?} mean over {runs} runs", total / runs);

    // A number for a freeze that does not freeze is worthless. Check it holds.
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    rt.install_stdlib();
    rt.eval(HARDEN).unwrap();
    rt.eval("__harden()").unwrap();
    for probe in [
        "Object.prototype.polluted = 1; String(Object.prototype.polluted)",
        "Array.prototype.map = function(){ return 'hijacked' }; String([1].map(x=>x))",
        "String.prototype.trim = function(){ return 'x' }; ' a '.trim()",
        "Object.defineProperty(Object.prototype, 'evil', {value: 1}); String(({}).evil)",
    ] {
        let program = format!("try {{ {probe} }} catch (e) {{ 'blocked' }}");
        println!("    mutation -> {}", rt.eval(&program).unwrap());
    }
}
