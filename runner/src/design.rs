//! The live table's wire (LLP 1121 D13, stage 4): a plan's design values,
//! their digests, and the batch a development build applies without a new
//! plan.
//!
//! A **batch** carries the tokens a value-only edit changed, each as wire
//! text (colours as CSS text, as LLP 1095's wire carries them) with its
//! slice per app tuple where it varies, the plan's structure digest, the
//! design digest it applies to (`base`) and the new one. A build applies it
//! only when `base` is the design it runs; otherwise it fetches the
//! **snapshot** (`/__dev/design`): every token, no base, applied when the
//! structure is its plan's. A **notice** (a batch of no tokens, [`notice`])
//! tells a build that connects, or missed a batch, which design to run.
//! Each process interns what it parses itself, so no wide colour id
//! crosses.

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

/// A batch or a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// The plan structure it is for ([`structure`]).
    pub structure: String,
    /// The design digest it applies to; `None` for a snapshot.
    pub base: Option<String>,
    /// The design digest once applied.
    pub design: String,
    /// The tokens it sets.
    pub tokens: Vec<Token>,
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
    }
}

/// A snapshot of design `tokens`: every token, no base.
pub fn snapshot(structure: &str, tokens: &[Token]) -> Edit {
    Edit {
        structure: structure.to_string(),
        base: None,
        design: digest(tokens),
        tokens: tokens.to_vec(),
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
        serde_json::json!({
            "kind": if self.base.is_some() { "tamber-batch" } else { "tamber-snapshot" },
            "structure": self.structure,
            "base": self.base,
            "design": self.design,
            "tokens": tokens,
        })
        .to_string()
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
        Some(Edit {
            structure: s("structure")?,
            base,
            design: s("design")?,
            tokens,
        })
    }
}
