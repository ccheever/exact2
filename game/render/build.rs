use std::{env, fs, path::Path};

fn main() {
    let out = env::var_os("OUT_DIR").unwrap();
    // Skinning is maintained by the concurrent skeleton slice. Its source stays
    // untouched; every shader assembled by pipeline.rs uses this packaging step.
    for name in [
        "frame",
        "transform",
        "shadow_sample",
        "forward",
        "shadow",
        "sky",
        "tonemap",
        "bloom",
        "model",
        "model_shadow",
    ] {
        let path = format!("src/shaders/{name}.wgsl");
        println!("cargo:rerun-if-changed={path}");
        let source = fs::read_to_string(path).unwrap();
        let packed = pack(&source);
        fs::write(Path::new(&out).join(format!("{name}.wgsl")), packed).unwrap();
    }
}

// WGSL has no string literals. Replace comments with whitespace (nested block
// comments included), then trim each line. Preserve every newline, so concatenated
// shader diagnostics retain their original line numbers; only columns change.
fn pack(source: &str) -> String {
    let mut chars = source.chars().peekable();
    let mut text = String::with_capacity(source.len());
    let mut block = 0;
    let mut line = false;
    while let Some(c) = chars.next() {
        if c == '\n' {
            text.push(c);
            line = false;
        } else if line {
            continue;
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            block += 1;
            text.push(' ');
        } else if block != 0 {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                block -= 1;
            }
        } else if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            line = true;
        } else {
            text.push(c);
        }
    }
    assert_eq!(block, 0, "unterminated shader comment");
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        out.push_str(line.trim());
        if line.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}
