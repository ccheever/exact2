//! The forms, the two session finders and the theme picker as overlays
//! (workspace.rs `open_form`, `submit_form`, `Overlay::PullRequests`,
//! `Overlay::Conversations`, `Overlay::Themes`): opening, keys, presses,
//! and the replies of the `fleet` jobs they ask for.

use super::{Overlay, Page, PopupKind, Reply, SessionItem, Workspace};
use crate::finders::{self, FinderKey, OpenSession, TopicJob};
use crate::forms::{self, submit::Submission, Form, FormOutcome};
use crate::picker::Mods;
use crate::themes::{self, ThemeKey, ThemePicker};

impl Workspace {
    /// Open a form, restoring its kind's draft (workspace.rs `open_form`).
    pub fn open_form(&mut self, form: Form) {
        let kind = form.kind;
        let form = match self.form_drafts.get(&kind) {
            Some(draft) if form.keeps_draft() => form.with_draft(draft),
            _ => form,
        };
        self.focus_id = format!("field-{}", form.focus);
        self.overlay = Overlay::Form(Box::new(form));
        self.sync_form_io();
    }

    /// Open a form from a constructor that may refuse (the refusal is a toast).
    pub fn open_form_result(&mut self, form: Result<Form, String>) {
        match form {
            Ok(form) => self.open_form(form),
            Err(e) => self.set_error(e),
        }
    }

    /// Close the form, keeping a draft for the kinds that keep one.
    pub fn close_form(&mut self) {
        let Overlay::Form(form) = &self.overlay else {
            return;
        };
        if form.keeps_draft() {
            match form.draft() {
                Some((kind, values)) => {
                    self.form_drafts.insert(kind, values);
                    self.set_message(forms::DRAFT_KEPT);
                }
                None => {
                    self.form_drafts.remove(&form.kind);
                }
            }
        }
        self.overlay = Overlay::None;
    }

    /// What a form key or press asked for.
    pub fn form_outcome(&mut self, outcome: FormOutcome) {
        match outcome {
            FormOutcome::Close => self.close_form(),
            FormOutcome::Submit => self.submit_form(),
            FormOutcome::OpenChoice(index) => self.open_popup(PopupKind::FieldChoice(index), None),
            FormOutcome::None | FormOutcome::Ignored => self.sync_form_io(),
        }
    }

    /// Ask the module for the folders or the model catalog the form wants.
    pub fn sync_form_io(&mut self) {
        let Overlay::Form(form) = &mut self.overlay else {
            return;
        };
        if let Some(key) = form.wanted_catalog(&self.state) {
            form.catalog_requested(&key);
            let argv = key.argv(&self.state);
            self.queue("models", argv, String::new(), Reply::Models(key));
        }
        let Overlay::Form(form) = &mut self.overlay else {
            return;
        };
        if let Some((machine, path)) = form.wanted_directories() {
            form.directories_requested();
            self.queue(
                "directories",
                vec!["directories".into(), machine.clone(), path.clone()],
                String::new(),
                Reply::Directories(machine, path),
            );
        }
    }

    /// Submit the form (workspace.rs `submit_form`).
    pub fn submit_form(&mut self) {
        let request = self.request_id();
        // Pair iMessage submits to a card, not a fleet command (ui.rs render_pair_imessage).
        let phone = match &self.overlay {
            Overlay::Form(form) => form.pairing_phone(),
            _ => return,
        };
        match phone {
            Some(Ok(phone)) => {
                self.overlay = Overlay::None;
                self.pair_imessage_generate(phone);
                return;
            }
            Some(Err(e)) => {
                if let Overlay::Form(f) = &mut self.overlay {
                    f.error = e;
                }
                return;
            }
            None => {}
        }
        let Overlay::Form(form) = &self.overlay else {
            return;
        };
        let kind = form.kind;
        let result = form.submit(&self.state, &request);
        match result {
            Ok(submission) => {
                self.form_drafts.remove(&kind);
                self.overlay = Overlay::None;
                self.run_submission(submission);
            }
            Err(e) => {
                let retry = matches!(&self.overlay, Overlay::Form(f) if f.retry_on_error());
                if retry {
                    if let Overlay::Form(f) = &mut self.overlay {
                        f.error = e;
                    }
                } else {
                    self.overlay = Overlay::None;
                    self.set_error(e);
                }
            }
        }
    }

    fn run_submission(&mut self, submission: Submission) {
        match submission {
            Submission::Terminal {
                key,
                title,
                argv,
                reconnect,
                session,
                replaces,
            } => {
                if let Some((machine, session_id)) = replaces {
                    let doomed: Vec<usize> = self
                        .tabs
                        .tabs
                        .iter()
                        .enumerate()
                        .filter(|(_, t)| {
                            t.session
                                .as_ref()
                                .is_some_and(|(m, s, _)| *m == machine && *s == session_id)
                        })
                        .map(|(i, _)| i)
                        .rev()
                        .collect();
                    for i in doomed {
                        self.close_tab(i);
                    }
                }
                self.open_terminal_tab(key, title, argv, reconnect, session);
            }
            Submission::Cli {
                argv,
                done,
                page,
                reveal,
            } => {
                self.run_cli(argv, done);
                if let Some(page) = page {
                    self.tabs.select_manager();
                    self.set_page(page);
                }
                if let Some((machine, session)) = reveal {
                    let key = format!("{machine}:{session}:false");
                    if let Some(pos) = self.tabs.position(&key) {
                        self.tabs.select(pos);
                    }
                }
            }
            Submission::Profile { argv, .. } => {
                self.run_cli(argv, "Profile saved");
            }
            Submission::Folder { key, title } => {
                match key {
                    Some(key) => {
                        self.tabs.rename_folder(&key, &title);
                    }
                    None => {
                        let key = format!("folder:{}", self.request_id());
                        if self.tabs.new_folder(key, &title).is_none() {
                            self.set_error("Give the folder a name");
                        }
                    }
                }
                self.persist_tabs();
            }
        }
    }

    /// Open the PR finder.
    pub fn open_pull_requests(&mut self) {
        self.overlay =
            Overlay::PullRequests(finders::PullRequestFinder::open(&self.state.machines));
        self.focus_id = "query".into();
    }

    /// Open the topic finder; opening is the opt-in that indexes.
    pub fn open_conversations(&mut self) {
        self.topics.open(self.now);
        self.overlay = Overlay::Conversations;
        self.focus_id = "query".into();
        self.poll_topics();
    }

    /// Open the theme picker over the bundled themes and the files the
    /// module found (workspace.rs `open_themes`).
    pub fn open_themes(&mut self, from_settings: bool) {
        let mut list = themes::bundled();
        list.extend(self.theme_files.iter().cloned());
        self.overlay = Overlay::Themes(Box::new(ThemePicker::open(
            list,
            &self.theme,
            from_settings,
        )));
        self.focus_id = "query".into();
        self.queue(
            "host",
            vec!["theme-files".into()],
            String::new(),
            Reply::ThemeFiles,
        );
    }

    /// Run the topic finder's due job, if any.
    pub fn poll_topics(&mut self) {
        if let Some(job) = self.topics.wanted_job(self.now) {
            self.topics.begin(&job);
            self.queue(
                "topics",
                job.argv.clone(),
                String::new(),
                Reply::Topics(Box::new(job)),
            );
        }
    }

    /// A topic job's reply.
    pub fn topics_arrived(&mut self, job: TopicJob, result: Result<String, String>) {
        match self.topics.set(&job, result, self.now) {
            Ok(note) if !note.is_empty() => self.set_message(note),
            Ok(_) => {}
            Err(e) => self.set_error(e),
        }
        self.poll_topics();
    }

    /// The theme files the module listed: `[[path, text], …]`.
    pub fn theme_files_arrived(&mut self, text: &str) {
        let files: Vec<(String, String)> = serde_json::from_str(text).unwrap_or_default();
        self.theme_files = themes::discover(&files);
        if let Overlay::Themes(picker) = &mut self.overlay {
            let mut list = themes::bundled();
            list.extend(self.theme_files.iter().cloned());
            let from_settings = picker.from_settings;
            let previous = picker.previous.clone();
            let mut fresh = ThemePicker::open(list, &self.theme, from_settings);
            fresh.previous = previous;
            fresh.picker.query = picker.picker.query.clone();
            **picker = fresh;
        }
    }

    fn apply_theme_key(&mut self, key: ThemeKey) {
        match key {
            ThemeKey::Preview(theme) => self.theme = theme,
            ThemeKey::Keep {
                theme,
                from_settings,
            } => {
                self.theme = theme;
                self.settings.theme = Some(self.theme.name.clone());
                let text = self.settings.to_json_pretty();
                self.host(vec!["save-desktop".into()], text);
                self.set_message(themes::kept_message(&self.theme.name));
                self.overlay = Overlay::None;
                if from_settings {
                    self.open_settings();
                }
            }
            ThemeKey::Revert {
                theme,
                from_settings,
                ..
            } => {
                self.theme = theme;
                self.overlay = Overlay::None;
                if from_settings {
                    self.open_settings();
                }
            }
            ThemeKey::Typed(_) | ThemeKey::None => {}
        }
    }

    /// Open a session a finder chose: attach it, or show it in the list.
    fn open_found(&mut self, machine_id: &str, session_id: &str, open: OpenSession) {
        self.overlay = Overlay::None;
        let item = self
            .state
            .machines
            .iter()
            .find(|m| m.id == machine_id)
            .and_then(|m| {
                let s = m
                    .last
                    .as_ref()?
                    .sessions
                    .iter()
                    .find(|s| s.id == session_id)?;
                Some(SessionItem {
                    machine: m.clone(),
                    session: s.clone(),
                })
            });
        let Some(item) = item else {
            self.set_error("That session is no longer in the fleet");
            return;
        };
        match open {
            OpenSession::Attach => self.attach_item(item, false),
            OpenSession::Reveal {
                all_sessions,
                history,
            } => {
                self.tabs.select_manager();
                self.all_sessions |= all_sessions;
                self.history |= history;
                self.set_page(Page::Sessions);
                if let Some(index) = self
                    .sessions()
                    .iter()
                    .position(|i| i.machine.id == machine_id && i.session.id == session_id)
                {
                    self.select(index);
                }
            }
        }
    }

    /// A key while a dialog of this module is open; `true` when it was one.
    pub(super) fn dialog_key(&mut self, name: &str, mods: &Mods) -> bool {
        match &mut self.overlay {
            Overlay::Form(form) => {
                let outcome = form.key(name, mods, &self.state);
                self.form_outcome(outcome);
                true
            }
            Overlay::PullRequests(finder) => {
                match finder.key(name, mods) {
                    FinderKey::Chosen(choice) => {
                        if choice.unhide {
                            self.run_cli(
                                vec![
                                    "unhide".into(),
                                    choice.entry.machine_id.clone(),
                                    choice.entry.session.id.clone(),
                                ],
                                finders::UNHIDE_DONE,
                            );
                        }
                        let (m, s) = (
                            choice.entry.machine_id.clone(),
                            choice.entry.session.id.clone(),
                        );
                        self.open_found(&m, &s, choice.open);
                    }
                    FinderKey::Closed => self.overlay = Overlay::None,
                    _ => {}
                }
                true
            }
            Overlay::Conversations => {
                match self.topics.key(name, mods, self.now) {
                    FinderKey::Chosen(choice) => {
                        let (m, s) = (choice.hit.machine.clone(), choice.hit.session.clone());
                        self.open_found(&m, &s, choice.open);
                    }
                    FinderKey::Closed => {
                        self.topics.close();
                        self.overlay = Overlay::None;
                    }
                    _ => self.poll_topics(),
                }
                true
            }
            Overlay::Themes(picker) => {
                let key = picker.key(name, mods);
                self.apply_theme_key(key);
                true
            }
            _ => false,
        }
    }

    /// Text typed into a dialog's query or field; `true` when it was one.
    pub(super) fn dialog_input(&mut self, id: &str, value: &str) -> bool {
        match &mut self.overlay {
            Overlay::Form(form) => {
                if let Some(index) = id.strip_prefix("field:").and_then(|n| n.parse().ok()) {
                    form.input(index, value);
                    self.sync_form_io();
                }
                true
            }
            Overlay::PullRequests(finder) if id == "query" => {
                finder.input(value);
                true
            }
            Overlay::Conversations if id == "query" => {
                self.topics.input(value, self.now);
                true
            }
            Overlay::Themes(picker) if id == "query" => {
                picker.input(value);
                true
            }
            _ => false,
        }
    }

    /// A press inside a dialog (`field:N`, `pick:ID`, `button:ID`, …); `true` when it was one.
    pub(super) fn dialog_press(&mut self, kind: &str, rest: &str) -> bool {
        match &mut self.overlay {
            Overlay::Form(form) => {
                let id = match kind {
                    // A choice field's press opens its dropdown.
                    "field" => match rest.parse::<usize>().ok().and_then(|i| form.fields.get(i)) {
                        Some(f) if f.kind == forms::FieldKind::Choice => format!("choice:{rest}"),
                        _ => format!("field:{rest}"),
                    },
                    "pick" | "button" => rest.to_string(),
                    other => format!("{other}:{rest}"),
                };
                let outcome = form.press(&id, &self.state);
                self.form_outcome(outcome);
                true
            }
            Overlay::PullRequests(finder) if kind == "pick" => {
                if let Some(choice) = finder.press(rest) {
                    let (m, s) = (
                        choice.entry.machine_id.clone(),
                        choice.entry.session.id.clone(),
                    );
                    self.open_found(&m, &s, choice.open);
                }
                true
            }
            Overlay::Conversations if kind == "pick" => {
                if let Some(choice) = self.topics.press(rest) {
                    let (m, s) = (choice.hit.machine.clone(), choice.hit.session.clone());
                    self.open_found(&m, &s, choice.open);
                }
                true
            }
            Overlay::Themes(picker) if kind == "pick" => {
                let key = picker.press(rest);
                self.apply_theme_key(key);
                true
            }
            _ => false,
        }
    }

    /// The dialog's `OVERLAY` JSON, when one of these is open.
    pub fn dialog_view(&self) -> Option<serde_json::Value> {
        Some(match &self.overlay {
            Overlay::Form(form) => form.view(&self.state),
            Overlay::PullRequests(finder) => finder.view(),
            Overlay::Conversations => self.topics.view(),
            Overlay::Themes(picker) => picker.view(),
            _ => return None,
        })
    }
}
