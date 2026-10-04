//! `difftest arith`: the semantics' binary64 model (`Contract.Binary64`)
//! against this machine's `f64`, operation by operation.
//!
//! For each operand pair `(a, b)` (random bits, edge values and their
//! neighbours, near-cancelling pairs, integers around `2^53`, operands near
//! overflow and underflow) and natural number `n`, both sides compute
//! `a + b`, `a - b`, `a * b`, `a / b`, `a % b`, `floor(a)`, `max(a, b)`,
//! `min(a, b)`, `trunc(a)`, `-a`, `n` as a double, and `a < b`, `a <= b`,
//! `a == b`, as the runner does (`f64` operators, `f64::floor`, `f64::max`,
//! `f64::min`, `f64::trunc`). Results are compared by bits, every NaN one.

use crate::leanrun;
use crate::rng::Rng;
use std::path::Path;
use std::process::Command;

const OPS: [&str; 14] = [
    "+", "-", "*", "/", "%", "floor", "max", "min", "trunc", "neg", "ofNat", "<", "<=", "==",
];

fn canon(x: f64) -> u64 {
    if x.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        x.to_bits()
    }
}

/// The runner's side of one line, in the Lean module's format.
fn expected(a: f64, b: f64, n: u64) -> String {
    let bit = |p: bool| if p { '1' } else { '0' };
    format!(
        "{:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {}{}{}",
        canon(a + b),
        canon(a - b),
        canon(a * b),
        canon(a / b),
        canon(a % b),
        canon(a.floor()),
        canon(a.max(b)),
        canon(a.min(b)),
        canon(a.trunc()),
        canon(-a),
        canon(n as f64),
        bit(a < b),
        bit(a <= b),
        bit(a == b),
    )
}

const EDGES: [f64; 22] = [
    0.0,
    -0.0,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NAN,
    1.0,
    -1.0,
    0.5,
    0.1,
    3.0,
    f64::MAX,
    f64::MIN_POSITIVE,
    5e-324,
    9007199254740992.0,
    9007199254740991.0,
    4503599627370496.0,
    1e308,
    1e-308,
    2.2250738585072009e-308,
    1.7976931348623157e308 / 2.0,
    0.49999999999999994,
    1e21,
];

/// One operand.
fn operand(rng: &mut Rng) -> f64 {
    match rng.below(8) {
        0 | 1 => f64::from_bits(rng.next_u64()),
        2 => {
            let e = EDGES[rng.below(EDGES.len() as u64) as usize];
            f64::from_bits(e.to_bits().wrapping_add(rng.below(5)).wrapping_sub(2))
        }
        3 => {
            // An integer, often near 2^53.
            let base = if rng.chance(1, 2) { 1u64 << 53 } else { 0 };
            let x = base as i64 + rng.below(2_000_001) as i64 - 1_000_000;
            let x = x as f64;
            if rng.chance(1, 4) {
                -x
            } else {
                x
            }
        }
        4 => {
            // Extreme exponents: products and quotients overflow, underflow,
            // and land among the subnormals.
            let e = if rng.chance(1, 2) { rng.below(80) } else { 2046 - rng.below(80) };
            let s = rng.below(2) << 63;
            f64::from_bits(s | (e << 52) | (rng.next_u64() & 0x000f_ffff_ffff_ffff))
        }
        5 => (rng.below(2_000_000) as f64 - 1_000_000.0) / 10f64.powi(rng.below(8) as i32),
        6 => (rng.below(1000) as f64) * 10f64.powi(rng.below(60) as i32 - 30),
        _ => rng.below(1 << 20) as f64,
    }
}

/// An operand pair: independent, or the second near the first (sums that
/// cancel, quotients near one, ties).
fn pair(rng: &mut Rng) -> (f64, f64) {
    let a = operand(rng);
    let b = match rng.below(4) {
        0 => {
            let d = rng.below(64) as i64 - 32;
            let b = f64::from_bits(a.to_bits().wrapping_add(d as u64));
            if rng.chance(1, 2) {
                -b
            } else {
                b
            }
        }
        1 => {
            // Same significand, nearby exponent.
            let shift = (rng.below(120) as i64 - 60) << 52;
            f64::from_bits(a.to_bits().wrapping_add(shift as u64))
        }
        _ => operand(rng),
    };
    (a, b)
}

/// A natural number to convert: small, around `2^53`, or anywhere.
fn natural(rng: &mut Rng) -> u64 {
    match rng.below(3) {
        0 => rng.below(1 << 20),
        1 => (1u64 << (53 + rng.below(8))) + rng.below(64) - 32,
        _ => rng.next_u64() >> rng.below(64),
    }
}

fn module(data: &Path) -> String {
    format!(
        "import Contract.Number\nimport Contract.OracleText\nopen Contract\n\n\
def hex16 (x : UInt64) : String :=\n  String.ofList ((List.range 16).reverse.map fun i => Nat.digitChar ((x.toNat >>> (4 * i)) % 16))\n\
def cb (x : F64) : String := hex16 (Number.canonicalBits x)\n\
def bit (p : Bool) : String := if p then \"1\" else \"0\"\n\n\
def main : IO Unit := do\n  let out ← IO.getStdout\n  for line in (← IO.FS.lines {data:?}) do\n    \
match line.splitOn \" \" with\n    \
| [sa, sb, sn] =>\n      \
match OracleText.hexN 16 0 sa.toList, OracleText.hexN 16 0 sb.toList, sn.toNat? with\n      \
| some (x, _), some (y, _), some n =>\n        \
let a := F64.ofBits (UInt64.ofNat x)\n        let b := F64.ofBits (UInt64.ofNat y)\n        \
out.putStrLn s!\"{{cb (a + b)}} {{cb (a - b)}} {{cb (a * b)}} {{cb (a / b)}} {{cb (Number.fmod a b)}} {{cb a.floor}} {{cb (Number.fmax a b)}} {{cb (Number.fmin a b)}} {{cb (Number.trunc a)}} {{cb (-a)}} {{cb (F64.ofNat n)}} {{bit (decide (a < b))}}{{bit (decide (a ≤ b))}}{{bit (a == b)}}\"\n      \
| _, _, _ => out.putStrLn \"?\"\n    \
| _ => out.putStrLn \"?\"\n",
        data = data.display().to_string()
    )
}

/// Run one batch in one Lean process: its output lines.
fn run_batch(lines: &[String], dir: &Path, i: usize) -> Result<Vec<String>, String> {
    let data = dir.join(format!("arith-{}-{i}.txt", std::process::id()));
    let file = dir.join(format!("arith-{}-{i}.lean", std::process::id()));
    std::fs::write(&data, lines.join("\n") + "\n").map_err(|e| format!("{}: {e}", data.display()))?;
    std::fs::write(&file, module(&data)).map_err(|e| format!("{}: {e}", file.display()))?;
    let out = Command::new(leanrun::lake())
        .args(["env", "lean", "--run"])
        .arg(&file)
        .current_dir(leanrun::project())
        .output()
        .map_err(|e| format!("lake env lean: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{}: lean failed:\n{}",
            file.display(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let _ = std::fs::remove_file(&data);
    let _ = std::fs::remove_file(&file);
    Ok(String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect())
}

/// `difftest arith`: `count` operand pairs from `seed`; true when every
/// operation agreed.
pub fn run(seed: u64, count: usize, dir: &Path) -> Result<bool, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut rng = Rng::new(seed);
    let cases: Vec<(f64, f64, u64)> = (0..count)
        .map(|_| {
            let (a, b) = pair(&mut rng);
            (a, b, natural(&mut rng))
        })
        .collect();
    let lines: Vec<String> = cases
        .iter()
        .map(|(a, b, n)| format!("{:016x} {:016x} {n}", a.to_bits(), b.to_bits()))
        .collect();
    let jobs = std::env::var("DIFFTEST_JOBS")
        .ok()
        .and_then(|j| j.parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1)));
    let chunk = count.div_ceil(jobs).max(1);
    let start = std::time::Instant::now();
    let results: Vec<Result<Vec<String>, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = lines
            .chunks(chunk)
            .enumerate()
            .map(|(i, part)| scope.spawn(move || run_batch(part, dir, i)))
            .collect();
        handles.into_iter().map(|h| h.join().expect("arith batch")).collect()
    });
    let elapsed = start.elapsed().as_secs_f64();
    let mut lean = Vec::with_capacity(count);
    for r in results {
        lean.extend(r?);
    }
    if lean.len() != count {
        return Err(format!("arith: expected {count} lines from Lean, got {}", lean.len()));
    }
    let mut bad = [0usize; OPS.len()];
    let mut shown = 0;
    for ((a, b, n), l) in cases.iter().zip(&lean) {
        let r = expected(*a, *b, *n);
        if &r == l {
            continue;
        }
        let rs: Vec<&str> = r.split(' ').collect();
        let ls: Vec<&str> = l.split(' ').collect();
        for k in 0..OPS.len() {
            let (rv, lv) = if k < 11 {
                (rs.get(k).copied().unwrap_or(""), ls.get(k).copied().unwrap_or(""))
            } else {
                let at = |v: &[&str]| v.get(11).and_then(|s| s.get(k - 11..k - 10)).unwrap_or("").to_string();
                if at(&rs) == at(&ls) {
                    continue;
                }
                bad[k] += 1;
                if shown < 20 {
                    shown += 1;
                    println!("DIVERGE {} a={:016x} b={:016x}: rust {} lean {}", OPS[k], a.to_bits(), b.to_bits(), at(&rs), at(&ls));
                }
                continue;
            };
            if rv != lv {
                bad[k] += 1;
                if shown < 20 {
                    shown += 1;
                    println!("DIVERGE {} a={:016x} b={:016x} n={n}: rust {rv} lean {lv}", OPS[k], a.to_bits(), b.to_bits());
                }
            }
        }
    }
    let total: usize = bad.iter().sum();
    let per: Vec<String> = OPS.iter().zip(bad).map(|(o, b)| format!("{o} {b}")).collect();
    println!(
        "difftest: {count} operand pairs, {} operations, {total} disagreed ({}); Lean side {:.1}s, {:.0} pairs/s over {jobs} processes",
        count * OPS.len(),
        per.join(", "),
        elapsed,
        count as f64 / elapsed
    );
    Ok(total == 0)
}
