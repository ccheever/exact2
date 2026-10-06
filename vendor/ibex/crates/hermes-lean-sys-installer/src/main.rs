#[allow(dead_code)]
#[path = "../../hermes-lean-sys/build_support.rs"]
mod build_support;

// @ref LLP 0057.000#l1--the-bindings-door — L1h keeps acquisition outside
// offline consumer builds while sharing the resolver's trust implementation.
use build_support::{
    acquire_validated_host_bundle, acquire_validated_target_bundle, download_options_from_env,
    installer_command, pin_for_target, BundlePin, RELEASE_TAG,
};
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::process::Command;

const USAGE: &str = "Install the pinned Hermes release bundles into Cargo's verified cache.\n\n\
Usage:\n  hermes-lean-sys-installer [--target <rust-triple>]...\n\n\
The host bundle is always installed. Each --target adds a cross-compilation\n\
target; its host bundle supplies the executable hermesc.";

fn main() {
    if let Err(error) = run(env::args_os().skip(1)) {
        eprintln!("Hermes bundle installation failed: {error}");
        std::process::exit(1);
    }
}

fn run(arguments: impl Iterator<Item = OsString>) -> Result<(), String> {
    let arguments: Vec<OsString> = arguments.collect();
    if arguments
        .iter()
        .any(|argument| argument == "-h" || argument == "--help")
    {
        println!("{USAGE}");
        return Ok(());
    }

    let (requested, test_pin) = parse_arguments(arguments)?;
    let host = rustc_host()?;
    let mut targets = BTreeSet::from([host.clone()]);
    targets.extend(requested);

    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut options = download_options_from_env(manifest_dir)?;
    // The installer IS the explicit online step. A consumer that forces
    // HERMES_LEAN_SYS_OFFLINE in its .cargo/config.toml [env] passes that value
    // to `cargo run` too, so the build's offline switch must not apply here.
    options.offline = false;
    println!(
        "Installing Hermes pins {RELEASE_TAG} with {}",
        installer_command(&options)
    );

    println!("Installing pinned Hermes bundle for host {host}");
    let host_pin = selected_pin(&host, test_pin.as_ref())?;
    let validated_host = acquire_validated_host_bundle(host_pin, &options, &host, false)?;
    println!("Installed {host} at {}", validated_host.root.display());

    for target in targets.into_iter().filter(|target| target != &host) {
        println!("Installing pinned Hermes bundle for target {target}");
        let pin = selected_pin(&target, test_pin.as_ref())?;
        let root = acquire_validated_target_bundle(pin, &options, &target, &validated_host, false)?;
        println!("Installed {target} at {}", root.display());
    }

    Ok(())
}

fn parse_arguments(arguments: Vec<OsString>) -> Result<(Vec<String>, Option<BundlePin>), String> {
    let mut targets = Vec::new();
    let mut test_pin = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let argument = argument
            .into_string()
            .map_err(|_| "arguments must be UTF-8".to_owned())?;
        match argument.as_str() {
            "--target" => targets.push(value(&mut arguments, "--target")?),
            // This test-only override cannot weaken a real build: the build
            // resolver still admits only its compiled-in target and digest
            // table. It lets the end-to-end test exercise this executable
            // against a small local-mirror fixture.
            "--test-pin" => {
                if test_pin.is_some() {
                    return Err("--test-pin may be specified only once".to_owned());
                }
                let target = leak(value(&mut arguments, "--test-pin target")?);
                let asset = leak(value(&mut arguments, "--test-pin asset")?);
                let sha256 = leak(value(&mut arguments, "--test-pin sha256")?);
                test_pin = Some(BundlePin {
                    target,
                    asset,
                    sha256,
                });
            }
            _ => return Err(format!("unrecognized argument {argument:?}\n\n{USAGE}")),
        }
    }
    Ok((targets, test_pin))
}

fn value(arguments: &mut impl Iterator<Item = OsString>, option: &str) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))?
        .into_string()
        .map_err(|_| format!("{option} value must be UTF-8"))
}

fn selected_pin(target: &str, test_pin: Option<&BundlePin>) -> Result<&'static BundlePin, String> {
    if let Some(pin) = test_pin.filter(|pin| pin.target == target) {
        // The process owns these leaked test strings until exit. The normal
        // path always returns the static production trust table.
        return Ok(Box::leak(Box::new(*pin)));
    }
    pin_for_target(target)
}

fn rustc_host() -> Result<String, String> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let output = Command::new(&rustc)
        .arg("-vV")
        .output()
        .map_err(|error| format!("cannot run {:?} -vV to determine the host: {error}", rustc))?;
    if !output.status.success() {
        return Err(format!("{:?} -vV failed with {}", rustc, output.status));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| format!("{:?} -vV output was not UTF-8", rustc))?;
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| format!("{:?} -vV did not report a host triple", rustc))
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_are_repeatable_and_a_test_pin_is_explicit() {
        let (targets, pin) = parse_arguments(
            [
                "--target",
                "aarch64-apple-ios",
                "--target",
                "x86_64-unknown-linux-gnu",
                "--test-pin",
                "test-host",
                "test.tar.gz",
                &"a".repeat(64),
            ]
            .into_iter()
            .map(OsString::from)
            .collect(),
        )
        .expect("arguments");
        assert_eq!(targets, ["aarch64-apple-ios", "x86_64-unknown-linux-gnu"]);
        let pin = pin.expect("test pin");
        assert_eq!(pin.target, "test-host");
        assert_eq!(pin.asset, "test.tar.gz");
    }
}
