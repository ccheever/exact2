//! The live table on the web (LLP 1121 D13, stage 4): a batch or snapshot
//! from the dev stream applied in place, no plan, no reload.

use super::super::{Batch, Host};
use exact_runner::{design::Edit, DataSource, LiveRefusal, Timed};

impl<D: DataSource> Host<D> {
    /// Apply a `tamber-batch` or `tamber-snapshot` frame: a batch whose
    /// `live` says `applied` (with `unsupported` when the wide colour table
    /// refused a colour), `stale` (ask for a snapshot), `structure` (a plan
    /// is coming) or `refused`.
    pub fn apply_live(&mut self, frame: &str) -> String {
        let Some(edit) = Edit::parse(frame) else {
            return self.finish(
                crate::batch::Batch::new(),
                Some("live: not a tamber batch or snapshot"),
            );
        };
        let mut batch = crate::batch::Batch::new();
        let (receipts, live, unsupported) = match self.runner.apply_live(&edit) {
            Ok(applied) => (
                {
                    // @ref LLP 1095 D11 — the role sheet again, in this
                    // batch's one style update (LLP 1121 stage 5); the
                    // receipt sends every node's rows again.
                    if applied.roles {
                        batch.theme(&self.runner.theme_css());
                    }
                    vec![Timed {
                        at_ms: self.now_ms,
                        receipt: applied.receipt,
                    }]
                },
                "applied".to_string(),
                applied.full,
            ),
            Err(LiveRefusal::Stale) => (vec![], "stale".to_string(), false),
            Err(LiveRefusal::Structure) => (vec![], "structure".to_string(), false),
            Err(LiveRefusal::Refused(why)) => (vec![], format!("refused: {why}"), false),
        };
        let batch = self.batch_from(batch, &receipts, None);
        let mut out = String::from("{\"live\":");
        exact_runner::agent::quote(&live, &mut out);
        out.push_str(&format!(",\"unsupported\":{unsupported},\"design\":"));
        exact_runner::agent::quote(&self.runner.design(), &mut out);
        out.push(',');
        out.push_str(&batch[1..]);
        out
    }
}

impl<D: DataSource> Host<D> {
    /// Close a batch: the runner's deadline, frames wanted (LLP 1073 D5), clock, `seq` (LLP 1079).
    pub(crate) fn finish(&self, batch: Batch, error: Option<&str>) -> String {
        let due = self.runner.timer_due_ms();
        let batch = batch.seq(self.runner.seq_range());
        let batch = batch.alerts(self.runner.take_alerts());
        let batch = batch.landing(self.runner.data_timer_landing());
        let mut out = batch.finish(due, self.runner.wants_frames(), self.runner.now_ms(), error);
        if self.runner.timer_tolerance_ms() > 0.0 {
            out.pop();
            out.push_str(&format!(
                ",\"timer_tolerance_ms\":{}}}",
                self.runner.timer_tolerance_ms()
            ));
        }
        if self.runner.plan().timers.iter().any(|t| t.wait != 0) {
            let waits = self.runner.waits();
            let states: Vec<f64> = [waits.frame, waits.update, waits.scrollend, waits.idle]
                .into_iter()
                .map(|w| {
                    if w.held {
                        -2.0
                    } else {
                        w.due_ms.unwrap_or(-1.0)
                    }
                })
                .collect();
            out.pop();
            out.push_str(&format!(
                ",\"waits\":{}}}",
                serde_json::to_string(&states).unwrap()
            ));
        }
        out
    }
}
