//! The uploads a runner has asked (LLP 1016.000 D10): for each resource,
//! the identifying arguments an upload was asked at and how it stands.
//! Nothing asks an upload again at arguments it has a mark for: the
//! resource shows the answer the upload ended with, or its failure. The
//! marks last as long as the runner, and a plan reload or a data module's
//! update carries them by resource, source and arguments; an upload that
//! one cut off comes back failed, `interrupted`, once its host says it had
//! begun to go out, and is asked again if it had not. A launch starts with
//! none: an app records what an upload answered in its store and stops
//! showing the resource, or the launch asks it again.

use super::{DataSource, Runner, Target};
use crate::failure::{Failure, FailureCode};
use exact_plan::Value;
use std::rc::Rc;

/// How many of a resource's uploads keep the answer each ended with, the
/// newest; an older one keeps only that it went out (`Gone`).
pub(super) const ANSWERS_KEPT: usize = 32;

/// How an upload asked at some arguments stands.
#[derive(Clone, Debug, PartialEq)]
pub enum UploadEnd {
    /// On its way, under this runner's ticket.
    Sent(u64),
    /// The answer it ended with.
    Answered(Value),
    /// Its failure: its source failed to take its end, or it was cut off.
    Failed(Failure),
    /// It went out and ended, and its answer is no longer kept: asked
    /// again, it answers that it was sent already, and is not sent again.
    Gone,
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

/// The failure of an upload asked again whose answer is no longer kept.
fn already_sent() -> Failure {
    Failure {
        code: FailureCode::Error,
        message: format!(
            "already sent: the upload went out at these arguments, and of a resource's uploads only the newest {ANSWERS_KEPT} keep their answers; give new arguments to send it again"
        ),
        reason: "already-sent".into(),
    }
}

/// The marks, oldest first, shared with the checkpoints that copy them
/// until one changes; and the uploads let go before the runner heard they
/// started, which wait for the host's word (`uploads_started`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Uploads {
    pub(super) marks: Rc<Vec<UploadMark>>,
    let_go: Vec<u64>,
}

impl Uploads {
    /// How the upload `resource` asked of `source` at `args` stands, if it
    /// was asked.
    pub(super) fn at(&self, resource: &str, source: &str, args: &[Value]) -> Option<&UploadEnd> {
        (self.marks.iter().rev())
            .find(|m| m.resource == resource && m.source == source && m.args == args)
            .map(|m| &m.end)
    }

    /// `resource` asked an upload at `args`, on its way under `ticket`.
    pub(super) fn sent(&mut self, resource: &str, source: &str, args: &[Value], ticket: u64) {
        let marks = Rc::make_mut(&mut self.marks);
        marks.retain(|m| !(m.resource == resource && m.source == source && m.args == args));
        marks.push(UploadMark {
            resource: resource.into(),
            source: source.into(),
            args: args.to_vec(),
            end: UploadEnd::Sent(ticket),
        });
    }

    /// Every mark on its way under `ticket` ends as `end`, and of each
    /// resource's ended uploads only the newest [`ANSWERS_KEPT`] keep how
    /// they ended.
    pub(super) fn end(&mut self, ticket: u64, end: &UploadEnd) {
        let marks = Rc::make_mut(&mut self.marks);
        let mut resources = Vec::new();
        for m in marks
            .iter_mut()
            .filter(|m| m.end == UploadEnd::Sent(ticket))
        {
            m.end = end.clone();
            resources.push(m.resource.clone());
        }
        for resource in resources {
            let ended = |m: &UploadMark| {
                m.resource == resource
                    && matches!(m.end, UploadEnd::Answered(_) | UploadEnd::Failed(_))
            };
            let over = marks
                .iter()
                .filter(|m| ended(m))
                .count()
                .saturating_sub(ANSWERS_KEPT);
            for m in marks.iter_mut().filter(|m| ended(m)).take(over) {
                m.end = UploadEnd::Gone;
            }
        }
    }

    /// Forget the marks under `ticket`: its upload never reached the
    /// network, so asking it again sends nothing twice.
    pub(super) fn unmark(&mut self, ticket: u64) {
        if self.marks.iter().any(|m| m.end == UploadEnd::Sent(ticket)) {
            Rc::make_mut(&mut self.marks).retain(|m| m.end != UploadEnd::Sent(ticket));
        }
    }
}

impl<D: DataSource> Runner<D> {
    /// Start with the marks the old runner carried: those of a resource the
    /// plan still has, with the same source. One on its way waits for the
    /// host's word on whether it had begun to go out (`uploads_started`),
    /// and this runner's tickets start past its, so the two never meet.
    pub(super) fn carry_uploads_in(&mut self, marks: &[UploadMark]) {
        let plan = &self.plan;
        let marks: Vec<UploadMark> = (marks.iter())
            .filter(|m| {
                (plan.resources.iter())
                    .any(|row| plan.str(row.name) == m.resource && plan.str(row.source) == m.source)
            })
            .cloned()
            .collect();
        let let_go: Vec<u64> = (marks.iter())
            .filter_map(|m| match m.end {
                UploadEnd::Sent(ticket) => Some(ticket),
                _ => None,
            })
            .collect();
        if let Some(last) = let_go.iter().max() {
            self.next_ticket = self.next_ticket.max(last + 1);
        }
        self.uploads = Uploads {
            marks: Rc::new(marks),
            let_go,
        };
    }

    /// The marks as a reload or an update carries them: an upload on its way
    /// that this runner heard begin is cut off, `interrupted`; one it did not
    /// stays on its way, for the next runner to ask its host about.
    pub(super) fn carried_uploads(&self) -> Vec<UploadMark> {
        (self.uploads.marks.iter())
            .map(|m| match m.end {
                UploadEnd::Sent(t) if self.pending.iter().any(|p| p.ticket == t && p.sent) => {
                    UploadMark {
                        end: UploadEnd::Failed(interrupted()),
                        ..m.clone()
                    }
                }
                _ => m.clone(),
            })
            .collect()
    }

    /// Resource `i`'s identifying arguments among `args`: its `with`
    /// context reaches the source but does not identify the upload.
    pub(super) fn upload_identity<'a>(&self, i: usize, args: &'a [Value]) -> &'a [Value] {
        let row = &self.plan.resources[i];
        let n = (row.args.len as usize).saturating_sub(usize::from(row.context));
        &args[..n.min(args.len())]
    }

    /// How resource `i`'s upload asked at `args` stands, if it was asked.
    pub(super) fn upload_at(&self, i: usize, args: &[Value]) -> Option<&UploadEnd> {
        let row = &self.plan.resources[i];
        let (name, source) = (self.plan.str(row.name), self.plan.str(row.source));
        self.uploads.at(name, source, self.upload_identity(i, args))
    }

    /// Whether an upload is in flight for resource `i` (LLP 1016.000 D10).
    pub(super) fn uploading(&self, i: usize) -> bool {
        (self.pending.iter()).any(|p| p.target == Target::Resource(i) && p.upload)
    }

    /// Whether resource `i`'s failure is an upload's, at the arguments it holds.
    pub(super) fn upload_failed(&self, i: usize) -> bool {
        (self.failed_args[i].as_ref())
            .is_some_and(|a| matches!(self.upload_at(i, a), Some(UploadEnd::Failed(_))))
    }

    /// Whether resource `i`'s answer is an upload's, in flight, ended or
    /// failed: nothing asks it again at its arguments (D10).
    pub(super) fn upload_stands(&self, i: usize) -> bool {
        self.uploading(i)
            || (self.resources[i].as_ref()).is_some_and(|s| self.upload_at(i, &s.args).is_some())
            || self.upload_failed(i)
    }

    /// `target` asked an upload at `args`, handed to its host as `ticket`.
    pub(super) fn upload_sent(&mut self, target: Target, args: &[Value], ticket: u64) {
        let Target::Resource(i) = target else { return };
        let row = &self.plan.resources[i];
        let (name, source) = (self.plan.str(row.name), self.plan.str(row.source));
        let identity = self.upload_identity(i, args).to_vec();
        self.uploads.sent(name, source, &identity, ticket);
    }

    /// Upload `ticket` is over: `end` when it reached the network, else
    /// forgotten, an ordinary answer that `refresh` asks again.
    pub(super) fn upload_over(&mut self, ticket: u64, sent: bool, end: UploadEnd) {
        match sent {
            true => self.uploads.end(ticket, &end),
            false => self.uploads.unmark(ticket),
        }
    }

    /// Upload `ticket` was let go before its reply, before the runner heard
    /// whether it had started: the host, whose executor knows, says so after
    /// the commit (`uploads_started`). Until then it stands as cut off.
    pub(super) fn upload_let_go(&mut self, ticket: u64) {
        self.uploads.let_go.push(ticket);
    }

    /// The host's word on the uploads let go since it last spoke, after it
    /// let their tickets go (`Core::let_go_started` natively), or on those an
    /// old runner had on their way, when it swapped in this one: each in
    /// `started` had begun to go out, and is cut off, `interrupted`. Any
    /// other never reached the network and is no upload asked: `refresh`
    /// asks it again, and a resource that shows it cut off is asked again at
    /// the next commit (LLP 1016.000 D10).
    pub fn uploads_started(&mut self, started: &[u64]) {
        for ticket in std::mem::take(&mut self.uploads.let_go) {
            if started.contains(&ticket) {
                self.uploads.end(ticket, &UploadEnd::Failed(interrupted()));
                continue;
            }
            let left: Vec<(String, Vec<Value>)> = (self.uploads.marks.iter())
                .filter(|m| m.end == UploadEnd::Sent(ticket))
                .map(|m| (m.resource.clone(), m.args.clone()))
                .collect();
            self.uploads.unmark(ticket);
            for i in 0..self.plan.resources.len() {
                let name = self.plan.str(self.plan.resources[i].name);
                let shows = (self.failed_args[i].as_ref()).is_some_and(|failed| {
                    let identity = self.upload_identity(i, failed);
                    left.iter()
                        .any(|(r, args)| r == name && args[..] == *identity)
                });
                if shows {
                    self.force_refresh(i);
                }
            }
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
            UploadEnd::Gone => failed(&already_sent()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(uploads: &Uploads) -> Vec<(f64, UploadEnd)> {
        (uploads.marks.iter())
            .map(|m| match m.args[..] {
                [Value::Number(n)] => (n, m.end.clone()),
                _ => unreachable!(),
            })
            .collect()
    }

    #[test]
    fn an_end_reaches_every_mark_of_its_ticket_and_an_unsent_one_is_forgotten() {
        let mut uploads = Uploads::default();
        uploads.sent("photo", "upload", &[Value::Number(1.0)], 4);
        // The same request for newer arguments (LLP 1054.000.000 D3).
        uploads.sent("photo", "upload", &[Value::Number(2.0)], 4);
        uploads.sent("photo", "upload", &[Value::Number(3.0)], 5);
        let ok = UploadEnd::Answered(Value::str("ok"));
        uploads.end(4, &ok);
        assert_eq!(
            uploads.at("photo", "upload", &[Value::Number(1.0)]),
            Some(&ok)
        );
        assert_eq!(
            uploads.at("photo", "upload", &[Value::Number(2.0)]),
            Some(&ok)
        );
        uploads.unmark(5);
        assert_eq!(uploads.at("photo", "upload", &[Value::Number(3.0)]), None);
    }

    #[test]
    fn of_a_resources_uploads_the_newest_keep_their_answers_and_the_rest_that_they_went_out() {
        let mut uploads = Uploads::default();
        let n = ANSWERS_KEPT as u64 + 3;
        for ticket in 1..=n {
            uploads.sent("photo", "upload", &[Value::Number(ticket as f64)], ticket);
            uploads.sent(
                "other",
                "upload",
                &[Value::Number(ticket as f64)],
                1000 + ticket,
            );
            uploads.end(ticket, &UploadEnd::Answered(Value::Number(ticket as f64)));
        }
        let photo: Vec<_> = (marks(&uploads).into_iter().zip(uploads.marks.iter()))
            .filter(|(_, m)| m.resource == "photo")
            .map(|(mark, _)| mark)
            .collect();
        assert_eq!(photo.len(), n as usize);
        for (k, (n, end)) in photo.into_iter().enumerate() {
            let kept = UploadEnd::Answered(Value::Number(n));
            assert_eq!(end, if k < 3 { UploadEnd::Gone } else { kept }, "{n}");
        }
        // Another resource's are its own, here still on their way.
        let other = uploads.marks.iter().filter(|m| m.resource == "other");
        assert!(
            other.clone().count() == n as usize
                && other
                    .into_iter()
                    .all(|m| matches!(m.end, UploadEnd::Sent(_)))
        );
    }

    #[test]
    fn a_checkpoint_shares_the_marks_until_they_change() {
        let mut uploads = Uploads::default();
        uploads.sent("photo", "upload", &[Value::Number(1.0)], 1);
        uploads.sent("photo", "upload", &[Value::Number(2.0)], 2);
        uploads.end(1, &UploadEnd::Answered(Value::str("done")));
        assert_eq!(
            uploads.at("photo", "elsewhere", &[Value::Number(1.0)]),
            None,
            "another source's"
        );
        let copy = uploads.clone();
        assert!(
            Rc::ptr_eq(&copy.marks, &uploads.marks),
            "a copy shares them"
        );
        uploads.sent("photo", "upload", &[Value::Number(3.0)], 3);
        assert!(!Rc::ptr_eq(&copy.marks, &uploads.marks) && copy.marks.len() == 2);
    }
}
