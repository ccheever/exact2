//! Kept answers (LLP 1027 D4, as ruled 2026-09-03): a store-reading
//! resource's last fresh answer, persisted beside the app's secrets under
//! [`Store::KEPT`] so the next launch's first frame shows it before the
//! data source is ready — a TypeScript module its host loads after first
//! pixel — and the runner asks again at `data_ready`.
//!
//! Encoded as the identifying arguments (one canonical list) and the value, hex, joined
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
    let mut name = String::from(Store::KEPT);
    name.push_str(resource);
    name
}

pub(super) fn encode(args: &[Value], value: &Value) -> String {
    let mut s = hex(&Value::list(args.to_vec()).to_bytes());
    s.push('|');
    s.push_str(&hex(&value.to_bytes()));
    s
}

// Count canonical bytes only as far as the hex-text budget permits. Iterators
// borrow children: even an enormous list never allocates a traversal-sized stack.
fn fits(args: &[Value], value: &Value) -> bool {
    let mut remaining = (MAX_KEPT_BYTES - 1) / 2 - 5; // separator and args-list header
    let mut stack = vec![std::slice::from_ref(value).iter(), args.iter()];
    while let Some(items) = stack.last_mut() {
        let Some(item) = items.next() else {
            stack.pop();
            continue;
        };
        let (bytes, children) = match item {
            Value::Number(_) => (9, None),
            Value::Bool(_) => (2, None),
            s @ exact_plan::str_value!() => {
                let Some(bytes) = s.text().len().checked_add(5) else {
                    return false;
                };
                (bytes, None)
            }
            Value::Unit | Value::Option(None) => (1, None),
            Value::Option(Some(v)) => (1, Some(std::slice::from_ref(v.as_ref()))),
            Value::List(v) | Value::Record(v) => (5, Some(&v[..])),
        };
        let Some(left) = remaining.checked_sub(bytes) else {
            return false;
        };
        remaining = left;
        if let Some(children) = children {
            // Every child uses at least one canonical byte.
            if children.len() > remaining {
                return false;
            }
            stack.push(children.iter());
        }
    }
    true
}

pub(super) fn decode(text: &str) -> Option<(Vec<Value>, Value)> {
    if text.len() > MAX_KEPT_BYTES {
        return None;
    }
    let (a, v) = text.split_once('|')?;
    let args = match Value::from_bytes(&unhex(a)?).ok()? {
        Value::List(items) => items.to_vec(),
        _ => return None,
    };
    let value = Value::from_bytes(&unhex(v)?).ok()?;
    Some((args, value))
}

/// Two lowercase digits a byte (`{:02x}`'s), without the formatter: a kept
/// answer is written on a first press's path.
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 15)]));
    }
    out
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    fn digit(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    s.as_bytes()
        .chunks_exact(2)
        .map(|pair| Some(digit(pair[0])? * 16 + digit(pair[1])?))
        .collect()
}

impl<D: DataSource> Runner<D> {
    /// Keep a store-reading resource's fresh answer for the next boot's
    /// first frame (LLP 1027 D4), when this source may not be ready then.
    pub(super) fn keep_answer(&mut self, i: usize, args: &[Value], value: &Value) {
        if !self.keeps_answers || !self.store_readers[i] {
            return;
        }
        // @ref LLP 1039 D3 / LLP 1030 D7 — runner facts are never kept answers.
        let source = self.plan.str(self.plan.resources[i].source);
        if source == crate::viewport::SOURCE
            || source == crate::time::SOURCE
            || source == crate::page::SOURCE
            || source == crate::delivery::SOURCE
            || source == crate::surface_record::SOURCE
        {
            return;
        }
        // @ref LLP 1027.005 D6 — context reaches the source but does not
        // enter the kept entry or its 8 KB budget. With no context these
        // are the exact same arguments and encoding as before.
        let row = &self.plan.resources[i];
        let args = &args[..row.args.len as usize - usize::from(row.context)];
        if !fits(args, value) {
            return;
        }
        let encoded = encode(args, value);
        let name = kept_name(self.plan.str(self.plan.resources[i].name));
        self.store.keep(&name, &encoded);
    }

    /// The data source is ready — a host loaded its TypeScript module after
    /// the first pixel (LLP 1027 D4): every deferred resource shown
    /// from a placeholder is asked again, and every send made before now
    /// is sent, in one commit. `None` when nothing was waiting, or when the
    /// source is still not ready.
    pub fn data_ready(&mut self) -> Result<Option<CommitReceipt>, RunnerError> {
        if !self.data.ready() {
            self.log("data_ready: the data source is not ready");
            return Ok(None);
        }
        let stale: Vec<usize> = (0..self.stale.len()).filter(|i| self.stale[*i]).collect();
        if self.unsent.is_empty() {
            return self.recommit(stale, "data_ready");
        }
        let what = format!(
            "data_ready ({} asked again, {} sent)",
            stale.len(),
            self.unsent.len()
        );
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(false);
        self.refresh_next.extend(stale);
        let result = if self.poisoned {
            Err(RunnerError::Poisoned)
        } else {
            let later = self.send_unsent();
            // The gate step joins this settlement as every commit's (LLP
            // 1092 D8; b6 review A2): a gate the sent answers open arms its
            // task, and a key that is no key refuses this commit.
            match self
                .router_change()
                .and_then(|_| self.settle(false))
                .and_then(|_| self.gate_step())
            {
                Ok(()) => {
                    for (m, source, args, request) in later {
                        self.enqueue(super::Target::Mutation(m), source, args, request, false);
                    }
                    self.update()
                }
                Err(e) => {
                    self.discard_later(&later);
                    Err(e)
                }
            }
        };
        self.conclude(checkpoint, &result, was_poisoned);
        self.arm_then(result.is_ok());
        self.arm_next(result.is_ok());
        self.log_outcome(&what, &result, was_poisoned);
        result.map(Some)
    }

    /// Each waiting send asks its source now, as the action would have: an
    /// answer lands in the mutation's slot (its `then` armed, what it
    /// refreshes asked again); a request is returned, to go out once the
    /// commit stands. The action that made it committed long ago, so a
    /// source's refusal ends that one send, said in the journal, as a
    /// refused reply does — never this commit.
    #[allow(clippy::type_complexity)]
    fn send_unsent(&mut self) -> Vec<(usize, String, Vec<Value>, super::Request)> {
        let mut later = Vec::new();
        for (m, source, args) in std::mem::take(&mut self.unsent) {
            let target = super::Target::Mutation(m);
            let name = self.plan.str(self.plan.mutations[m].name).to_string();
            let refused = match self
                .data
                .answer_for(target, &mut self.store, &source, &args)
            {
                Ok(super::Answer::Now(value))
                    if self.conforms(&value, self.plan.mutations[m].ty) =>
                {
                    match self.mutation_slot(m) {
                        Ok(slot) => {
                            self.slots[slot] = Value::some(value);
                            self.landed.push(m);
                            for r in self.declared_refreshes(m) {
                                self.force_refresh(r);
                            }
                            None
                        }
                        Err(e) => Some(format!("{e:?}")),
                    }
                }
                Ok(super::Answer::Now(_)) => {
                    Some("its answer does not fit the mutation's shape".to_string())
                }
                Ok(super::Answer::Later(request)) => {
                    self.reread_next.extend(self.declared_refreshes(m));
                    later.push((m, source, args, request));
                    None
                }
                Err(error) => Some(format!("{error:?}")),
            };
            if let Some(why) = refused {
                self.log(super::lines::unsent_refused(&name, &why));
            }
        }
        self.sync_pending_flags();
        // Pending until the request goes out with the commit, or is let go.
        for (m, ..) in &later {
            self.pending_mut[*m] = true;
        }
        later
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
        let valid_args = hex(&Value::list(vec![]).to_bytes());
        for invalid in ["€", "€x", "é", "💬", "a💬a", "0g", "+1"] {
            assert!(decode(&format!("{invalid}|00")).is_none());
            assert!(decode(&format!("{valid_args}|{invalid}")).is_none());
        }
        assert!(decode(&text[..text.len() - 1]).is_none());
        assert_eq!(kept_name("remembered"), "exact.kept.remembered");
    }

    #[test]
    fn hex_is_two_lowercase_digits_a_byte() {
        let every: Vec<u8> = (0..=255).collect();
        let reference = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(&every), reference(&every));
        assert_eq!(hex(&[]), "");
        let value = Value::record(vec![Value::str("é💬"), Value::Number(-2.5)]).to_bytes();
        assert_eq!(hex(&value), reference(&value));
    }

    #[test]
    fn size_refusal_matches_the_wire_budget_including_arguments_and_unicode() {
        let values = [
            Value::Unit,
            Value::NONE,
            Value::some(Value::Number(-0.0)),
            Value::Bool(true),
            Value::str("é💬"),
            Value::list(vec![Value::Bool(false), Value::Unit]),
            Value::record(vec![Value::str("field"), Value::Number(2.0)]),
        ];
        for value in &values {
            for args in [&[][..], &values[..]] {
                assert_eq!(
                    fits(args, value),
                    encode(args, value).len() <= MAX_KEPT_BYTES
                );
            }
        }
        // Hex plus its separator is always odd: 8191 fits, 8193 does not.
        for size in [4084, 4085, 4086, 10000] {
            let value = Value::str(&"x".repeat(size));
            for args in [&[][..], &values[..]] {
                assert_eq!(
                    fits(args, &value),
                    encode(args, &value).len() <= MAX_KEPT_BYTES
                );
            }
        }
        assert!(!fits(&[], &Value::list(vec![Value::Unit; 100_000])));
    }
}
