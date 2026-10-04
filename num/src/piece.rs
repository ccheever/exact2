//! Text written straight into a `String`, byte for byte as `{}` writes it,
//! without `core::fmt`: `fmt::write`, `Formatter::pad` and the integer
//! printers are then not on the paths that use these, so the first module
//! of a split page need not carry them (LLP 1047 §10, the core diet).

use crate::text::ascii;
use crate::{Shortest, Shortest32};
use std::rc::Rc;

/// A value that appends its `{}` text to a `String`.
pub trait Piece {
    /// Append this value's text.
    fn push_to(&self, out: &mut String);
}

impl<T: Piece + ?Sized> Piece for &T {
    fn push_to(&self, out: &mut String) {
        (**self).push_to(out);
    }
}

impl Piece for str {
    fn push_to(&self, out: &mut String) {
        out.push_str(self);
    }
}

impl Piece for String {
    fn push_to(&self, out: &mut String) {
        self.as_str().push_to(out);
    }
}

impl Piece for Rc<str> {
    fn push_to(&self, out: &mut String) {
        (**self).push_to(out);
    }
}

impl Piece for char {
    fn push_to(&self, out: &mut String) {
        out.push(*self);
    }
}

impl Piece for bool {
    fn push_to(&self, out: &mut String) {
        out.push_str(if *self { "true" } else { "false" });
    }
}

macro_rules! unsigned {
    ($($t:ty),*) => {$(
        impl Piece for $t {
            fn push_to(&self, out: &mut String) {
                push_u64(out, *self as u64);
            }
        }
    )*};
}
unsigned!(u8, u16, u32, u64, usize);

macro_rules! signed {
    ($($t:ty),*) => {$(
        impl Piece for $t {
            fn push_to(&self, out: &mut String) {
                push_i64(out, *self as i64);
            }
        }
    )*};
}
signed!(i32, i64, isize);

impl Piece for Shortest {
    fn push_to(&self, out: &mut String) {
        if !few_places(self.0, out) {
            // Writing to a `String` cannot fail.
            let _ = self.write(out);
        }
    }
}

/// `v` as `{}` writes it when its shortest text has at most three decimal
/// places and it is below 10^9 in magnitude (most numbers an app shows), by
/// trying 0 to 3 places in turn: the first `n / 10^k` that reads back as
/// `v` is the shortest text, and the only one at `k` places, an ulp of `v`
/// being far below 10^-3. False, writing nothing, for any other value.
fn few_places(v: f64, out: &mut String) -> bool {
    if v == 0.0 || !v.is_finite() || v.abs() >= 1e9 {
        return false;
    }
    for (k, scale) in [1.0, 10.0, 100.0, 1000.0].into_iter().enumerate() {
        let n = (v * scale).round();
        if n / scale != v {
            continue;
        }
        if v < 0.0 {
            out.push('-');
        }
        let m = n.abs() as u64;
        let unit = 10u64.pow(k as u32);
        push_u64(out, m / unit);
        if k > 0 {
            out.push('.');
            let mut buf = [0; 20];
            let frac = ascii(m % unit, &mut buf);
            for _ in frac.len()..k {
                out.push('0');
            }
            out.push_str(frac);
        }
        return true;
    }
    false
}

impl Piece for Shortest32 {
    fn push_to(&self, out: &mut String) {
        let _ = self.write(out);
    }
}

fn push_u64(out: &mut String, v: u64) {
    let mut buf = [0; 20];
    out.push_str(ascii(v, &mut buf));
}

fn push_i64(out: &mut String, v: i64) {
    if v < 0 {
        out.push('-');
    }
    push_u64(out, v.unsigned_abs());
}

/// `template` with each `{}` replaced by the next argument's text, `{{`
/// and `}}` by one brace — `format!`'s grammar for `{}` alone — appended
/// to `out`. The text between holes stays one string in the data, as
/// `format!`'s pieces do, so a line costs no more code than it did.
#[inline(never)]
pub fn fill_into(out: &mut String, template: &str, args: &[&dyn Piece]) {
    // Braces are ASCII: every cut falls on a character boundary.
    let bytes = template.as_bytes();
    let (mut start, mut at, mut next) = (0, 0, 0);
    while at < bytes.len() {
        match (bytes[at], bytes.get(at + 1)) {
            (b'{', Some(b'}')) => {
                out.push_str(&template[start..at]);
                if let Some(arg) = args.get(next) {
                    arg.push_to(out);
                }
                next += 1;
            }
            (b'{', Some(b'{')) | (b'}', Some(b'}')) => out.push_str(&template[start..=at]),
            _ => {
                at += 1;
                continue;
            }
        }
        at += 2;
        start = at;
    }
    out.push_str(&template[start..]);
    debug_assert_eq!(next, args.len(), "holes and arguments in {template:?}");
}

/// [`fill_into`] a new `String`.
pub fn fill(template: &str, args: &[&dyn Piece]) -> String {
    let mut out = String::new();
    fill_into(&mut out, template, args);
    out
}

/// `text!("query {}: {}", resource, source)`: `format!` for `{}` holes
/// (and `{{`, `}}`), each argument a [`Piece`], without `core::fmt`.
#[macro_export]
macro_rules! text {
    ($template:expr $(, $arg:expr)* $(,)?) => {
        $crate::fill($template, &[$(&$arg as &dyn $crate::Piece),*])
    };
}

/// `push_text!(out, "{},", id)`: [`text!`] appended to `out`, a `&mut
/// String` — `write!` for `{}` holes, without `core::fmt`.
#[macro_export]
macro_rules! push_text {
    ($out:expr, $template:expr $(, $arg:expr)* $(,)?) => {
        $crate::fill_into($out, $template, &[$(&$arg as &dyn $crate::Piece),*])
    };
}
