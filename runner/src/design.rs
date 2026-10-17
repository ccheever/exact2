//! The live table's wire (LLP 1121 D13, stage 4): a plan's design values,
//! their digests, and the batch a development build applies without a new
//! plan.
//!
//! A **batch** carries the tokens a value-only edit changed, each as wire
//! text (colours as CSS text, as LLP 1095's wire carries them) with its
//! slice per app tuple where it varies, the design's shape digest
//! ([`shape`]), the design digest it applies to (`base`) and the new one. A
//! build applies it only when the shape is its plan's and `base` is the
//! design it runs; otherwise it fetches the **snapshot** (`/__dev/design`):
//! every token, no base, applied when the shape is its plan's. A **notice** (a batch of no tokens, [`notice`])
//! tells a build that connects, or missed a batch, which design to run.
//! Each process interns what it parses itself, so no wide colour id
//! crosses.

use exact_kernel::style::theme::{typeface, Kind};
use exact_plan::Plan;
use sha2::{Digest, Sha256};

/// One token's design value: its default tuple's text and, where it varies
/// by an app axis, one text per full tuple in tuple order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The theme row's name (a synthetic token's is its expression).
    pub name: String,
    /// The default tuple's text.
    pub value: String,
    /// Every tuple's text, or none.
    pub slices: Vec<String>,
}

/// A batch or a snapshot. Its `structure` is the design's shape
/// ([`shape`]), which every build compiled from the same sources has, whatever
/// its target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// The design shape it is for ([`shape`]).
    pub structure: String,
    /// The design digest it applies to; `None` for a snapshot.
    pub base: Option<String>,
    /// The design digest once applied.
    pub design: String,
    /// The tokens it sets.
    pub tokens: Vec<Token>,
    /// The scrub frame it is, when it is one (*1.10*).
    pub scrub: Option<Scrub>,
}

/// A scrub's frame (LLP 1121 D13): the scrub's id and the frame's
/// revision, rising through the scrub, its end's the highest. A build drops
/// a frame or snapshot of the scrub it last applied one of whose revision
/// is not above that one's, and applies a scrub's frames with no transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scrub {
    /// The scrub's id.
    pub id: String,
    /// The frame's revision.
    pub rev: u64,
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The digest of `plan`'s structure ([`Plan::structure`]): equal for two
/// plans that differ only in design values.
pub fn structure(plan: &Plan) -> String {
    hex(&plan.structure())
}

/// The digest of `plan`'s design shape: its axes, its `@keyframes`, and each theme row's name,
/// role and slice count: which tokens override which role, not with what
/// (an override's value is a live edit, LLP 1121 stage 5). A batch is for one
/// shape: the JS target's build and a wasm or native build of the same
/// sources share it, though their plans differ (LLP 1122 D4's one stream
/// for every copy).
pub fn shape(plan: &Plan) -> String {
    let mut bytes = Vec::new();
    let mut text = |t: &str| {
        bytes.extend_from_slice(&(t.len() as u32).to_le_bytes());
        bytes.extend_from_slice(t.as_bytes());
    };
    for a in &plan.axes {
        text(plan.str(a.name));
        text(plan.str(a.values));
    }
    text("");
    for row in &plan.theme {
        text(plan.str(row.name));
        text(plan.str(row.role));
        text(&row.slices.len.to_string());
        // A kind is which rows may name the token: a change is structure.
        text(&row.kind.to_string());
        // A type token's longhands and family, a family token's stack, each
        // tuple's (stage 6 C, the lead's ruling): which rows it writes and
        // which stack it names are lowered, so a change is structure.
        if let Some(k @ (Kind::Type | Kind::Family)) = Kind::of(row.kind) {
            let values = std::iter::once(row.value).chain(
                plan.theme_slices[row.slices.start as usize..][..row.slices.len as usize]
                    .iter()
                    .map(|s| s.value),
            );
            for value in values {
                let value = plan.str(value);
                text(&match k {
                    Kind::Family => value.trim().to_string(),
                    _ => typeface::parts(value).map_or_else(String::new, |p| {
                        format!(
                            "{} {} {} {:?}",
                            p.style.is_some(),
                            p.weight.is_some(),
                            p.leading.is_some(),
                            p.family
                        )
                    }),
                });
            }
        }
    }
    // The `@keyframes` rules, which a token named in a colour keyframe is
    // folded into when the app compiles: an edit to it changes a rule, so it
    // is a rebuild, never a batch that would leave the rule's old colour.
    text("");
    for row in &plan.keyframes {
        text(plan.str(row.name));
        text(plan.str(row.css));
    }
    hex(&bytes)
}

/// `plan`'s design values, one per theme row, in table order.
pub fn tokens(plan: &Plan) -> Vec<Token> {
    plan.theme
        .iter()
        .map(|row| Token {
            name: plan.str(row.name).to_string(),
            value: plan.str(row.value).to_string(),
            slices: plan.theme_slices[row.slices.start as usize..][..row.slices.len as usize]
                .iter()
                .map(|s| plan.str(s.value).to_string())
                .collect(),
        })
        .collect()
}

/// The digest of a design's values.
pub fn digest(tokens: &[Token]) -> String {
    let mut bytes = Vec::new();
    for t in tokens {
        for text in std::iter::once(&t.name)
            .chain(std::iter::once(&t.value))
            .chain(&t.slices)
        {
            bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
        }
        bytes.extend_from_slice(&(t.slices.len() as u32).to_le_bytes());
    }
    hex(&bytes)
}

/// The batch from design `from` to design `to` of one structure: the tokens
/// whose value or slices changed. `None` when the two differ in their
/// tokens' names (a structure change).
pub fn batch(structure: &str, from: &[Token], to: &[Token]) -> Option<Edit> {
    if from.len() != to.len() || from.iter().zip(to).any(|(a, b)| a.name != b.name) {
        return None;
    }
    Some(Edit {
        structure: structure.to_string(),
        base: Some(digest(from)),
        design: digest(to),
        tokens: from
            .iter()
            .zip(to)
            .filter(|(a, b)| a != b)
            .map(|(_, b)| b.clone())
            .collect(),
        scrub: None,
    })
}

/// A notice that the design is now `design`: a batch with no tokens whose
/// base is that design, so a build that runs it applies nothing and one
/// that does not answers stale and fetches the snapshot. The dev stream
/// sends it on connect, and in place of a batch too large for a frame.
pub fn notice(structure: &str, design: &str) -> Edit {
    Edit {
        structure: structure.to_string(),
        base: Some(design.to_string()),
        design: design.to_string(),
        tokens: Vec::new(),
        scrub: None,
    }
}

/// A snapshot of design `tokens`: every token, no base.
pub fn snapshot(structure: &str, tokens: &[Token]) -> Edit {
    Edit {
        structure: structure.to_string(),
        base: None,
        design: digest(tokens),
        tokens: tokens.to_vec(),
        scrub: None,
    }
}

impl Edit {
    /// As the dev stream carries it: `kind` `tamber-batch` or
    /// `tamber-snapshot`.
    pub fn json(&self) -> String {
        let tokens: Vec<serde_json::Value> = self
            .tokens
            .iter()
            .map(|t| serde_json::json!({"name": t.name, "value": t.value, "slices": t.slices}))
            .collect();
        let mut v = serde_json::json!({
            "kind": if self.base.is_some() { "tamber-batch" } else { "tamber-snapshot" },
            "structure": self.structure,
            "base": self.base,
            "design": self.design,
            "tokens": tokens,
        });
        if let Some(s) = &self.scrub {
            v["scrub"] = serde_json::json!({"id": s.id, "rev": s.rev});
        }
        v.to_string()
    }

    /// One read off the dev stream; `None` when it is no batch or snapshot.
    pub fn parse(text: &str) -> Option<Edit> {
        let v: serde_json::Value = serde_json::from_str(text).ok()?;
        let kind = v.get("kind")?.as_str()?;
        if kind != "tamber-batch" && kind != "tamber-snapshot" {
            return None;
        }
        let s = |key: &str| v.get(key)?.as_str().map(str::to_string);
        let tokens = v
            .get("tokens")?
            .as_array()?
            .iter()
            .map(|t| {
                Some(Token {
                    name: t.get("name")?.as_str()?.to_string(),
                    value: t.get("value")?.as_str()?.to_string(),
                    slices: t
                        .get("slices")?
                        .as_array()?
                        .iter()
                        .map(|s| s.as_str().map(str::to_string))
                        .collect::<Option<_>>()?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let base = match kind {
            "tamber-batch" => Some(s("base")?),
            _ => None,
        };
        // A scrub frame's mark is optional; one malformed is no mark.
        let scrub = v.get("scrub").and_then(|m| {
            Some(Scrub {
                id: m.get("id")?.as_str()?.to_string(),
                rev: m.get("rev")?.as_u64()?,
            })
        });
        Some(Edit {
            structure: s("structure")?,
            base,
            design: s("design")?,
            tokens,
            scrub,
        })
    }
}
