//! The live table on the web (LLP 1121 D13, stage 4): a batch or snapshot
//! from the dev stream applied in place, no plan, no reload.

use super::super::Host;
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
        // Its own roles across the receipt's walk, which an edit sets in
        // place (LLP 1121 stage 5).
        let _roles = self.runner.bind_roles();
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
