//! The IR, `tamber.json` (LLP 1121 D2, D7): the folded table every
//! generator, the compiler and the editor read. One token a line, so a
//! golden's diff names what moved.

use crate::check::DialValue;
use crate::Folding;

/// The IR's format number; a reader refuses another.
pub const FORMAT: u32 = 1;

pub(crate) fn dial_text(v: &DialValue) -> String {
    match v {
        DialValue::Color(f) => f.css(),
        DialValue::Number(n) => exact_color::number_text(*n),
        DialValue::Length(n) => format!("{}px", exact_color::number_text(*n)),
    }
}

fn string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn strings<'a>(items: impl IntoIterator<Item = &'a str>, out: &mut String) {
    out.push('[');
    for (i, s) in items.into_iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        string(s, out);
    }
    out.push(']');
}

/// FNV-1a, 64-bit: the design's digest, over the IR without it.
fn digest(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

pub(crate) fn write(f: &Folding) -> String {
    let d = &f.design;
    let mut body = String::new();
    body.push_str("  \"name\": ");
    string(&d.name, &mut body);
    body.push_str(&format!(",\n  \"tuples\": {},\n  \"axes\": [", f.tuples));
    for (i, a) in d.axes.iter().enumerate() {
        body.push_str(if i == 0 {
            "\n    {\"name\": "
        } else {
            ",\n    {\"name\": "
        });
        string(&a.name.value, &mut body);
        body.push_str(", \"values\": ");
        strings(a.values.iter().map(String::as_str), &mut body);
        body.push_str(if a.under_dark {
            ", \"under\": \"dark\"}"
        } else {
            ", \"under\": null}"
        });
    }
    body.push_str(if d.axes.is_empty() {
        "],\n  \"dials\": ["
    } else {
        "\n  ],\n  \"dials\": ["
    });
    for (i, (dial, value)) in d.dials.iter().zip(&f.dials).enumerate() {
        body.push_str(if i == 0 {
            "\n    {\"name\": "
        } else {
            ",\n    {\"name\": "
        });
        string(&dial.name.value, &mut body);
        body.push_str(&format!(
            ", \"kind\": \"{}\", \"value\": ",
            dial.kind.name()
        ));
        string(value, &mut body);
        body.push_str(", \"range\": ");
        match &dial.range {
            Some((lo, hi)) => strings([lo.as_str(), hi.as_str()], &mut body),
            None => body.push_str("null"),
        }
        body.push_str(", \"meta\": {");
        for (j, (k, v)) in dial.meta.iter().enumerate() {
            if j > 0 {
                body.push_str(", ");
            }
            string(k, &mut body);
            body.push_str(": ");
            string(v, &mut body);
        }
        body.push_str("}}");
    }
    body.push_str(if d.dials.is_empty() {
        "],\n  \"tokens\": ["
    } else {
        "\n  ],\n  \"tokens\": ["
    });
    for (i, t) in f.tokens.iter().enumerate() {
        body.push_str(if i == 0 {
            "\n    {\"name\": "
        } else {
            ",\n    {\"name\": "
        });
        string(&t.name, &mut body);
        body.push_str(", \"kind\": \"color\", \"role\": ");
        match &t.role {
            Some(r) => string(r, &mut body),
            None => body.push_str("null"),
        }
        if t.fixed {
            body.push_str(", \"scheme\": \"fixed\"");
        }
        // D5's mode key: distinct values, and per full tuple an index into
        // them when the token varies.
        let mut values: Vec<String> = Vec::new();
        let mut entries = Vec::new();
        for v in &t.values {
            let text = v.css();
            let at = values.iter().position(|x| *x == text).unwrap_or_else(|| {
                values.push(text);
                values.len() - 1
            });
            entries.push(at.to_string());
        }
        body.push_str(", \"values\": ");
        strings(values.iter().map(String::as_str), &mut body);
        body.push_str(", \"entries\": ");
        if values.len() > 1 {
            body.push_str(&format!("[{}]", entries.join(", ")));
        } else {
            body.push_str("null");
        }
        body.push_str(", \"dials\": ");
        strings(t.dials.iter().map(String::as_str), &mut body);
        body.push_str(", \"fallback\": ");
        match &f.fallbacks[i] {
            Some([l, d]) => strings([l.as_str(), d.as_str()], &mut body),
            None => body.push_str("null"),
        }
        body.push('}');
    }
    body.push_str(if f.tokens.is_empty() {
        "],\n  \"exports\": "
    } else {
        "\n  ],\n  \"exports\": "
    });
    match &d.exports {
        Some(e) => strings(e.iter().map(|n| n.value.as_str()), &mut body),
        None => body.push_str("null"),
    }
    let mut out = format!(
        "{{\n  \"format\": {FORMAT},\n  \"digest\": \"{}\",\n",
        digest(&body)
    );
    out.push_str(&body);
    out.push_str("\n}\n");
    out
}
