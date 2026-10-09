//! What the launch composer asks the caller to run: `fleet models` for each
//! harness's catalog while the model menu is open (workspace.rs
//! `schedule_launch_catalog`), and `fleet directories` for the path typed
//! into the project menu (`schedule_directory`). `wanted_*` say what to run, `*_requested`
//! mark it taken, `set_*` bring the answer back.

use serde::{Deserialize, Serialize};

use super::{account, default_account, machine, Launch, MenuKind, Outcome, PROVIDERS};
use crate::quick::{self, Step};
use crate::types::State;

/// One model `fleet models` lists.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModelOption {
    /// The identifier the harness takes.
    pub id: String,
    /// Its display name.
    pub name: String,
    /// A line under the name.
    pub description: String,
    /// The harness's own default.
    pub default: bool,
}

/// What `fleet directories` answers.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct DirectoryMatches {
    /// Matching folders, each ending in `/`.
    pub paths: Vec<String>,
    /// More matched than were listed.
    pub truncated: bool,
}

/// One `fleet models` run: machine, harness, account handle and folder.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
pub struct CatalogKey {
    /// The machine id.
    pub machine: String,
    /// The harness.
    pub provider: String,
    /// The account handle (empty for the machine login).
    pub account: String,
    /// The folder.
    pub cwd: String,
}

impl CatalogKey {
    /// `fleet models MACHINE --provider P --account NAME --cwd C`; the CLI
    /// takes the account's name, not its handle.
    pub fn argv(&self, state: &State) -> Vec<String> {
        let name = account(state, &self.account)
            .map(|a| a.name.clone())
            .unwrap_or_default();
        vec![
            "models".into(),
            self.machine.clone(),
            "--provider".into(),
            self.provider.clone(),
            "--account".into(),
            name,
            "--cwd".into(),
            self.cwd.clone(),
        ]
    }
}

/// Folder completion results for one typed path, on one machine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirectoryState {
    /// The matches, each ending in `/`.
    pub paths: Vec<String>,
    /// More matched than were listed.
    pub truncated: bool,
    /// The highlighted match.
    pub index: usize,
    /// An answer is outstanding.
    pub loading: bool,
    /// The caller took the query (`wanted_directories`) already.
    pub requested: bool,
    /// Why nothing could be listed.
    pub error: String,
}

/// The provider's model catalog for the chosen machine and account.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelState {
    /// The catalog.
    pub options: Vec<ModelOption>,
    /// An answer is outstanding.
    pub loading: bool,
    /// The caller took the query (`wanted_catalog`) already.
    pub requested: bool,
    /// The catalog was asked for at least once.
    pub active: bool,
    /// Why it could not load.
    pub error: String,
    /// The machine's name, for "loading from …".
    pub machine: String,
    /// What it was loaded for.
    pub query: Option<CatalogKey>,
}

impl Launch {
    /// Load every harness's model catalog while the model menu is open (only
    /// the current harness under quick launch), so the whole list fills in
    /// at once. Loaded results stay visible while `force` refreshes them.
    pub fn sync_catalogs(&mut self, state: &State, force: bool) {
        let quick = self.quick.as_ref().is_some_and(|q| q.step == Step::Model);
        let menu = self
            .menu
            .as_ref()
            .is_some_and(|m| m.kind == MenuKind::Model);
        if !menu && !quick {
            return;
        }
        let catalog_key = format!("{}\n{}\n{}", self.machine, self.account, self.cwd);
        if self.catalog_key != catalog_key {
            self.catalogs.clear();
            self.catalog_key = catalog_key;
        }
        let machine_name = machine(state, &self.machine).map(|m| m.name.clone());
        for provider in PROVIDERS {
            if quick && provider != self.provider {
                continue;
            }
            let handle = if provider == self.provider {
                self.account.clone()
            } else {
                default_account(state, provider)
            };
            let account_known = handle.is_empty() || account(state, &handle).is_some();
            let key = CatalogKey {
                machine: self.machine.clone(),
                provider: provider.into(),
                account: handle,
                cwd: self.cwd.clone(),
            };
            let catalog = self.catalogs.entry(provider.to_string()).or_default();
            if catalog.active && catalog.query.as_ref() == Some(&key) && !force {
                continue;
            }
            let options = if catalog.query.as_ref() == Some(&key) {
                std::mem::take(&mut catalog.options)
            } else {
                Vec::new()
            };
            *catalog = ModelState {
                active: true,
                query: Some(key.clone()),
                options,
                ..Default::default()
            };
            let Some(name) = machine_name.clone() else {
                catalog.error = format!("machine {} not found", self.machine);
                continue;
            };
            catalog.machine = name;
            if !account_known {
                catalog.error = format!("account {} not found", key.account);
                continue;
            }
            catalog.loading = true;
        }
    }

    /// The next `fleet models` to run, if a catalog is waiting for one;
    /// `catalog_requested` marks it taken.
    pub fn wanted_catalog(&self) -> Option<CatalogKey> {
        PROVIDERS
            .iter()
            .filter_map(|p| self.catalogs.get(*p))
            .find(|c| c.loading && !c.requested)
            .and_then(|c| c.query.clone())
    }

    /// The caller started `fleet models` for this key.
    pub fn catalog_requested(&mut self, key: &CatalogKey) {
        if let Some(c) = self
            .catalogs
            .values_mut()
            .find(|c| c.query.as_ref() == Some(key))
        {
            c.requested = true;
        }
    }

    /// `fleet models` answered; a stale answer (another key) is dropped. A
    /// quick launch pick that waited for the catalog may now submit.
    pub fn set_models(
        &mut self,
        key: &CatalogKey,
        result: Result<Vec<ModelOption>, String>,
        state: &State,
    ) -> Outcome {
        let Some(c) = self
            .catalogs
            .values_mut()
            .find(|c| c.query.as_ref() == Some(key))
        else {
            return Outcome::None;
        };
        c.loading = false;
        c.requested = false;
        match result {
            Ok(options) => {
                c.options = options;
                c.error.clear();
            }
            Err(error) => c.error = error,
        }
        quick::finish_pending(self, state)
    }

    /// The typed path is asked for again (workspace.rs `schedule_directory`),
    /// when the project menu's query reads like one.
    pub(super) fn refresh_directories(&mut self) {
        if let Some(menu) = self.path_menu_mut() {
            menu.directory = DirectoryState {
                loading: true,
                ..Default::default()
            };
        }
    }

    fn path_menu_mut(&mut self) -> Option<&mut super::Menu> {
        self.menu
            .as_mut()
            .filter(|m| m.kind == MenuKind::Project && m.is_path())
    }

    /// The `fleet directories MACHINE PATH` to run, if a path is waiting for
    /// its completions; `directories_requested` marks it taken.
    pub fn wanted_directories(&self) -> Option<(String, String)> {
        let menu = self
            .menu
            .as_ref()
            .filter(|m| m.kind == MenuKind::Project && m.is_path())?;
        if menu.directory.loading && !menu.directory.requested {
            Some((self.machine.clone(), menu.query.trim().to_string()))
        } else {
            None
        }
    }

    /// The caller started `fleet directories` for the path.
    pub fn directories_requested(&mut self) {
        if let Some(menu) = self.path_menu_mut() {
            menu.directory.requested = true;
        }
    }

    /// `fleet directories` answered for `machine` and `path`; an answer to
    /// an earlier path or machine is dropped.
    pub fn set_directories(
        &mut self,
        machine: &str,
        path: &str,
        result: Result<DirectoryMatches, String>,
    ) {
        if self.machine != machine {
            return;
        }
        let Some(menu) = self.path_menu_mut() else {
            return;
        };
        if menu.query.trim() != path {
            return;
        }
        let d = &mut menu.directory;
        d.loading = false;
        d.requested = false;
        match result {
            Ok(m) => {
                d.paths = m.paths;
                d.truncated = m.truncated;
            }
            Err(e) => d.error = e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::tests::{fresh, state};
    use crate::launch::MenuKind;

    #[test]
    fn catalogs_load_while_the_model_menu_is_open_and_answers_land_by_key() {
        let st = state();
        let mut launch = fresh(&st);
        launch.provider = "claude".into();
        launch.account = "claude:me@example.com".into();
        assert_eq!(launch.wanted_catalog(), None);
        launch.open_menu(MenuKind::Model, &st);
        let key = launch.wanted_catalog().unwrap();
        assert_eq!(key.provider, "codex");
        assert_eq!(
            key.argv(&st)[0..4],
            ["models", "laptop", "--provider", "codex"]
        );
        launch.catalog_requested(&key);
        let next = launch.wanted_catalog().unwrap();
        assert_eq!(next.provider, "claude");
        assert_eq!(next.account, "claude:me@example.com");
        assert_eq!(next.argv(&st)[5], "me");
        assert_eq!(launch.catalogs["codex"].machine, "LAPTOP");
        let opus = ModelOption {
            id: "opus".into(),
            name: "Opus".into(),
            ..Default::default()
        };
        assert_eq!(launch.set_models(&next, Ok(vec![opus]), &st), Outcome::None);
        launch.set_models(&key, Err("offline".into()), &st);
        assert_eq!(launch.catalogs["codex"].error, "offline");
        assert!(launch.model_rows().iter().any(|r| r.id == "opus"));
        // A stale answer for another folder is ignored; a forced refresh
        // keeps the loaded catalog visible while asking again.
        let stale = CatalogKey {
            cwd: "/elsewhere".into(),
            ..key.clone()
        };
        launch.set_models(&stale, Ok(vec![]), &st);
        assert_eq!(launch.catalogs["codex"].error, "offline");
        launch.sync_catalogs(&st, true);
        assert!(launch.catalogs["claude"].loading);
        assert_eq!(launch.catalogs["claude"].options.len(), 1);
        // An unknown account never asks; the catalog says so.
        launch.account = "nobody".into();
        launch.sync_catalogs(&st, false);
        assert_eq!(launch.catalogs["claude"].error, "account nobody not found");
        assert!(launch
            .wanted_catalog()
            .is_some_and(|k| k.provider == "codex"));
        // Every harness is asked for, the new ones included.
        let providers: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| launch.catalogs.contains_key(**p))
            .copied()
            .collect();
        assert_eq!(providers, PROVIDERS);
    }

    #[test]
    fn folder_completion_follows_the_typed_path() {
        let st = state();
        let mut launch = fresh(&st);
        launch.open_menu(MenuKind::Project, &st);
        launch.input("launch-menu-query", "fleet");
        assert_eq!(launch.wanted_directories(), None);
        launch.input("launch-menu-query", "~/");
        assert_eq!(
            launch.wanted_directories(),
            Some(("laptop".into(), "~/".into()))
        );
        launch.directories_requested();
        assert_eq!(launch.wanted_directories(), None);
        launch.input("launch-menu-query", " ~/s ");
        assert_eq!(
            launch.wanted_directories(),
            Some(("laptop".into(), "~/s".into()))
        );
        // The answer to the earlier path is dropped; the current one lands.
        launch.set_directories("laptop", "~/", Ok(DirectoryMatches::default()));
        assert!(launch.wanted_directories().is_some());
        launch.set_directories("studio", "~/s", Ok(DirectoryMatches::default()));
        assert!(launch.wanted_directories().is_some());
        let matches = DirectoryMatches {
            paths: vec!["~/srv/".into()],
            truncated: true,
        };
        launch.set_directories("laptop", "~/s", Ok(matches));
        let d = &launch.menu.as_ref().unwrap().directory;
        assert_eq!(d.paths, ["~/srv/"]);
        assert!(d.truncated);
        assert!(!d.loading);
        launch.set_directories("laptop", "~/s", Err("gone".into()));
        assert_eq!(launch.menu.as_ref().unwrap().directory.error, "gone");
    }
}
