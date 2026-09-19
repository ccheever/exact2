//! Exact Live's one app-owned composition; existing sources keep their owners.
#![forbid(unsafe_code)]

mod crew;
mod jobs;
mod runbook;

use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Store};
use interaction_gallery_data::Gallery;
use jobs::Jobs;
use messages_stress_data::ReusableMessagesStress;

/// Independent device-local workspace. No network work occurs in construction.
pub struct Live {
    gallery: Gallery,
    crew: crew::Crew,
    history: ReusableMessagesStress,
    jobs: Jobs,
    runbook: runbook::Runbook,
}

fn curated_gallery() -> Gallery {
    let mut gallery = Gallery::default();
    trim_to_scene(&mut gallery).expect("known initial fixture identities");
    gallery
}

fn trim_to_scene(gallery: &mut Gallery) -> Result<(), DataError> {
    // The existing fixture starts with 100 identities. Retire the unused 94
    // through its public operations before any row cache is requested.
    // This is bounded app startup work, not asynchronous or constant-cost proof.
    for id in 6..100 {
        gallery.query(
            "galleryAction",
            &[
                Value::str("delete"),
                Value::str(&format!("photo-{id:05}")),
                Value::Number(0.),
            ],
        )?;
    }
    // Finish scene initialization in its real photo interaction mode. The
    // existing operation replaces deletion bookkeeping with useful guidance.
    gallery.query(
        "galleryAction",
        &[Value::str("mode"), Value::str("photos"), Value::Number(0.)],
    )?;
    Ok(())
}

impl Default for Live {
    fn default() -> Self {
        Self {
            gallery: curated_gallery(),
            crew: crew::Crew::default(),
            history: ReusableMessagesStress::default(),
            jobs: Jobs::default(),
            runbook: runbook::Runbook::default(),
        }
    }
}

fn job_source(name: &str) -> bool {
    matches!(
        name,
        "openWave" | "releaseWave" | "fixtureStats" | "completion" | "counters"
    )
}

impl DataSource for Live {
    fn app_id(&self) -> &str {
        "com.exact.live"
    }

    fn grants(&self) -> &str {
        self.jobs.grants()
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "history" if args.first() == Some(&Value::Number(0.)) => self.crew.query(args),
            "history" => self.history.query(source, args),
            "runbook" => self.runbook.query(args),
            "galleryAction" if args.first().and_then(Value::as_str) == Some("reset") => {
                if args != [Value::str("reset"), Value::str(""), Value::Number(0.)] {
                    return Err(DataError::BadArguments(
                        "reset uses empty identity and zero token".into(),
                    ));
                }
                let current = self.gallery.query("gallery", &[])?;
                let Value::Record(fields) = current else {
                    unreachable!("Gallery metadata")
                };
                if fields[7].as_number().unwrap() > f64::from(u32::MAX - 95) {
                    return Err(DataError::Unavailable(
                        "scene revision exhausted; reopen the app".into(),
                    ));
                }
                // Reuse this owner so old viewer/move tokens stay retired.
                self.gallery.query(
                    "galleryAction",
                    &[Value::str("load"), Value::str(""), Value::Number(100.)],
                )?;
                trim_to_scene(&mut self.gallery)?;
                self.gallery.query("gallery", &[])
            }
            "gallery" | "galleryRows" | "galleryAction" | "galleryReorder" | "theme" => {
                self.gallery.query(source, args)
            }
            name if job_source(name) => self.jobs.query(source, args),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if job_source(source) {
            self.jobs.answer(store, source, args)
        } else if source == "runbook" {
            self.runbook.answer(store, args)
        } else {
            self.query(source, args).map(Answer::Now)
        }
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if job_source(source) {
            self.jobs.parse(store, source, args, outcome)
        } else if source == "runbook" {
            self.runbook.parse(store, args, outcome)
        } else {
            Err(DataError::UnknownSource(source.into()))
        }
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        self.runbook.continuation(token)
    }
}
