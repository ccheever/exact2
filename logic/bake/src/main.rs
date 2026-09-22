//! Bake the first frame through the exact candidate Rust Wasm artifact.
//! No compiler or bake implementation is linked into an app. LLP 1029.000.
use exact_runner::{DataError, DataSource, Value};
use sha2::{Digest, Sha256};
use std::io::{BufRead, Read, Write};
use std::path::Path;

struct Admitted {
    app: String,
    grants: String,
}
impl DataSource for Admitted {
    fn app_id(&self) -> &str {
        &self.app
    }
    fn grants(&self) -> &str {
        &self.grants
    }
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::Unavailable(
            "the candidate module must be active at bake".into(),
        ))
    }
}
fn card(file: &str, bytes: &[u8]) -> serde_json::Value {
    serde_json::json!({"file":file,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes))})
}
fn bake(args: &[String]) -> Result<(), String> {
    let mapped = args.len() == 5 && args[4] == "--map";
    if args.len() != 4 && !mapped {
        return Err(
            "usage: exact-logic-bake <app.contract> <app.module.wasm> <compat.json> <output.plan> [--map]"
                .into(),
        );
    }
    let (mut plan, map) = if mapped {
        contract::compile_path_mapped(Path::new(&args[0])).map(|(plan, map)| (plan, Some(map)))
    } else {
        contract::compile_path(Path::new(&args[0])).map(|plan| (plan, None))
    }
    .map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&args[1]).map_err(|e| e.to_string())?;
    let compat: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[2]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let app = compat["inputs"]["app"]
        .as_str()
        .ok_or("compat has no app")?
        .to_owned();
    let grants = compat["inputs"]["rustGrants"]
        .as_str()
        .ok_or("compat has no Rust source grants")?
        .to_owned();
    let ceiling = compat["inputs"]["grantCeiling"]
        .as_str()
        .ok_or("compat has no grant ceiling")?;
    let admitted: Vec<_> = ceiling
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if grants
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .any(|line| !admitted.contains(&line))
    {
        return Err("Rust source grants exceed the app ceiling".into());
    }
    plan.app_id.clone_from(&app);
    let encoded = plan.encode();
    let receipt = serde_json::json!({"version":1,"kind":"rust","abi":exact_logic::ABI,"appId":app,"grants":grants,"target":"wasm32-unknown-unknown","executor":"wasm","plan":card("app.plan",&encoded),"module":card("app.module.wasm",&bytes)});
    let mut source = exact_logic::Swappable::wasm(Admitted { app, grants })
        .replacement(&encoded, &receipt.to_string(), bytes)
        .map_err(|e| format!("module admission: {e:?}"))?;
    source
        .activate()
        .map_err(|e| format!("module activation: {e:?}"))?;
    let baked = contract::bake(plan, source)
        .map_err(|e| {
            map.as_ref().map_or_else(
                || format!("bake: {e:?}"),
                |map| map.bake_error(&e).to_string(),
            )
        })?
        .encode();
    // The caller owns a private output directory and publishes only after this
    // request succeeds. A map failure must not report a complete candidate.
    if let Some(map) = map {
        std::fs::write(format!("{}.map.json", args[3]), map.json(&baked))
            .map_err(|e| e.to_string())?;
    }
    std::fs::write(&args[3], baked).map_err(|e| e.to_string())
}
fn serve() -> Result<(), String> {
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    writeln!(
        output,
        "{}",
        serde_json::json!({"ready":true,"pid":std::process::id()})
    )
    .map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    loop {
        let mut line = Vec::new();
        let count = input
            .by_ref()
            .take(1024 * 1024)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(());
        }
        if line.last() != Some(&b'\n') {
            return Err("unterminated or oversized bake request".into());
        }
        let result = serde_json::from_slice::<Vec<String>>(&line)
            .map_err(|e| e.to_string())
            .and_then(|args| bake(&args));
        let reply = match result {
            Ok(()) => serde_json::json!({"ok":true}),
            Err(error) => serde_json::json!({"ok":false,"error":error}),
        };
        writeln!(output, "{reply}").map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
    }
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = if args == ["--serve"] {
        serve()
    } else {
        bake(&args)
    };
    if let Err(error) = result {
        eprintln!("Rust module bake: {error}");
        std::process::exit(1);
    }
}
