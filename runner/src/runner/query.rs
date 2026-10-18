//! Asking one resource of its source: the runner's own sources, then the
//! data seam (LLP 1030 D7, LLP 1039 D1, LLP 1038 D5).

use super::{lines, DataError, DataSource, Runner, RunnerError};
use crate::request::Answer;
use exact_plan::Value;

/// Asks of one resource made by answers' commits, each with arguments unlike
/// the last, within a second, that say an argument moves on every answer.
const CHURN_ASKS: u32 = 50;

/// What a development build counts to say once that a resource is asked
/// without end: an argument that reads a clock in a derive (`performanceNow()`)
/// changes with every commit, so each answer's commit asks again.
#[derive(Default)]
pub(super) struct Churn {
    /// An answer's commit is running: only its asks count, so a person or a
    /// drive typing into a search field is not churn, under a frozen clock too.
    pub(super) answering: bool,
    /// Per resource: the arguments last asked, when its second began, the
    /// asks in it with new arguments, and whether it was said.
    rows: std::collections::HashMap<usize, (Vec<Value>, f64, u32, bool)>,
}

impl<D: DataSource> Runner<D> {
    pub(super) fn query(&mut self, i: usize, args: &[Value]) -> Result<Answer, RunnerError> {
        let row = &self.plan.resources[i];
        let source = self.plan.str(row.source).to_string();
        let resource = self.plan.str(row.name).to_string();
        // Delivery is the runner's own (LLP 1030 D7): the data seam never
        // sees it, and a data crate could not answer it if it did.
        if source == crate::delivery::SOURCE {
            return self
                .delivery_answer(i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        // @ref LLP 1039 D1 — host facts never reach the app data source.
        if source == crate::viewport::SOURCE {
            return self
                .viewport_answer(i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        if source == crate::time::SOURCE {
            return self
                .time_answer(i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        if source == crate::page::SOURCE {
            return self
                .page_answer(i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        if source == crate::transport::SOURCE {
            return self
                .transport_answer(i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        if source == crate::midi::SOURCE {
            return self
                .midi_answer(i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        if source == crate::surface_record::SOURCE {
            let answer = self.links.surface_answer.ok_or_else(|| RunnerError::Data {
                resource: resource.clone(),
                error: DataError::Unavailable("this host links no surfaces".into()),
            })?;
            return answer(&self.plan, &self.surface_records, i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        // @ref LLP 1038 D5 / §8 — distinguish asked sources from compiled boot values.
        self.log(lines::query(&resource, &source));
        if self.live.admitted {
            self.count_churn(i, args, &resource);
        }
        self.snapshot_sockets();
        let answer =
            self.data
                .answer_for(super::Target::Resource(i), &mut self.store, &source, args);
        // What its turn sent on its module's sockets (LLP 1016.000 D7, the
        // amendment of 2026-10-18), whatever it answered.
        self.stage_answer(i);
        for line in self.data.take_logs() {
            self.log(line);
        }
        answer.map_err(|error| RunnerError::Data { resource, error })
    }

    /// Say once, in a development build, that resource `i` is asked again
    /// and again, its arguments new each time: [`CHURN_ASKS`] in a second.
    fn count_churn(&mut self, i: usize, args: &[Value], resource: &str) {
        let (now, answering) = (self.now_ms, self.churn.answering);
        let row = self
            .churn
            .rows
            .entry(i)
            .or_insert_with(|| (args.to_vec(), now, 0, false));
        let Some(k) = row
            .0
            .iter()
            .zip(args)
            .position(|(a, b)| a != b)
            .or((row.0.len() != args.len()).then_some(row.0.len().min(args.len())))
        else {
            return;
        };
        row.0 = args.to_vec();
        if !answering {
            return;
        }
        if now - row.1 > 1000.0 {
            row.1 = now;
            row.2 = 0;
        }
        row.2 += 1;
        if row.2 < CHURN_ASKS || std::mem::replace(&mut row.3, true) {
            return;
        }
        self.log(format!(
            "`{resource}` was asked {CHURN_ASKS} times in a second, its arguments new each time (argument {} last): an argument that moves on every answer, such as a clock read in a derive (`performanceNow()`), asks again without end. Give it a value that changes only when the answer should, such as a timer's tick",
            k + 1
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::CHURN_ASKS;
    use crate::{Answer, Store};
    use exact_kernel::{Kernel, NodeType};
    use exact_plan::{builder::PlanBuilder, TypeKind, Value};

    struct Later;
    impl DataSource for Later {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::Unavailable(source.into()))
        }
        fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
            Ok(Answer::Now(Value::str("answer")))
        }
    }

    /// `r = src(<a number>)`, asked by hand with the arguments a moving
    /// clock would give it.
    fn runner() -> Runner<Later> {
        let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        let string = b.primitive(TypeKind::String);
        let arg = b.constant(&Value::Number(0.));
        let r = b.resource("r", "src", &[arg], string, Some(&Value::str("loading")));
        b.set_resource_initial_args(r, &[Value::Number(0.)]);
        b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
        let plan = b.finish().unwrap();
        Runner::boot_stored(
            plan,
            Later,
            Kernel::with_monospace(),
            Vec::new(),
            Default::default(),
            "/",
        )
        .unwrap()
    }

    fn said(r: &Runner<Later>) -> Vec<String> {
        r.journal()
            .filter(|l| l.contains("times in a second"))
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn a_resource_asked_again_and_again_with_new_arguments_is_said_once_in_development() {
        let mut r = runner();
        r.admit_live(None);
        r.churn.answering = true;
        for n in 0..(2 * CHURN_ASKS) {
            r.query(0, &[Value::Number(f64::from(n) + 1.)]).unwrap();
        }
        let lines = said(&r);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].contains(
                "`r` was asked 50 times in a second, its arguments new each time (argument 1 last)"
            ),
            "{}",
            lines[0]
        );
        // Asks an event's commit makes (typing into a search field, the
        // clock frozen) are not churn; nor the same arguments again, or new
        // ones a second apart.
        let mut r = runner();
        r.admit_live(None);
        for n in 0..(2 * CHURN_ASKS) {
            r.query(0, &[Value::Number(f64::from(n) + 1.)]).unwrap();
        }
        assert!(said(&r).is_empty());
        r.churn.answering = true;
        for n in 0..(2 * CHURN_ASKS) {
            r.query(0, &[Value::Number(1.)]).unwrap();
            r.now_ms = 1001. * f64::from(n);
            r.query(0, &[Value::Number(f64::from(n) + 2.)]).unwrap();
        }
        assert!(said(&r).is_empty());
        // A production build says nothing.
        let mut r = runner();
        r.churn.answering = true;
        for n in 0..(2 * CHURN_ASKS) {
            r.query(0, &[Value::Number(f64::from(n) + 1.)]).unwrap();
        }
        assert!(said(&r).is_empty());
    }
}
