//! Optional Linux delivery composition (LLP 1030 D4).
//! A binary-only app calls `exact_linux::run`; an update-capable app calls this crate.
#![deny(missing_docs)]
mod store;
use exact_linux::delivery::{Selection, Store};
use exact_runner::{DataSource, Delivery};
use std::path::PathBuf;
use std::sync::Arc;
pub use store::Updates;

/// Run the app with its update store attached before boot.
pub fn run<D: DataSource + Default>(baked: &[u8], compat: &str) -> i32 {
    if Delivery::default().with_compat(compat).store == '0' {
        eprintln!(
            "exact: this binary-only app must use the core host entry; regenerate the app entry"
        );
        return 1;
    }
    let started = std::time::Instant::now();
    let mut config = exact_linux::app::Config::from_env(baked, compat);
    match Updates::open(compat, baked, &config.assets) {
        Ok(updates) => config.use_updates(Box::new(updates), baked),
        Err(error) => eprintln!("exact update: {error}"),
    }
    exact_linux::app::run_config::<D>(&mut config, started)
}

impl Store for Updates {
    fn prepare_selected(&mut self) -> Option<Selection> {
        let prepared = Updates::prepare_selected(self)?;
        self.pin(prepared.generation.clone());
        Some(Selection {
            entry: prepared.generation.entry,
            plan: prepared.plan,
            assets: Arc::new(move |name| prepared.assets.resolve(name)),
        })
    }
    fn boot_started(&mut self) {
        Updates::boot_started(self);
    }
    fn entry_refused(&mut self, entry: &str, why: &str) {
        Updates::entry_refused(self, entry, why);
    }
    fn selection_corrupt(&mut self, why: &str) {
        self.refuse_pinned(why);
    }
    fn boot_succeeded(&mut self) {
        Updates::boot_succeeded(self);
    }
    fn take_note(&mut self) -> Option<String> {
        Updates::take_note(self)
    }
    fn fd(&self) -> std::os::unix::io::RawFd {
        Updates::fd(self)
    }
    fn check(&self) -> bool {
        Updates::check(self)
    }
    fn take_line(&mut self) -> Option<String> {
        Updates::take_line(self)
    }
    fn activate(&self) -> Option<(Vec<u8>, PathBuf)> {
        Updates::activate(self)
    }
    fn status_into(&self, delivery: &mut Delivery) {
        Updates::status_into(self, delivery);
    }
}

#[cfg(test)]
mod app_tests;
