//! `exact-bake compat <app-dir> --platform <p> [--target <triple>] [--json]`
//! — the compatibility id (LLP 1030 D3a) the bake writes beside the plan,
//! and with `--json` the inputs it digests, for reading why two differ.

use std::process::ExitCode;

const USAGE: &str = "usage:
  exact-bake compat <app-dir> --platform <ios|macos|linux|web> [--target <triple>] [--json]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--help" | "-h") if args.len() == 1 => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("compat") => compat(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

struct CompatOptions<'a> {
    dir: &'a str,
    platform: &'a str,
    target: Option<&'a str>,
    json: bool,
}

fn compat_options(args: &[String]) -> Option<CompatOptions<'_>> {
    let (dir, flags) = args.split_first()?;
    if dir.is_empty() || dir.starts_with('-') {
        return None;
    }
    let (mut platform, mut target, mut json) = (None, None, false);
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "--platform" if platform.is_none() => {
                let value = flags.next()?.as_str();
                if !matches!(value, "ios" | "macos" | "linux" | "web") {
                    return None;
                }
                platform = Some(value);
            }
            "--target" if target.is_none() => {
                let value = flags.next()?.as_str();
                if value.is_empty() || value.starts_with('-') {
                    return None;
                }
                target = Some(value);
            }
            "--json" if !json => json = true,
            _ => return None,
        }
    }
    Some(CompatOptions {
        dir,
        platform: platform?,
        target,
        json,
    })
}

/// `exact-bake compat <app-dir> --platform <p> [--target <triple>] [--json]`:
/// the compatibility id of the app built for that platform — the id alone,
/// or with `--json` the id and every input it digests. The grants are the
/// data crate's to declare and cannot be asked here without linking it;
/// the bake, which links it, passes them (`build.rs` of each host crate).
fn compat(args: &[String]) -> ExitCode {
    const USAGE: &str = "usage: exact-bake compat <app-dir> --platform <ios|macos|linux|web> [--target <triple>] [--json]";
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let Some(CompatOptions {
        dir,
        platform,
        target,
        json,
    }) = compat_options(args)
    else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let target = target
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS));
    let app_dir = std::path::Path::new(dir);
    let result = contract::Manifest::read(app_dir)
        .and_then(|m| exact_bake::compatibility_id(app_dir, platform, &target, &m, None));
    match result {
        Ok(c) => {
            if json {
                print!("{}", c.to_json());
            } else {
                println!("{}", c.id);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("exact-bake compat: {e}");
            ExitCode::from(1)
        }
    }
}
