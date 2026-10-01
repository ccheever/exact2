//! The one menu that opens over the composer (launch.rs `Menu`, `Row`,
//! `rows` and the workspace.rs `launch_rows` / `launch_accounts` around
//! them): projects on the machine being looked at, models grouped by
//! harness, accounts by email, and "everything else" (effort and
//! guardrails). Only one is ever open, and the prompt keeps its text while
//! a menu filters its own list.

use super::{
    account_handle, catalog::DirectoryState, is_eas, machine_permission_default, provider_label,
    Launch, ModelOption, PROVIDERS,
};
use crate::picker::fuzzy_score;
use crate::types::State;

/// Which menu is open over the composer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuKind {
    /// Where it runs (⌘L).
    Project,
    /// What runs it (⌘M).
    Model,
    /// Who runs it (⌘U).
    Account,
    /// Effort and guardrails (⌘E): the settings the launch history says are
    /// touched a handful of times a year, so they are not on the line.
    More,
}

/// The open menu: its search text, highlight, and folder completions.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    /// Which one.
    pub kind: MenuKind,
    /// The search box's text.
    pub query: String,
    /// The highlighted row.
    pub index: usize,
    /// Folder completions, when the query is being typed as a path.
    pub directory: DirectoryState,
}

impl Menu {
    /// A menu with an empty search.
    pub fn new(kind: MenuKind) -> Menu {
        Menu {
            kind,
            query: String::new(),
            index: 0,
            directory: DirectoryState::default(),
        }
    }

    /// Only the lists long enough to need one get a search box.
    pub fn searchable(&self) -> bool {
        matches!(
            self.kind,
            MenuKind::Project | MenuKind::Model | MenuKind::Account
        )
    }

    /// A query that names a path asks the machine for folders instead of
    /// filtering the projects it already knows.
    pub fn is_path(&self) -> bool {
        let q = self.query.trim_start();
        q.starts_with('~') || q.starts_with('/') || q.starts_with("./")
    }

    /// The search box's placeholder.
    pub fn placeholder(&self) -> &'static str {
        match self.kind {
            MenuKind::Project => "Search projects, or type a path",
            MenuKind::Model => "Search models",
            MenuKind::Account => "Search accounts",
            MenuKind::More => "",
        }
    }
}

/// One line of an open menu. Groups and notes are there to read, not to pick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// A heading.
    Group(String),
    /// A line of explanation.
    Note(String),
    /// Start in the machine's home folder — what "no project" means.
    NoProject,
    /// A known folder, by its index in `projects`.
    Project(usize),
    /// A typed or completed path.
    Folder(String),
    /// A harness and model.
    Model(ModelRow),
    /// An account.
    Account(AccountChoice),
    /// An effort level.
    Effort(String),
    /// A permission mode.
    Perm(String),
}

impl Row {
    /// Whether the highlight can land on it.
    pub fn selectable(&self) -> bool {
        !matches!(self, Row::Group(_) | Row::Note(_))
    }
}

/// An account as the menu shows it: the email is the name worth reading, the
/// profile directory is what Fleet actually keys off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountChoice {
    /// The handle `fleet run --account` takes.
    pub handle: String,
    /// The email, or the name when there is none.
    pub email: String,
    /// The account's name.
    pub profile: String,
}

/// One model the menu offers: a harness with a model ID (empty for the
/// native default) or the typed custom ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRow {
    /// The harness.
    pub provider: String,
    /// The model id, empty for the harness default.
    pub id: String,
    /// The display name.
    pub name: String,
    /// The line beside it.
    pub description: String,
    /// One of the models this account launched recently.
    pub recent: bool,
}

/// How many project rows show before the list asks to be searched instead.
/// Real inventories are long: 46 repos on one Mac, 144 on another machine,
/// and OpenCode proxies a 373-model catalog.
pub const PROJECTS_SHOWN: usize = 10;
/// How many project rows a search shows.
pub const PROJECTS_SEARCHED: usize = 20;
/// How many models each harness shows unsearched.
pub const MODELS_PER_HARNESS: usize = 12;
/// How many models a search shows.
pub const MODELS_SEARCHED: usize = 25;

impl Launch {
    /// Every model the menu knows of: recent ones first, then every harness
    /// in order, its default first, then the loaded catalog. A custom ID that
    /// was typed stays listed under its harness so it can be picked again.
    pub fn model_rows(&self) -> Vec<ModelRow> {
        let mut rows: Vec<ModelRow> = self
            .recent_models
            .iter()
            .filter(|m| PROVIDERS.contains(&m.provider.as_str()))
            .map(|m| ModelRow {
                provider: m.provider.clone(),
                id: m.id.clone(),
                name: if m.name.is_empty() {
                    m.id.clone()
                } else {
                    m.name.clone()
                },
                description: provider_label(&m.provider).to_string(),
                recent: true,
            })
            .collect();
        for provider in PROVIDERS {
            let mut options = vec![ModelOption {
                id: String::new(),
                name: "Default".into(),
                description: format!("Whatever {} is set up with", provider_label(provider)),
                default: false,
            }];
            if let Some(catalog) = self.catalogs.get(provider) {
                // A harness that lists its own default (Claude's "Default
                // (recommended)") replaces the synthetic one.
                if catalog.options.iter().any(|o| o.id == "default") {
                    options.clear();
                }
                options.extend(catalog.options.iter().cloned());
            }
            let custom = provider == self.provider
                && !self.model.is_empty()
                && !options.iter().any(|o| o.id == self.model);
            if custom {
                options.push(ModelOption {
                    id: self.model.clone(),
                    name: self.model.clone(),
                    description: "Custom model identifier".into(),
                    default: false,
                });
            }
            rows.extend(options.into_iter().map(|o| ModelRow {
                provider: provider.to_string(),
                id: o.id,
                name: o.name,
                description: o.description,
                recent: false,
            }));
        }
        rows
    }

    /// Rows of the open menu. Nothing here reaches into app state: the
    /// accounts and the machine's default mode are handed in.
    pub fn rows(&self, accounts: &[AccountChoice], machine_default: &str) -> Vec<Row> {
        let Some(menu) = self.menu.as_ref() else {
            return Vec::new();
        };
        let query = menu.query.trim().to_string();
        match menu.kind {
            MenuKind::Project => self.project_rows(menu, &query),
            MenuKind::Model => self.model_menu_rows(&query),
            MenuKind::Account => self.account_rows(accounts, &query),
            MenuKind::More => self.more_rows(machine_default),
        }
    }

    /// The open menu's rows for this fleet (workspace.rs `launch_rows`): its
    /// accounts and machine default, and only Codex on an EAS machine.
    pub fn menu_rows(&self, state: &State) -> Vec<Row> {
        let eas = is_eas(state, &self.machine);
        let machine_default = machine_permission_default(state, &self.machine, &self.provider);
        self.rows(&self.accounts(state), &machine_default)
            .into_iter()
            .filter(|row| !eas || !matches!(row, Row::Model(m) if m.provider != "codex"))
            .collect()
    }

    /// Shared accounts for the harness being launched, named by their email
    /// (workspace.rs `launch_accounts`).
    pub fn accounts(&self, state: &State) -> Vec<AccountChoice> {
        state
            .accounts
            .iter()
            .filter(|a| a.shared && a.provider == self.provider)
            .map(|a| AccountChoice {
                handle: account_handle(a),
                email: if a.email.is_empty() {
                    a.name.clone()
                } else {
                    a.email.clone()
                },
                profile: a.name.clone(),
            })
            .collect()
    }

    /// Whether a row names what the launch is already set to, for the tick.
    pub fn row_is_current(&self, row: &Row) -> bool {
        match row {
            Row::NoProject => self.cwd == "~",
            Row::Project(i) => self
                .projects
                .get(*i)
                .is_some_and(|p| p.machine == self.machine && p.cwd == self.cwd),
            Row::Model(m) => m.provider == self.provider && m.id == self.model,
            Row::Account(a) => a.handle == self.account,
            Row::Effort(level) => *level == self.effort,
            Row::Perm(mode) => *mode == self.permissions,
            Row::Folder(_) | Row::Group(_) | Row::Note(_) => false,
        }
    }

    /// Projects on the machine being looked at — the machine is the bar across
    /// the top of the menu, not one of the rows inside it. A query that starts
    /// like a path lists folders from the machine instead.
    fn project_rows(&self, menu: &Menu, query: &str) -> Vec<Row> {
        if menu.is_path() {
            let mut rows: Vec<Row> = menu
                .directory
                .paths
                .iter()
                .map(|p| Row::Folder(p.clone()))
                .collect();
            if !query.is_empty() && !menu.directory.paths.iter().any(|p| p == query) {
                rows.push(Row::Folder(query.to_string()));
            }
            if menu.directory.loading && rows.is_empty() {
                rows.push(Row::Note("Looking…".into()));
            }
            if !menu.directory.error.is_empty() {
                rows.push(Row::Note(menu.directory.error.clone()));
            }
            return rows;
        }
        let mut scored: Vec<(usize, i32)> = self
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| p.machine == self.machine)
            .filter_map(|(i, p)| {
                if query.is_empty() {
                    Some((i, 0))
                } else {
                    fuzzy_score(query, &format!("{} {}", p.cwd, p.detail())).map(|s| (i, s))
                }
            })
            .collect();
        if !query.is_empty() {
            scored.sort_by_key(|(_, s)| *s);
        }
        let cap = if query.is_empty() {
            PROJECTS_SHOWN
        } else {
            PROJECTS_SEARCHED
        };
        let hidden = scored.len().saturating_sub(cap);
        let mut rows: Vec<Row> = Vec::new();
        if query.is_empty() {
            rows.push(Row::NoProject);
        }
        rows.extend(scored.into_iter().take(cap).map(|(i, _)| Row::Project(i)));
        if hidden > 0 {
            rows.push(Row::Note(format!(
                "{hidden} more here — keep typing to narrow it, or type a path"
            )));
        }
        if rows.is_empty() {
            rows.push(Row::Note("Nothing matches. Type a path to use one.".into()));
        }
        rows
    }

    /// Models grouped by harness, each group led by that harness's own default
    /// — 119 of 165 real launches used one — then what has been reached for
    /// recently, then the rest of the catalog behind the search box.
    fn model_menu_rows(&self, query: &str) -> Vec<Row> {
        let all = self.model_rows();
        if !query.is_empty() {
            let mut seen: Vec<(String, String)> = Vec::new();
            let mut scored: Vec<(ModelRow, i32)> = all
                .into_iter()
                .filter(|r| {
                    let key = (r.provider.clone(), r.id.clone());
                    if seen.contains(&key) {
                        return false;
                    }
                    seen.push(key);
                    true
                })
                .filter_map(|r| {
                    fuzzy_score(query, &format!("{} {} {}", r.name, r.id, r.provider))
                        .map(|s| (r, s))
                })
                .collect();
            scored.sort_by_key(|(_, s)| *s);
            let hidden = scored.len().saturating_sub(MODELS_SEARCHED);
            let mut rows: Vec<Row> = scored
                .into_iter()
                .take(MODELS_SEARCHED)
                .map(|(r, _)| Row::Model(r))
                .collect();
            if hidden > 0 {
                rows.push(Row::Note(format!("{hidden} more match")));
            }
            if rows.is_empty() {
                rows.push(Row::Note("No model by that name".into()));
            }
            return rows;
        }
        let mut rows = Vec::new();
        for provider in PROVIDERS {
            // The catalog when it has loaded, plus any model this account
            // actually launched that the catalog does not list — otherwise the
            // menu is empty until the round trip finishes.
            let mut group: Vec<ModelRow> = all
                .iter()
                .filter(|r| !r.recent && r.provider == provider)
                .cloned()
                .collect();
            for row in all.iter().filter(|r| r.recent && r.provider == provider) {
                if !group.iter().any(|g| g.id == row.id) {
                    group.push(row.clone());
                }
            }
            if group.is_empty() {
                continue;
            }
            rows.push(Row::Group(provider_label(provider).to_string()));
            // OpenCode chooses among its own proxied catalog inside its own
            // interface, so listing those here implies a choice Ocho does not
            // make. Its default is the only row worth offering.
            let shown = if provider == "opencode" {
                1
            } else {
                MODELS_PER_HARNESS
            };
            let recent_first = self.recent_first(group, provider);
            let hidden = recent_first.len().saturating_sub(shown);
            rows.extend(recent_first.into_iter().take(shown).map(Row::Model));
            if hidden > 0 {
                rows.push(Row::Note(format!(
                    "{hidden} more {} models — type to search them",
                    provider_label(provider)
                )));
            }
        }
        rows
    }

    /// A harness's own default first, then the models this account has
    /// actually launched, then the catalog as the provider ordered it.
    fn recent_first(&self, group: Vec<ModelRow>, provider: &str) -> Vec<ModelRow> {
        let rank = |row: &ModelRow| -> usize {
            if row.id.is_empty() {
                return 0;
            }
            self.recent_models
                .iter()
                .filter(|m| m.provider == provider)
                .position(|m| m.id == row.id)
                .map(|at| at + 1)
                .unwrap_or(usize::MAX - 1)
        };
        // Stable sort, so models the account has not used keep the order the
        // provider published them in — newest first, not alphabetical.
        let mut out = group;
        out.sort_by_key(rank);
        out
    }

    fn account_rows(&self, accounts: &[AccountChoice], query: &str) -> Vec<Row> {
        let mut rows: Vec<Row> = accounts
            .iter()
            .filter(|a| {
                query.is_empty()
                    || fuzzy_score(query, &format!("{} {}", a.email, a.profile)).is_some()
            })
            .cloned()
            .map(Row::Account)
            .collect();
        if rows.is_empty() {
            rows.push(Row::Note(format!(
                "No {} account signed in on this machine",
                provider_label(&self.provider)
            )));
        }
        rows
    }

    /// Everything the line does not show, because launches almost never set it.
    fn more_rows(&self, machine_default: &str) -> Vec<Row> {
        let mut rows = Vec::new();
        if self.has_effort() {
            rows.push(Row::Group("Effort".into()));
            rows.extend(self.effort_levels().into_iter().map(Row::Effort));
        }
        rows.push(Row::Group("Guardrail".into()));
        rows.extend(
            self.permission_choices(machine_default)
                .into_iter()
                .map(Row::Perm),
        );
        rows
    }

    /// Items for a workspace popup over the composer. The composer's menus
    /// open inside its card now (`menu_rows`), so the retired
    /// `PopupKind::LaunchAccount` / `LaunchPermissions` popups are empty;
    /// kept so model/menus.rs compiles until it drops them.
    pub fn popup_items(
        &self,
        _kind: crate::picker::PopupKind,
        _state: &State,
    ) -> Vec<crate::picker::PopupItem> {
        Vec::new()
    }

    /// The models this account reached for last, in that order: what ← → on
    /// the model control walks.
    pub(super) fn recent_model_ring(&self) -> Vec<(String, String)> {
        self.recent_models
            .iter()
            .filter(|m| PROVIDERS.contains(&m.provider.as_str()))
            .map(|m| (m.provider.clone(), m.id.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::tests::{fresh, state};
    use crate::launch::{ModelState, Project, ProjectSource, RecentModel};

    #[test]
    fn the_project_menu_shows_one_machine_and_offers_its_home_folder() {
        let st = state();
        let mut launch = fresh(&st);
        launch.menu = Some(Menu::new(MenuKind::Project));
        let rows = launch.rows(&[], "");
        for row in &rows {
            if let Row::Project(i) = row {
                assert_eq!(launch.projects[*i].machine, "laptop");
            }
        }
        assert_eq!(rows.first(), Some(&Row::NoProject));
        assert!(rows.iter().any(|r| matches!(r, Row::Project(_))));

        launch.menu.as_mut().unwrap().query = "fleet".into();
        let rows = launch.rows(&[], "");
        let found: Vec<&str> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Project(i) => Some(launch.projects[*i].cwd.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(found, vec!["~/Developer/fleet"]);
        // The other machine's folders stay behind its own tab.
        assert!(!found.contains(&"/srv/app"));
        assert!(!rows.contains(&Row::NoProject));
        launch.menu.as_mut().unwrap().query = "zzzz".into();
        assert_eq!(
            launch.rows(&[], ""),
            vec![Row::Note("Nothing matches. Type a path to use one.".into())]
        );
    }

    #[test]
    fn a_path_query_lists_folders_from_the_machine_instead_of_projects() {
        let st = state();
        let mut launch = fresh(&st);
        let mut menu = Menu::new(MenuKind::Project);
        menu.query = "~/Devel".into();
        assert!(menu.is_path());
        menu.directory.paths = vec!["~/Developer".into(), "~/Developer/fleet".into()];
        launch.menu = Some(menu);
        let rows = launch.rows(&[], "");
        assert!(rows.iter().all(|r| matches!(r, Row::Folder(_))));
        // Whatever was typed stays usable even when nothing completed it.
        assert!(rows.contains(&Row::Folder("~/Devel".into())));
        let menu = launch.menu.as_mut().unwrap();
        menu.directory = DirectoryState {
            loading: true,
            ..Default::default()
        };
        menu.query = "/".into();
        let rows = launch.rows(&[], "");
        assert_eq!(rows, vec![Row::Folder("/".into())]);
        launch.menu.as_mut().unwrap().query = "./".into();
        launch.menu.as_mut().unwrap().directory.error = "gone".into();
        let rows = launch.rows(&[], "");
        assert_eq!(rows.last(), Some(&Row::Note("gone".into())));
    }

    #[test]
    fn long_lists_stop_short_and_say_how_much_is_left() {
        let st = state();
        let mut launch = fresh(&st);
        launch.projects = (0..PROJECTS_SHOWN + 4)
            .map(|i| Project {
                machine: "laptop".into(),
                machine_name: "LAPTOP".into(),
                cwd: format!("~/p{i}"),
                source: ProjectSource::Session,
            })
            .collect();
        launch.menu = Some(Menu::new(MenuKind::Project));
        let rows = launch.rows(&[], "");
        assert_eq!(
            rows.iter().filter(|r| matches!(r, Row::Project(_))).count(),
            PROJECTS_SHOWN
        );
        assert_eq!(
            rows.last(),
            Some(&Row::Note(
                "4 more here — keep typing to narrow it, or type a path".into()
            ))
        );
    }

    #[test]
    fn the_model_menu_groups_by_harness_and_leaves_opencodes_catalog_out() {
        let st = state();
        let mut launch = fresh(&st);
        let proxies = (0..30)
            .map(|i| ModelOption {
                id: format!("proxy-{i}"),
                name: format!("Proxy {i}"),
                ..Default::default()
            })
            .collect();
        launch.catalogs.insert(
            "opencode".into(),
            ModelState {
                options: proxies,
                ..Default::default()
            },
        );
        launch.menu = Some(Menu::new(MenuKind::Model));
        let rows = launch.rows(&[], "");
        let groups: Vec<&str> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Group(name) => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            groups,
            ["Codex", "Claude", "OpenCode", "Antigravity", "Grok"]
        );
        let opencode = rows
            .iter()
            .skip_while(|r| !matches!(r, Row::Group(n) if n == "OpenCode"))
            .skip(1)
            .take_while(|r| matches!(r, Row::Model(_)))
            .count();
        assert_eq!(opencode, 1);
        assert!(rows.contains(&Row::Note(
            "30 more OpenCode models — type to search them".into()
        )));
        // A search ranks across every harness and caps what it lists.
        launch.menu.as_mut().unwrap().query = "proxy".into();
        let rows = launch.rows(&[], "");
        assert_eq!(
            rows.iter().filter(|r| matches!(r, Row::Model(_))).count(),
            MODELS_SEARCHED
        );
        assert_eq!(rows.last(), Some(&Row::Note("5 more match".into())));
        launch.menu.as_mut().unwrap().query = "nothing-like-it".into();
        assert_eq!(
            launch.rows(&[], ""),
            vec![Row::Note("No model by that name".into())]
        );
    }

    #[test]
    fn a_harness_lists_its_catalog_in_the_order_it_published_it() {
        let st = state();
        let mut launch = fresh(&st);
        let options = ["gpt-6-astra", "gpt-6-sol", "daybreak-blue", "gpt-5.5"]
            .iter()
            .map(|id| ModelOption {
                id: (*id).into(),
                name: (*id).into(),
                ..Default::default()
            })
            .collect();
        launch.catalogs.insert(
            "codex".into(),
            ModelState {
                options,
                ..Default::default()
            },
        );
        launch.recent_models = vec![RecentModel {
            provider: "codex".into(),
            id: "gpt-5.5".into(),
            name: "gpt-5.5".into(),
        }];
        launch.menu = Some(Menu::new(MenuKind::Model));
        let listed: Vec<String> = launch
            .rows(&[], "")
            .into_iter()
            .skip_while(|r| !matches!(r, Row::Group(n) if n == "Codex"))
            .skip(1)
            .map_while(|r| match r {
                Row::Model(m) => Some(m.id),
                _ => None,
            })
            .collect();
        assert_eq!(
            listed,
            vec!["", "gpt-5.5", "gpt-6-astra", "gpt-6-sol", "daybreak-blue"]
        );
    }

    #[test]
    fn the_drawer_holds_effort_and_guardrails() {
        let st = state();
        let mut launch = fresh(&st);
        launch.menu = Some(Menu::new(MenuKind::More));
        assert!(!launch.menu.as_ref().unwrap().searchable());
        let rows = launch.rows(&[], "yolo");
        assert!(rows.contains(&Row::Group("Effort".into())));
        assert!(rows.contains(&Row::Group("Guardrail".into())));
        assert!(rows.contains(&Row::Effort("xhigh".into())));
        assert!(rows.contains(&Row::Perm("read-only".into())));
        let perms: Vec<&str> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Perm(mode) => Some(mode.as_str()),
                _ => None,
            })
            .collect();
        // The machine's own default leads, then the provider's own behaviour.
        assert_eq!(perms[0], "");
        assert_eq!(perms[1], crate::permissions::NATIVE);
        // OpenCode has no effort levels, so only the guardrails show.
        launch.provider = "opencode".into();
        assert_eq!(launch.rows(&[], "")[0], Row::Group("Guardrail".into()));
    }

    #[test]
    fn the_account_menu_names_accounts_by_email() {
        let st = state();
        let mut launch = fresh(&st);
        launch.menu = Some(Menu::new(MenuKind::Account));
        let accounts = vec![
            AccountChoice {
                handle: "claude:one@example.com".into(),
                email: "one@example.com".into(),
                profile: "claude-aaa".into(),
            },
            AccountChoice {
                handle: "claude:two@example.org".into(),
                email: "two@example.org".into(),
                profile: "claude-bbb".into(),
            },
        ];
        assert_eq!(launch.rows(&accounts, "").len(), 2);
        launch.menu.as_mut().unwrap().query = "example.org".into();
        assert_eq!(
            launch.rows(&accounts, ""),
            vec![Row::Account(accounts[1].clone())]
        );
        assert_eq!(
            launch.rows(&[], ""),
            vec![Row::Note(
                "No Codex account signed in on this machine".into()
            )]
        );
        launch.provider = "claude".into();
        let mine = launch.accounts(&st);
        assert_eq!(mine[0].email, "me@example.com");
        assert_eq!(mine[0].profile, "me");
        assert_eq!(mine[0].handle, "claude:me@example.com");
    }

    #[test]
    fn an_eas_machine_offers_only_codex_models() {
        let mut st = state();
        st.machines[0].eas = Some(Default::default());
        let mut launch = fresh(&st);
        launch.menu = Some(Menu::new(MenuKind::Model));
        let rows = launch.menu_rows(&st);
        assert!(rows
            .iter()
            .all(|r| !matches!(r, Row::Model(m) if m.provider != "codex")));
        assert!(rows.iter().any(|r| matches!(r, Row::Model(_))));
    }
}
