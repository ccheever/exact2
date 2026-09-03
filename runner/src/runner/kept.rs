//! Kept answers (LLP 1027 D4, as ruled 2026-09-03): a store-reading
//! resource's last fresh answer, persisted beside the app's secrets under
//! [`Store::KEPT`] so the next launch's first frame shows it before the
//! data source is ready — a TypeScript module its host loads after first
//! pixel — and the runner asks again at `data_ready`.
//!
//! Encoded as the arguments (one canonical list) and the value, hex, joined
//! by `|`; a kept answer whose arguments no longer match, whose bytes no
//! longer decode, or whose value no longer fits the declared shape is
//! simply not used, and the compiled empty-store placeholder stands.

use super::{DataSource, Runner, RunnerError};
use crate::store::Store;
use exact_kernel::CommitReceipt;
use exact_plan::Value;

/// The largest kept answer, encoded: a session, a list of names — never a feed.
pub(super) const MAX_KEPT_BYTES: usize = 8 * 1024;

/// The store name a resource's kept answer lives under.
pub(super) fn kept_name(resource: &str) -> String {
    format!("{}{resource}", Store::KEPT)
}

pub(super) fn encode(args: &[Value], value: &Value) -> String {
    let mut s = hex(&Value::list(args.to_vec()).to_bytes());
    s.push('|');
    s.push_str(&hex(&value.to_bytes()));
    s
}

pub(super) fn decode(text: &str) -> Option<(Vec<Value>, Value)> {
    let (a, v) = text.split_once('|')?;
    let args = match Value::from_bytes(&unhex(a)?).ok()? {
        Value::List(items) => items.to_vec(),
        _ => return None,
    };
    let value = Value::from_bytes(&unhex(v)?).ok()?;
    Some((args, value))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

impl<D: DataSource> Runner<D> {
    /// Keep a store-reading resource's fresh answer for the next boot's
    /// first frame (LLP 1027 D4), when this source may not be ready then.
    pub(super) fn keep_answer(&mut self, i: usize, args: &[Value], value: &Value) {
        if !self.keeps_answers || !self.store_readers[i] {
            return;
        }
        let encoded = encode(args, value);
        if encoded.len() > MAX_KEPT_BYTES {
            return;
        }
        let name = kept_name(self.plan.str(self.plan.resources[i].name));
        self.store.keep(&name, &encoded);
    }

    /// The data source is ready — a host loaded its TypeScript module after
    /// the first pixel (LLP 1027 D4): every store-reading resource shown
    /// from a placeholder is asked again, in one commit. `None` when nothing
    /// was waiting, or when the source is still not ready.
    pub fn data_ready(&mut self) -> Result<Option<CommitReceipt>, RunnerError> {
        if !self.data.ready() {
            self.log("data_ready: the data source is not ready");
            return Ok(None);
        }
        let stale: Vec<usize> = (0..self.stale.len()).filter(|i| self.stale[*i]).collect();
        self.recommit(stale, "data_ready")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_answer_round_trips_and_a_damaged_one_is_not_used() {
        let args = vec![Value::str("mv"), Value::Number(3.0)];
        let value = Value::record(vec![Value::Bool(true), Value::str("ada")]);
        let text = encode(&args, &value);
        let (a, v) = decode(&text).unwrap();
        assert_eq!(a, args);
        assert_eq!(v.to_bytes(), value.to_bytes());
        assert!(decode("").is_none());
        assert!(decode("zz|00").is_none());
        assert!(decode(&text[..text.len() - 1]).is_none());
        assert_eq!(kept_name("remembered"), "exact.kept.remembered");
    }
}
