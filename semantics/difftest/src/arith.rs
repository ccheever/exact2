//! `difftest arith`: the semantics' binary64 model (`Contract.Binary64`)
//! against this machine's `f64`, operation by operation.
//!
//! For each operand pair `(a, b)` (random bits, edge values and their
//! neighbours, near-cancelling pairs, integers around `2^53`, operands near
//! overflow and underflow) and natural number `n`, both sides compute
//! `a + b`, `a - b`, `a * b`, `a / b`, `a % b`, `floor(a)`, `max(a, b)`,
//! `min(a, b)`, `trunc(a)`, `-a`, `n` as a double, `ceil(a)`, `round(a)`
//! (JavaScript's `Math.round`), and `a < b`, `a <= b`, `a == b`, as the
//! runner does (`f64` operators, `f64::floor`, `f64::max`, `f64::min`,
//! `f64::trunc`, `f64::ceil`, `stdlib::js_round`). Results are compared by
//! bits, every NaN one. Each case also carries a numeral and two dates:
//! `parseNumber` of the numeral and `calendarDiff` of the dates in years and
//! in months (LLP 1102 §3.1, §3.4), as `exact_runner::stdlib` reads them;
//! and two digit counts: `toFixed(a, d)`, `formatDecimal(a, g)` and
//! `formatDecimal(trunc(a), g)` (§3.2), as the `format` capability prints.

use crate::leanrun;
use crate::rng::Rng;
use exact_plan::{Stdlib, Value};
use exact_runner::{formatting, stdlib};
use std::path::Path;
use std::process::Command;

const OPS: [&str; 22] = [
    "+",
    "-",
    "*",
    "/",
    "%",
    "floor",
    "max",
    "min",
    "trunc",
    "neg",
    "ofNat",
    "ceil",
    "round",
    "<",
    "<=",
    "==",
    "parseNumber",
    "years",
    "months",
    "toFixed",
    "formatDecimal",
    "formatDecimal∘trunc",
];
/// The fields printed in hex, then one of three comparison bits, then the
/// text reads.
const HEX: usize = 13;
const BITS: usize = HEX + 3;

fn canon(x: f64) -> u64 {
    if x.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        x.to_bits()
    }
}

/// One case: two operands, a natural number, a numeral and two dates.
struct Case {
    a: f64,
    b: f64,
    n: u64,
    numeral: String,
    from: String,
    to: String,
    /// `toFixed`'s digits (0–100) and `formatDecimal`'s (0–20).
    d: u32,
    g: u32,
}

/// The runner's side of one line, in the Lean module's format.
fn expected(c: &Case) -> String {
    let (a, b, n) = (c.a, c.b, c.n);
    let bit = |p: bool| if p { '1' } else { '0' };
    let opt = |v: Option<String>| v.unwrap_or_else(|| "none".into());
    let diff = |months| opt(stdlib::calendar_diff(&c.from, &c.to, months).map(|n| n.to_string()));
    let fmt =
        |f, x: f64, d: u32| match formatting(f, &[Value::Number(x), Value::Number(f64::from(d))]) {
            Some(v) => format!("[{}]", v.as_str().unwrap_or("?")),
            None => "refused".into(),
        };
    format!(
        "{:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {}{}{} {} {} {} {} {} {}",
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
        canon(a.ceil()),
        canon(stdlib::js_round(a)),
        bit(a < b),
        bit(a <= b),
        bit(a == b),
        opt(stdlib::parse_number(&c.numeral).map(|x| format!("{:016x}", canon(x)))),
        diff(false),
        diff(true),
        fmt(Stdlib::ToFixed, a, c.d),
        fmt(Stdlib::FormatDecimal, a, c.g),
        fmt(Stdlib::FormatDecimal, a.trunc(), c.g),
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
    f64::from_bits(0x000f_ffff_ffff_ffff), // the largest subnormal
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
            let e = if rng.chance(1, 2) {
                rng.below(80)
            } else {
                2046 - rng.below(80)
            };
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

/// Numerals at the grammar's edges and at rounding's: the largest finite
/// and just past it, the smallest subnormal's half on either side, a
/// halfway case, `2^53 + 1`, signed zeros, and text that is not a numeral.
const NUMERALS: &[&str] = &[
    "0",
    "-0",
    "+0",
    "-0.0e5",
    "5.",
    ".5",
    "+.5",
    "-.5e1",
    "5.e3",
    "1E5",
    "00012",
    "1e-7",
    "1.7976931348623157e308",
    "1.7976931348623158e308",
    "1.7976931348623159e308",
    "1e309",
    "2.4703282292062327e-324",
    "2.4703282292062328e-324",
    "4.9406564584124654e-324",
    "1e-400",
    "0e999999999999",
    "1e999999999999",
    "1e-999999999999",
    "9007199254740993",
    "2.2250738585072011e-308",
    "0.1",
    "123456789012345678901234567890",
    "",
    ".",
    "+",
    "-",
    "e5",
    ".e1",
    "1e",
    "1e+",
    "1.2.3",
    "0x10",
    "1_000",
    "Infinity",
    "NaN",
    "--1",
    "1,5",
    "١٢",
];

/// A numeral: a listed one, the shortest or a long expansion of a random
/// double with digits changed at its end, or random digits around a dot
/// and an exponent. No whitespace: lines split on spaces.
fn numeral(rng: &mut Rng, x: f64) -> String {
    match rng.below(4) {
        0 => NUMERALS[rng.below(NUMERALS.len() as u64) as usize].into(),
        1 if x.is_finite() => {
            let digits = rng.below(40) as usize + 1;
            let mut s = format!("{x:.digits$e}");
            if rng.chance(1, 2) {
                let at = s.find('e').unwrap_or(s.len());
                let tail = [
                    "5",
                    "49999999999999999999",
                    "50000000000000000001",
                    "1",
                    "0",
                ];
                s.insert_str(at, tail[rng.below(tail.len() as u64) as usize]);
            }
            s
        }
        3 if rng.chance(1, 64) => {
            // Past std's exponent reach (65,536), which `exact_num` shares:
            // overflowing, underflowing and in range by the exponent.
            let zeros = 65_000 + rng.below(6_000) as usize;
            let e = zeros as i64 + rng.below(700) as i64 - 350;
            format!("0.{}{}e{e}", "0".repeat(zeros), 1 + rng.below(9))
        }
        _ => {
            let mut s = String::new();
            if rng.chance(1, 3) {
                s.push(if rng.chance(1, 2) { '-' } else { '+' });
            }
            for _ in 0..rng.below(25) {
                s.push(char::from(b'0' + rng.below(10) as u8));
            }
            if rng.chance(1, 2) {
                s.push('.');
                for _ in 0..rng.below(25) {
                    s.push(char::from(b'0' + rng.below(10) as u8));
                }
            }
            if rng.chance(1, 2) {
                s.push(if rng.chance(1, 2) { 'e' } else { 'E' });
                if rng.chance(1, 2) {
                    s.push(if rng.chance(1, 2) { '-' } else { '+' });
                }
                s.push_str(&rng.below(400).to_string());
            }
            s
        }
    }
}

/// A date, mostly well formed: years at leap rules' edges, months and days
/// at and past their ends, now and then text that is not a date.
fn date(rng: &mut Rng) -> String {
    const YEARS: [u64; 8] = [0, 1900, 2000, 2023, 2024, 2025, 9999, 1970];
    if rng.chance(1, 12) {
        let bad = [
            "2024-1-01",
            "2024-02-29x",
            "20240229",
            "-",
            "2024/02/29",
            "x024-02-29",
        ];
        return bad[rng.below(bad.len() as u64) as usize].into();
    }
    let y = if rng.chance(1, 4) {
        rng.below(10000)
    } else {
        YEARS[rng.below(8) as usize]
    };
    let m = if rng.chance(1, 10) {
        rng.below(14)
    } else {
        rng.below(12) + 1
    };
    let d = if rng.chance(1, 3) {
        [1, 28, 29, 30, 31, 0, 32][rng.below(7) as usize]
    } else {
        rng.below(31) + 1
    };
    format!("{y:04}-{m:02}-{d:02}")
}

fn module(data: &Path) -> String {
    format!(
        "import Contract.Number\nimport Contract.Value\nimport Contract.Format\nimport Contract.OracleText\nopen Contract\n\n\
def hex16 (x : UInt64) : String :=\n  String.ofList ((List.range 16).reverse.map fun i => Nat.digitChar ((x.toNat >>> (4 * i)) % 16))\n\
def cb (x : F64) : String := hex16 (Number.canonicalBits x)\n\
def bit (p : Bool) : String := if p then \"1\" else \"0\"\n\
def opt {{α}} (f : α → String) : Option α → String\n  | some x => f x\n  | none => \"none\"\n\n\
def main : IO Unit := do\n  let out ← IO.getStdout\n  for line in (← IO.FS.lines {data:?}) do\n    \
match line.splitOn \" \" with\n    \
| [sa, sb, sn, num, d1, d2, sd, sg] =>\n      \
match OracleText.hexN 16 0 sa.toList, OracleText.hexN 16 0 sb.toList, sn.toNat?, sd.toNat?, sg.toNat? with\n      \
| some (x, _), some (y, _), some n, some fd, some fg =>\n        \
let a := F64.ofBits (UInt64.ofNat x)\n        let b := F64.ofBits (UInt64.ofNat y)\n        \
out.putStrLn s!\"{{cb (a + b)}} {{cb (a - b)}} {{cb (a * b)}} {{cb (a / b)}} {{cb (Number.fmod a b)}} {{cb a.floor}} {{cb (Number.fmax a b)}} {{cb (Number.fmin a b)}} {{cb (Number.trunc a)}} {{cb (-a)}} {{cb (F64.ofNat n)}} {{cb a.ceil}} {{cb a.jsRound}} {{bit (decide (a < b))}}{{bit (decide (a ≤ b))}}{{bit (a == b)}} {{opt cb (Str.parseNumber num)}} {{opt toString (Str.calendarDiff d1 d2 false)}} {{opt toString (Str.calendarDiff d1 d2 true)}} [{{Format.toFixed a fd}}] [{{Format.formatDecimal a fg}}] [{{Format.formatDecimal (Number.trunc a) fg}}]\"\n      \
| _, _, _, _, _ => out.putStrLn \"?\"\n    \
| _ => out.putStrLn \"?\"\n",
        data = data.display().to_string()
    )
}

/// Run one batch in one Lean process: its output lines.
fn run_batch(lines: &[String], dir: &Path, i: usize) -> Result<Vec<String>, String> {
    let data = dir.join(format!("arith-{}-{i}.txt", std::process::id()));
    let file = dir.join(format!("arith-{}-{i}.lean", std::process::id()));
    std::fs::write(&data, lines.join("\n") + "\n")
        .map_err(|e| format!("{}: {e}", data.display()))?;
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
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

/// `difftest arith`: `count` operand pairs from `seed`; true when every
/// operation agreed.
pub fn run(seed: u64, count: usize, dir: &Path) -> Result<bool, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut rng = Rng::new(seed);
    let cases: Vec<Case> = (0..count)
        .map(|_| {
            let (a, b) = pair(&mut rng);
            let n = natural(&mut rng);
            let numeral = numeral(&mut rng, b);
            let (from, to) = (date(&mut rng), date(&mut rng));
            let (d, g) = (rng.below(101) as u32, rng.below(21) as u32);
            Case {
                a,
                b,
                n,
                numeral,
                from,
                to,
                d,
                g,
            }
        })
        .collect();
    let lines: Vec<String> = cases
        .iter()
        .map(|c| {
            let (a, b, n) = (c.a.to_bits(), c.b.to_bits(), c.n);
            format!(
                "{a:016x} {b:016x} {n} {} {} {} {} {}",
                c.numeral, c.from, c.to, c.d, c.g
            )
        })
        .collect();
    let jobs = std::env::var("DIFFTEST_JOBS")
        .ok()
        .and_then(|j| j.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1))
        });
    let chunk = count.div_ceil(jobs).max(1);
    let start = std::time::Instant::now();
    let results: Vec<Result<Vec<String>, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = lines
            .chunks(chunk)
            .enumerate()
            .map(|(i, part)| scope.spawn(move || run_batch(part, dir, i)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("arith batch"))
            .collect()
    });
    let elapsed = start.elapsed().as_secs_f64();
    let mut lean = Vec::with_capacity(count);
    for r in results {
        lean.extend(r?);
    }
    if lean.len() != count {
        return Err(format!(
            "arith: expected {count} lines from Lean, got {}",
            lean.len()
        ));
    }
    let mut bad = [0usize; OPS.len()];
    let mut shown = 0;
    for (c, l) in cases.iter().zip(&lean) {
        let (a, b, n) = (&c.a, &c.b, c.n);
        let r = expected(c);
        if &r == l {
            continue;
        }
        let rs: Vec<&str> = r.split(' ').collect();
        let ls: Vec<&str> = l.split(' ').collect();
        for k in 0..OPS.len() {
            let field = if k < HEX {
                k
            } else if k < BITS {
                HEX
            } else {
                k - BITS + HEX + 1
            };
            let (rv, lv) = if !(HEX..BITS).contains(&k) {
                (
                    rs.get(field).copied().unwrap_or(""),
                    ls.get(field).copied().unwrap_or(""),
                )
            } else {
                let at = |v: &[&str]| {
                    v.get(HEX)
                        .and_then(|s| s.get(k - HEX..k - HEX + 1))
                        .unwrap_or("")
                        .to_string()
                };
                if at(&rs) == at(&ls) {
                    continue;
                }
                bad[k] += 1;
                if shown < 20 {
                    shown += 1;
                    println!(
                        "DIVERGE {} a={:016x} b={:016x}: rust {} lean {}",
                        OPS[k],
                        a.to_bits(),
                        b.to_bits(),
                        at(&rs),
                        at(&ls)
                    );
                }
                continue;
            };
            if rv != lv {
                bad[k] += 1;
                if shown < 20 {
                    shown += 1;
                    println!(
                        "DIVERGE {} a={:016x} b={:016x} n={n} numeral={} dates={} {}: rust {rv} lean {lv}",
                        OPS[k],
                        a.to_bits(),
                        b.to_bits(),
                        c.numeral,
                        c.from,
                        c.to
                    );
                }
            }
        }
    }
    let total: usize = bad.iter().sum();
    let per: Vec<String> = OPS
        .iter()
        .zip(bad)
        .map(|(o, b)| format!("{o} {b}"))
        .collect();
    println!(
        "difftest: {count} operand pairs, {} operations, {total} disagreed ({}); Lean side {:.1}s, {:.0} pairs/s over {jobs} processes",
        count * OPS.len(),
        per.join(", "),
        elapsed,
        count as f64 / elapsed
    );
    Ok(total == 0)
}
