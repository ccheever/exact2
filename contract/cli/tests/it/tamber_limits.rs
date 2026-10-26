//! The design checker's own bounds through the design server (theme2's
//! approval of the checker bound; rt1's wire co-sign): a write crossing a
//! length's 256 terms is refused in the build's id, words and place; a value
//! past 4096 bytes is refused before anything reads it, quickly, without
//! echoing it; and a design that is past a bound on disk (a hand edit, or a
//! design read again) is folded in moments.
#![cfg(unix)]

use super::tamber_package_params::reading;
use super::tamber_rows_check::transaction;
use super::tamber_serve::Serve;
use serde_json::json;
use std::time::{Duration, Instant};

/// This thread's CPU time: what an in-process build's work takes, not the
/// wall clock, which a loaded machine (the suite's other tests and servers,
/// another agent's builds) stretches with no work the build did (dtool2's
/// run at load 72 read 9.3 s of wall clock for a 3.0 s build; the build
/// runs on its caller's thread).
fn cpu() -> Duration {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a valid timespec for the call to fill.
    let read = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    assert_eq!(read, 0, "this thread's CPU clock");
    Duration::new(t.tv_sec as u64, t.tv_nsec as u32)
}

/// Two levels of `clamp()` over `space.md`, a base name the app leaves
/// unset: `c1` holds 13 atoms (9 values, within the kernel's `calc()`
/// limits, so the app builds); a value nesting three more `clamp()`s of it
/// holds 364, past the bound.
fn chain() -> String {
    let mut lines = vec!["tamber t\n\ntheme\n  ink = #111111\n  c0 = 4px".to_string()];
    for k in 1..3 {
        lines.push(format!("  c{k} = clamp(c{0}, c{0}, c{0})", k + 1));
    }
    lines.push("  c3 = calc(space.md + 1px)\n".to_string());
    lines.join("\n")
}

const PAST: &str = "clamp(clamp(clamp(c1, c1, c1), clamp(c1, c1, c1), clamp(c1, c1, c1)), clamp(clamp(c1, c1, c1), clamp(c1, c1, c1), clamp(c1, c1, c1)), clamp(clamp(c1, c1, c1), clamp(c1, c1, c1), clamp(c1, c1, c1)))";

/// The server refuses a write that makes `c0` hold 364 atoms, and `contract
/// build` refuses the written design with the same id, words and place.
#[test]
fn a_write_past_a_lengths_terms_is_refused_in_the_builds_words() {
    let dir = reading("limits-length");
    let design = chain();
    std::fs::write(dir.join("app.tamber"), &design).unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "c0", "value": PAST}),
    );
    drop(s);
    let d = &w["error"]["diagnostics"][0];
    assert_eq!(d["id"], "tamber-length-size", "{w}");
    std::fs::write(
        dir.join("app.tamber"),
        design.replace("c0 = 4px", &format!("c0 = {PAST}")),
    )
    .unwrap();
    let built = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(d["id"], built.id, "{w}");
    assert_eq!(d["message"], built.message, "{w}");
    assert_eq!(d["line"], built.span.line, "{w}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A value of 1 MB through the wire is refused before it is read, in
/// moments, in words that give its size and never echo it (rt1's co-sign).
#[test]
fn a_megabyte_value_is_refused_before_it_is_read() {
    let dir = reading("limits-bytes");
    let mut s = Serve::start(&dir, &[]);
    let big = format!("calc(1px{})", " + 1px".repeat(150_000));
    let began = Instant::now();
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-token", "name": "ink", "value": big}),
    );
    let took = began.elapsed();
    drop(s);
    let d = &w["error"]["diagnostics"][0];
    assert_eq!(
        d["id"],
        "tamber-value-size",
        "{}",
        w.to_string().chars().take(400).collect::<String>()
    );
    assert!(w.to_string().len() < 2000, "the reply echoes no value");
    assert!(took < Duration::from_secs(5), "{took:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A design past a bound on disk, read when the server starts and read
/// again after a hand edit, is folded in moments (on main a 30-level chain
/// ran for minutes), and the build refuses it with the bound's id.
#[test]
fn a_design_past_a_bound_on_disk_is_folded_in_moments() {
    let dir = reading("limits-reload");
    let mut long = vec!["tamber t\n\ntheme\n  ink = #111111".to_string()];
    for k in 0..30 {
        long.push(format!("  d{k} = clamp(d{0}, d{0}, d{0})", k + 1));
    }
    long.push("  d30 = calc(space.md + 1px)\n".to_string());
    let long = long.join("\n");
    std::fs::write(dir.join("app.tamber"), &long).unwrap();
    let began = Instant::now();
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello"}));
    assert!(
        began.elapsed() < Duration::from_secs(10),
        "{:?}",
        began.elapsed()
    );
    assert!(
        hello.get("ok").is_some() || hello.get("error").is_some(),
        "{hello}"
    );
    // Read again after a hand edit that makes it longer still.
    std::fs::write(
        dir.join("app.tamber"),
        long.replace(
            "d30 = calc(space.md + 1px)",
            "d30 = clamp(d31, d31, d31)\n  d31 = calc(space.md + 1px)",
        ),
    )
    .unwrap();
    let again = Instant::now();
    let (_, _) = s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#222222"}]}));
    assert!(
        again.elapsed() < Duration::from_secs(10),
        "{:?}",
        again.elapsed()
    );
    drop(s);
    let built = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(built.id, "tamber-length-size", "{built:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A profile's colour past the bound is left as written for the check, so
/// the build refuses it with the bound's id, as the design server does,
/// rather than rewriting it before the check (Astra's round 2).
#[test]
fn a_profile_colour_past_the_bound_is_refused_by_the_build_too() {
    let dir = reading("limits-profile");
    let padded = format!("color(--dci-p3 0 0 0{})", " ".repeat(5000));
    std::fs::write(
        dir.join("app.tamber"),
        format!("tamber t\n\ntheme\n  ink = #111111\n  wide = {padded}\n"),
    )
    .unwrap();
    let built = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(built.id, "tamber-value-size", "{built:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A param's name past the bound is refused before the wire resolves it,
/// never echoed (Astra's round 3: `set-param` quoted it whole).
#[test]
fn a_long_param_name_is_refused_before_it_is_resolved() {
    let dir = reading("limits-param");
    let mut s = Serve::start(&dir, &[]);
    let name = format!("Map.{}", "p".repeat(900_000));
    let w = transaction(
        &mut s,
        &dir,
        json!({"op": "set-param", "name": name, "value": "#abcdef"}),
    );
    drop(s);
    let said = w.to_string();
    assert!(
        said.contains("tamber-value-size"),
        "{}",
        &said[..said.len().min(400)]
    );
    assert!(said.len() < 2000, "the reply echoes no name");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Profile colours are taken in one pass over the design: 3,000 written
/// backwards, each naming the next, build in moments (Astra's round 3: a
/// scan of the design per profile, each against every profile so far).
#[test]
fn many_profile_colours_are_taken_in_moments() {
    let dir = reading("limits-profiles");
    let n = 3000;
    let mut src = String::from("tamber t\n\ntheme\n  ink = #111111\n");
    for k in (1..n).rev() {
        src += &format!("  p{k} = p{}\n", k - 1);
    }
    src += "  p0 = color(--dci-p3 0 0 0)\n";
    std::fs::write(dir.join("app.tamber"), &src).unwrap();
    let began = cpu();
    let built = contract::compile_path(&dir.join("app.contract"));
    let took = cpu() - began;
    assert!(took < Duration::from_secs(20), "{took:?} of CPU");
    assert!(built.is_ok(), "{:?}", built.err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The build's one-scheme question and its retention walk read each token
/// once (the checker bound's sweep): 10,000 tokens marked `scheme=fixed`
/// beside a chain of 400 aliases written backwards, and a chain of 30,000
/// names a param's value reads, build in moments (15.6 s before, 1.4 s
/// now).
#[test]
fn the_builds_scheme_and_retention_walks_are_linear() {
    let dir = reading("limits-walks");
    let mut src = String::from("tamber t\n\ntheme\n  ink = #111111\n");
    for k in 0..10_000 {
        src += &format!("  f{k} = #222222  scheme=fixed\n");
    }
    for k in (1..400).rev() {
        src += &format!("  a{k} = a{}\n", k - 1);
    }
    src += "  a0 = f0\n";
    for k in 1..30_000 {
        src += &format!("  c{k} = c{}\n", k - 1);
    }
    src += "  c0 = #333333\n\nparams\n  Map.pin = c29999\n";
    std::fs::write(dir.join("app.tamber"), &src).unwrap();
    let began = cpu();
    let built = contract::compile_path(&dir.join("app.contract"));
    let took = cpu() - began;
    // (in CPU time: 9.2 s before the sweep, 0.9 s now at load 180)
    assert!(took < Duration::from_secs(4), "{took:?} of CPU");
    assert!(built.is_ok(), "{:?}", built.err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The axes and `when` caps are design limits: the build refuses past them
/// with the same ids the design server's check does (theme2's 19:26).
#[test]
fn the_build_refuses_past_the_axes_and_when_caps() {
    let dir = reading("limits-caps");
    let mut src = String::from("tamber t\n\naxes\n");
    for a in 0..=exact_tamber::MAX_AXES {
        src += &format!("  a{a} = v\n");
    }
    src += "\ntheme\n  ink = #111111\n";
    std::fs::write(dir.join("app.tamber"), &src).unwrap();
    let built = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(built.id, "tamber-axes", "{built:?}");
    let mut src = String::from("tamber t\n\naxes\n  a = v0");
    for v in 1..=exact_tamber::MAX_WHENS {
        src += &format!(" | v{v}");
    }
    src += "\n\ntheme\n  ink = #111111\n";
    for v in 0..=exact_tamber::MAX_WHENS {
        src += &format!("    when a=v{v}: #22222{}\n", v % 10);
    }
    std::fs::write(dir.join("app.tamber"), &src).unwrap();
    let built = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(built.id, "tamber-whens", "{built:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
