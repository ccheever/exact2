//! The uploads a runner has asked (LLP 1016.000 D10): for each resource,
//! the identifying arguments an upload was asked at and how it stands.
//! Nothing asks an upload again at arguments it has a mark for: the
//! resource shows the answer the upload ended with, or its failure. The
//! marks live as long as the app's data: a reload or an update carries
//! them by resource name and arguments, and a launch reads them back from
//! beside the kept answers. An upload a reload, an update or a quit cut
//! off comes back failed, `interrupted`, and is not sent again.

use super::{DataSource, Runner, Target};
use crate::failure::{Failure, FailureCode};
use crate::store::Store;
use exact_plan::Value;

/// Where the marks are kept between launches: beside the kept answers,
/// under a name no resource can have.
pub(super) const UPLOADS: &str = "exact.kept.@uploads";

/// The most the kept marks take, which the kept answers' budget counts
/// (`kept.rs` `KEPT_TOTAL_BYTES`): the oldest are forgotten first.
pub(super) const UPLOADS_BYTES: usize = 64 * 1024;

/// How an upload asked at some arguments stands.
#[derive(Clone, Debug, PartialEq)]
pub enum UploadEnd {
    /// On its way, under this runner's ticket.
    Sent(u64),
    /// The answer it ended with.
    Answered(Value),
    /// Its failure: its source failed to take its end, or it was cut off.
    Failed(Failure),
}

/// One upload: the resource and source that asked it, its identifying
/// arguments, and how it stands.
#[derive(Clone, Debug, PartialEq)]
pub struct UploadMark {
    /// The resource's name.
    pub resource: String,
    /// Its source's name.
    pub source: String,
    /// Its identifying arguments (not its `with` context).
    pub args: Vec<Value>,
    /// How the upload stands.
    pub end: UploadEnd,
}

/// The failure of an upload cut off before its reply.
pub(super) fn interrupted() -> Failure {
    Failure {
        code: FailureCode::Error,
        message: "the upload was cut off before its reply; it is not sent again at these arguments"
            .into(),
        reason: "interrupted".into(),
    }
}

/// The marks, oldest first.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Uploads(pub(super) Vec<UploadMark>);

impl Uploads {
    /// The marks a new runner starts with: the ones carried or kept, each
    /// still on its way cut off, since its ticket was the old runner's.
    pub(super) fn starting(marks: &[UploadMark]) -> Uploads {
        Uploads(
            marks
                .iter()
                .map(|m| UploadMark {
                    end: match &m.end {
                        UploadEnd::Sent(_) => UploadEnd::Failed(interrupted()),
                        end => end.clone(),
                    },
                    ..m.clone()
                })
                .collect(),
        )
    }

    /// How the upload `resource` asked at `args` stands, if it was asked.
    pub(super) fn at(&self, resource: &str, args: &[Value]) -> Option<&UploadEnd> {
        (self.0.iter().rev())
            .find(|m| m.resource == resource && m.args == args)
            .map(|m| &m.end)
    }

    /// `resource` asked an upload at `args`, on its way under `ticket`.
    pub(super) fn sent(&mut self, resource: &str, source: &str, args: &[Value], ticket: u64) {
        self.0
            .retain(|m| !(m.resource == resource && m.args == args));
        self.0.push(UploadMark {
            resource: resource.into(),
            source: source.into(),
            args: args.to_vec(),
            end: UploadEnd::Sent(ticket),
        });
    }

    /// Every mark on its way under `ticket` ends as `end`; whether any did.
    pub(super) fn end(&mut self, ticket: u64, end: &UploadEnd) -> bool {
        let mut any = false;
        for m in self
            .0
            .iter_mut()
            .filter(|m| m.end == UploadEnd::Sent(ticket))
        {
            m.end = end.clone();
            any = true;
        }
        any
    }

    /// Forget the marks under `ticket`: its upload never reached the
    /// network, so asking it again sends nothing twice.
    pub(super) fn unmark(&mut self, ticket: u64) -> bool {
        let before = self.0.len();
        self.0.retain(|m| m.end != UploadEnd::Sent(ticket));
        self.0.len() != before
    }

    /// The marks as kept between launches, the newest that fit
    /// [`UPLOADS_BYTES`], and how many older ones did not.
    fn encoded(&self) -> (Vec<u8>, usize) {
        let marks: Vec<Value> = self.0.iter().map(encode).collect();
        let mut from = 0;
        loop {
            let bytes = Value::list(marks[from..].to_vec()).to_bytes();
            if bytes.len() <= UPLOADS_BYTES || from == marks.len() {
                return (bytes, from);
            }
            from += 1;
        }
    }

    /// The marks kept in `store`, if any decode.
    pub(super) fn kept(store: &Store) -> Vec<UploadMark> {
        let Some(Ok(Value::List(marks))) = store.kept(UPLOADS).map(Value::from_bytes) else {
            return Vec::new();
        };
        marks.iter().filter_map(decode).collect()
    }
}

fn encode(m: &UploadMark) -> Value {
    let (tag, payload) = match &m.end {
        UploadEnd::Sent(_) => (0.0, Value::Unit),
        UploadEnd::Answered(v) => (1.0, v.clone()),
        UploadEnd::Failed(f) => (
            2.0,
            Value::list(vec![
                Value::str(f.code.name()),
                Value::str(&f.message),
                Value::str(&f.reason),
            ]),
        ),
    };
    Value::list(vec![
        Value::str(&m.resource),
        Value::str(&m.source),
        Value::list(m.args.clone()),
        Value::Number(tag),
        payload,
    ])
}

fn decode(v: &Value) -> Option<UploadMark> {
    let Value::List(items) = v else { return None };
    let [resource, source, Value::List(args), Value::Number(tag), payload] = &items[..] else {
        return None;
    };
    let end = match (*tag as u8, payload) {
        (0, _) => UploadEnd::Sent(0),
        (1, v) => UploadEnd::Answered(v.clone()),
        (2, Value::List(f)) => match &f[..] {
            [code, message, reason] => UploadEnd::Failed(Failure {
                code: FailureCode::from_name(code.as_str()?)?,
                message: message.as_str()?.into(),
                reason: reason.as_str()?.into(),
            }),
            _ => return None,
        },
        _ => return None,
    };
    Some(UploadMark {
        resource: resource.as_str()?.into(),
        source: source.as_str()?.into(),
        args: args.to_vec(),
        end,
    })
}

impl<D: DataSource> Runner<D> {
    /// Resource `i`'s identifying arguments among `args`: its `with`
    /// context reaches the source but does not identify the upload.
    pub(super) fn upload_identity<'a>(&self, i: usize, args: &'a [Value]) -> &'a [Value] {
        let row = &self.plan.resources[i];
        let n = (row.args.len as usize).saturating_sub(usize::from(row.context));
        &args[..n.min(args.len())]
    }

    /// How resource `i`'s upload asked at `args` stands, if it was asked.
    pub(super) fn upload_at(&self, i: usize, args: &[Value]) -> Option<&UploadEnd> {
        let name = self.plan.str(self.plan.resources[i].name);
        self.uploads.at(name, self.upload_identity(i, args))
    }

    /// `target` asked an upload at `args`, handed to its host as `ticket`.
    pub(super) fn upload_sent(&mut self, target: Target, args: &[Value], ticket: u64) {
        let Target::Resource(i) = target else { return };
        let row = &self.plan.resources[i];
        let (name, source) = (self.plan.str(row.name), self.plan.str(row.source));
        let identity = self.upload_identity(i, args).to_vec();
        self.uploads.sent(name, source, &identity, ticket);
        self.keep_uploads();
    }

    /// Upload `ticket` is over: `end` when it reached the network, else
    /// forgotten, an ordinary answer that `refresh` asks again.
    pub(super) fn upload_over(&mut self, ticket: u64, sent: bool, end: UploadEnd) {
        let changed = match sent {
            true => self.uploads.end(ticket, &end),
            false => self.uploads.unmark(ticket),
        };
        if changed {
            self.keep_uploads();
        }
    }

    /// What resource `i` answers at arguments its upload was asked at, in
    /// place of asking its source: the answer it ended with, while that
    /// still fits the resource's shape, or its failure.
    pub(super) fn ledger_answer(
        &self,
        i: usize,
        mark: Option<&UploadEnd>,
    ) -> Option<Result<crate::Answer, super::RunnerError>> {
        let failed = |f: &Failure| {
            Err(super::RunnerError::Data {
                resource: self.plan.str(self.plan.resources[i].name).to_string(),
                error: super::DataError::Failed(f.code, f.message.clone(), f.reason.clone()),
            })
        };
        Some(match mark? {
            UploadEnd::Answered(v) if self.check_shape(i, v).is_ok() => {
                Ok(crate::Answer::Now(v.clone()))
            }
            UploadEnd::Answered(_) => failed(&Failure {
                code: FailureCode::Shape,
                message: "the upload's answer no longer fits the resource's shape; it is not sent again at these arguments".into(),
                reason: "interrupted".into(),
            }),
            UploadEnd::Failed(f) => failed(f),
            UploadEnd::Sent(_) => failed(&interrupted()),
        })
    }

    /// Keep the marks for the next launch, beside the kept answers and
    /// within their budget. An app that keeps no answers (LLP 1069.008.000
    /// D7) keeps no marks either: they hold its answers.
    pub(super) fn keep_uploads(&mut self) {
        if crate::device::keeps_no_answers(self.data.grants()) {
            return;
        }
        let (bytes, forgot) = self.uploads.encoded();
        if forgot > 0 {
            let line = format!(
                "kept uploads are at most {} KiB together: forgot the {forgot} oldest, which a launch may send again at their arguments (LLP 1016.000 D10)",
                UPLOADS_BYTES / 1024
            );
            self.log(line);
            self.uploads.0.drain(..forgot);
        }
        if self.uploads.0.is_empty() {
            self.store.forget_kept(UPLOADS);
            return;
        }
        self.make_room(UPLOADS, bytes.len());
        self.store.keep(UPLOADS, bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(args: f64, end: UploadEnd) -> UploadMark {
        UploadMark {
            resource: "photo".into(),
            source: "upload".into(),
            args: vec![Value::Number(args)],
            end,
        }
    }

    #[test]
    fn marks_round_trip_and_a_new_runner_cuts_off_what_was_on_its_way() {
        let failed = Failure {
            code: FailureCode::Offline,
            message: "gone".into(),
            reason: String::new(),
        };
        let uploads = Uploads(vec![
            mark(1.0, UploadEnd::Answered(Value::str("done"))),
            mark(2.0, UploadEnd::Failed(failed)),
            mark(3.0, UploadEnd::Sent(7)),
        ]);
        let (bytes, forgot) = uploads.encoded();
        assert_eq!(forgot, 0);
        let Ok(Value::List(kept)) = Value::from_bytes(&bytes) else {
            panic!("a list")
        };
        let kept: Vec<UploadMark> = kept.iter().filter_map(decode).collect();
        let started = Uploads::starting(&kept);
        assert_eq!(started.0[..2], uploads.0[..2]);
        assert_eq!(started.0[2].end, UploadEnd::Failed(interrupted()));
    }

    #[test]
    fn an_end_reaches_every_mark_of_its_ticket_and_an_unsent_one_is_forgotten() {
        let mut uploads = Uploads::default();
        uploads.sent("photo", "upload", &[Value::Number(1.0)], 4);
        // The same request for newer arguments (LLP 1054.000.000 D3).
        uploads.sent("photo", "upload", &[Value::Number(2.0)], 4);
        uploads.sent("photo", "upload", &[Value::Number(3.0)], 5);
        assert!(uploads.end(4, &UploadEnd::Answered(Value::str("ok"))));
        assert_eq!(
            uploads.at("photo", &[Value::Number(1.0)]),
            Some(&UploadEnd::Answered(Value::str("ok")))
        );
        assert_eq!(
            uploads.at("photo", &[Value::Number(2.0)]),
            Some(&UploadEnd::Answered(Value::str("ok")))
        );
        assert!(uploads.unmark(5));
        assert_eq!(uploads.at("photo", &[Value::Number(3.0)]), None);
    }

    #[test]
    fn the_kept_marks_are_the_newest_that_fit() {
        let big = Value::str(&"x".repeat(UPLOADS_BYTES / 3));
        let uploads = Uploads(
            (0..4)
                .map(|n| mark(f64::from(n), UploadEnd::Answered(big.clone())))
                .collect(),
        );
        let (bytes, forgot) = uploads.encoded();
        assert!(bytes.len() <= UPLOADS_BYTES);
        assert_eq!(forgot, 2, "the two oldest");
    }
}
