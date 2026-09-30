//! Bake `app.contract` and `app.ts`. Before the bake, `local.ts` (gitignored)
//! is written from `EXPOSE_OPENROUTER_KEY` when that is set, or created empty
//! when it is missing, so the key a developer bakes into their own build never
//! enters the repository and a clean checkout still builds.

fn main() {
    println!("cargo:rerun-if-env-changed=EXPOSE_OPENROUTER_KEY");
    let app = std::path::Path::new("..");
    let local = app.join("local.ts");
    match std::env::var("EXPOSE_OPENROUTER_KEY") {
        Ok(key) if !key.trim().is_empty() => std::fs::write(
            &local,
            format!("// Written by build.rs from EXPOSE_OPENROUTER_KEY; never committed.\nexport const bakedKey = {};\n", serde_json_escape(key.trim())),
        )
        .expect("write local.ts"),
        _ if !local.exists() => std::fs::write(&local, "// Written by build.rs; never committed. Set EXPOSE_OPENROUTER_KEY to bake a key.\nexport const bakedKey = \"\";\n")
            .expect("write local.ts"),
        _ => {}
    }
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("ios") => "ios",
        _ => "macos",
    };
    exact_js_bake::build(app, platform).expect("bake Expose");
}

/// A JSON/JS string literal for a key: quotes, backslashes and control characters escaped.
fn serde_json_escape(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
