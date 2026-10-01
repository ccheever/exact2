//! Application-wide update state (a port of the GPUI desktop's `updater.rs`)
//! and the feed watcher's restart backoff (`feed.rs`). Network and
//! installation work runs in the bundled CLI (`fleet desktop-update …`);
//! this decides what to run and when, and reads what came back. Instants
//! are host milliseconds (`f64`).

use serde::Deserialize;

/// Where the update flow is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateState {
    /// Nothing to show (the rail's update button is hidden).
    #[default]
    Idle,
    /// A manual check is running.
    Checking,
    /// A manual check found nothing.
    Current,
    /// An update is being fetched.
    Downloading,
    /// An update is staged; a restart installs it.
    Ready {
        /// The commit the download carries.
        commit: String,
        /// The staging directory the CLI reported.
        stage: String,
    },
    /// The install was asked for; the app quits when it succeeds.
    Installing,
    /// Something went wrong; a click checks again.
    Failed(String),
}

impl UpdateState {
    /// The rail button's label; None hides the button.
    pub fn label(&self) -> Option<&'static str> {
        match self {
            Self::Idle => None,
            Self::Checking => Some("Checking for updates…"),
            Self::Current => Some("Up to date"),
            Self::Downloading => Some("Downloading update…"),
            Self::Ready { .. } => Some("Restart to update"),
            Self::Installing => Some("Restarting…"),
            Self::Failed(_) => Some("Update failed · Retry"),
        }
    }

    /// The button's tooltip.
    pub fn detail(&self) -> String {
        match self {
            Self::Downloading => "Downloading and verifying the update in the background.".into(),
            Self::Ready { .. } => "The update is ready. Restart Ocho to install it.".into(),
            Self::Installing => "Ocho will restart to finish installing the update.".into(),
            Self::Failed(error) => format!("{error}\nClick to check again."),
            Self::Current => "Ocho is running the latest published build.".into(),
            _ => "Check for Ocho updates".into(),
        }
    }
}

/// What `fleet desktop-update check` prints.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Status {
    /// The running build can be updated (a released bundle).
    pub supported: bool,
    /// A newer build is published.
    pub available: bool,
    /// The newest published commit.
    #[serde(default)]
    pub latest_commit: String,
    /// The helper's last error, when it has one.
    #[serde(default)]
    pub last_error: String,
}

/// What `fleet desktop-update download` prints.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Receipt {
    /// Where the build was staged.
    pub stage: String,
    /// The commit it carries.
    pub commit: String,
}

/// The `check` output, or why it could not be read.
pub fn parse_status(text: &str) -> Result<Status, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

/// The `download` output, or why it could not be read.
pub fn parse_receipt(text: &str) -> Result<Receipt, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

/// The message when the running app is not a released bundle.
pub const NOT_A_BUNDLE: &str = "Install a released Ocho.app to enable updates.";
/// The acknowledgement runs this long after launch: a launch that survives
/// its first second is a usable one, not a crash.
pub const ACKNOWLEDGE_AFTER_MS: f64 = 1_000.0;
/// Automatic checks repeat this often.
pub const CHECK_INTERVAL_MS: f64 = 15.0 * 60.0 * 1_000.0;

/// A `fleet desktop-update …` command for the app's module to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateCommand {
    /// `desktop-update acknowledge`: the launch was usable.
    Acknowledge,
    /// `desktop-update check`; `manual` says whether a person asked.
    Check {
        /// A person clicked, so the outcome is shown either way.
        manual: bool,
    },
    /// `desktop-update download --commit C`.
    Download {
        /// The commit to fetch.
        commit: String,
    },
    /// `desktop-update install --stage S --commit C --pid P`.
    Install {
        /// The staged build.
        stage: String,
        /// Its commit.
        commit: String,
    },
}

impl UpdateCommand {
    /// The arguments after `fleet`; `pid` is this process, for `install`.
    pub fn argv(&self, pid: u32) -> Vec<String> {
        let mut args = vec!["desktop-update".to_string()];
        match self {
            UpdateCommand::Acknowledge => args.push("acknowledge".into()),
            UpdateCommand::Check { .. } => args.push("check".into()),
            UpdateCommand::Download { commit } => {
                args.extend(["download".into(), "--commit".into(), commit.clone()]);
            }
            UpdateCommand::Install { stage, commit } => {
                args.extend([
                    "install".into(),
                    "--stage".into(),
                    stage.clone(),
                    "--commit".into(),
                    commit.clone(),
                    "--pid".into(),
                    pid.to_string(),
                ]);
            }
        }
        args
    }
}

/// The update state machine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Updater {
    /// Where the flow is.
    pub state: UpdateState,
    /// The running app is a released `Ocho.app` bundle with `fleet` inside;
    /// only then can it update itself (a development `FLEET_BIN` never does).
    pub bundled: bool,
    /// A check is running.
    checking: bool,
    /// The helper's last error already shown, so it is not shown twice.
    last_reported_error: String,
    /// When the windows were up, host ms.
    started_at: Option<f64>,
    /// The acknowledgement ran.
    acknowledged: bool,
    /// When the last automatic check started.
    last_check_at: Option<f64>,
}

impl Updater {
    /// Before the windows are up.
    pub fn new(bundled: bool) -> Updater {
        Updater {
            bundled,
            ..Default::default()
        }
    }

    /// The windows are up (`Updater::started`): the first check runs now,
    /// and the acknowledgement is due a second later.
    pub fn started(&mut self, now_ms: f64) -> Option<UpdateCommand> {
        self.started_at = Some(now_ms);
        self.last_check_at = Some(now_ms);
        self.check(false)
    }

    /// When the schedule next wants a tick: the acknowledgement a second
    /// after launch, then a check every 15 minutes. None before `started`
    /// or when the app cannot update itself.
    pub fn next_due(&self) -> Option<f64> {
        let started = self.started_at?;
        if !self.bundled {
            return None;
        }
        let check = self.last_check_at.unwrap_or(started) + CHECK_INTERVAL_MS;
        if self.acknowledged {
            Some(check)
        } else {
            Some((started + ACKNOWLEDGE_AFTER_MS).min(check))
        }
    }

    /// The commands due at `now_ms`: the acknowledgement once, and an
    /// automatic check when 15 minutes have passed since the last.
    pub fn tick(&mut self, now_ms: f64) -> Vec<UpdateCommand> {
        let mut due = Vec::new();
        let Some(started) = self.started_at else {
            return due;
        };
        if !self.bundled {
            return due;
        }
        if !self.acknowledged && now_ms >= started + ACKNOWLEDGE_AFTER_MS {
            self.acknowledged = true;
            due.push(UpdateCommand::Acknowledge);
        }
        if now_ms >= self.last_check_at.unwrap_or(started) + CHECK_INTERVAL_MS {
            self.last_check_at = Some(now_ms);
            due.extend(self.check(false));
        }
        due
    }

    /// Start a check unless one is running or an update is under way. A
    /// manual check shows "Checking for updates…" and, outside a bundle,
    /// fails visibly.
    pub fn check(&mut self, manual: bool) -> Option<UpdateCommand> {
        if self.checking
            || matches!(
                self.state,
                UpdateState::Downloading | UpdateState::Ready { .. } | UpdateState::Installing
            )
        {
            return None;
        }
        if !self.bundled {
            if manual {
                self.state = UpdateState::Failed(NOT_A_BUNDLE.into());
            }
            return None;
        }
        self.checking = true;
        if manual {
            self.state = UpdateState::Checking;
        }
        Some(UpdateCommand::Check { manual })
    }

    /// The `check` command finished with its stdout, or an error. An
    /// available update starts its download.
    pub fn checked(&mut self, manual: bool, result: Result<&str, String>) -> Option<UpdateCommand> {
        self.checking = false;
        if matches!(self.state, UpdateState::Installing) {
            return None;
        }
        match result.and_then(parse_status) {
            Ok(status) => {
                if !status.last_error.is_empty() && status.last_error != self.last_reported_error {
                    self.last_reported_error = status.last_error.clone();
                    self.state = UpdateState::Failed(status.last_error);
                } else if status.supported && status.available {
                    return self.download(status.latest_commit);
                } else if manual && status.supported {
                    self.state = UpdateState::Current;
                } else if manual {
                    self.state = UpdateState::Failed(NOT_A_BUNDLE.into());
                } else {
                    self.state = UpdateState::Idle;
                }
            }
            Err(error) if manual => self.state = UpdateState::Failed(error),
            // An automatic check's failure is only logged ("Ocho update check: …").
            Err(_) => {}
        }
        None
    }

    fn download(&mut self, commit: String) -> Option<UpdateCommand> {
        if !self.bundled {
            return None;
        }
        self.state = UpdateState::Downloading;
        Some(UpdateCommand::Download { commit })
    }

    /// The `download` command finished with its stdout, or an error.
    pub fn downloaded(&mut self, result: Result<&str, String>) {
        self.state = match result.and_then(parse_receipt) {
            Ok(receipt) => UpdateState::Ready {
                commit: receipt.commit,
                stage: receipt.stage,
            },
            Err(error) => UpdateState::Failed(error),
        };
    }

    /// The rail button was clicked: install a ready update, otherwise check
    /// again (a manual check).
    pub fn install(&mut self) -> Option<UpdateCommand> {
        if matches!(
            self.state,
            UpdateState::Downloading | UpdateState::Installing
        ) {
            return None;
        }
        let UpdateState::Ready { commit, stage } = &self.state else {
            return self.check(true);
        };
        let command = UpdateCommand::Install {
            stage: stage.clone(),
            commit: commit.clone(),
        };
        if !self.bundled {
            return None;
        }
        self.state = UpdateState::Installing;
        Some(command)
    }

    /// The `install` command finished. True: quit, the CLI restarts the app.
    pub fn installed(&mut self, result: Result<(), String>) -> bool {
        match result {
            Ok(()) => true,
            Err(error) => {
                self.state = UpdateState::Failed(error);
                false
            }
        }
    }

    /// A check is running.
    pub fn is_checking(&self) -> bool {
        self.checking
    }
}

// ----- the feed watcher's restart backoff (feed.rs) -------------------------

// Realized by the host: Ocho.swift FleetFeed restarts with this backoff.
#[allow(dead_code)]
/// How long a watcher stays down after its `failures`-th consecutive exit:
/// 2 s, 4 s, 8 s … capped at a minute. A user's refresh or restart ignores it.
pub fn restart_delay_secs(failures: u32) -> u64 {
    let seconds = 2u64.saturating_pow(failures.clamp(1, 6));
    seconds.min(60)
}

// Realized by the host: Ocho.swift FleetFeed restarts with this backoff.
#[allow(dead_code)]
/// A watcher that has run this long without exiting has left any crash loop
/// behind; only then does a completed observation clear earlier failures.
/// A crashing watcher can still emit a state event and even a machine event
/// before it dies, so those alone prove nothing.
pub const SURVIVAL_MS: f64 = 30_000.0;

// Realized by the host: Ocho.swift FleetFeed restarts with this backoff.
#[allow(dead_code)]
/// Whether an event of `kind` from a watcher started at `started_at` clears
/// the failure count.
pub fn proves_healthy(kind: &str, started_at: Option<f64>, now_ms: f64) -> bool {
    kind == "machine" && started_at.is_some_and(|at| now_ms - at >= SURVIVAL_MS)
}

// Realized by the host: Ocho.swift FleetFeed restarts with this backoff.
#[allow(dead_code)]
/// Consecutive watcher exits and the hold they put on the next start.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Backoff {
    /// Consecutive exits without a healthy round in between.
    failures: u32,
    /// No automatic restart before this; a crash loop otherwise respawns the
    /// CLI on every tick, spending a process and a stack trace every 8 s.
    retry_after: Option<f64>,
    /// When the current watcher was started; a crash within `SURVIVAL_MS`
    /// counts against it however much it emitted first.
    started_at: Option<f64>,
}

// Realized by the host: Ocho.swift FleetFeed restarts with this backoff.
#[allow(dead_code)]
impl Backoff {
    /// No failures yet.
    pub fn new() -> Backoff {
        Backoff::default()
    }

    /// Consecutive exits so far.
    pub fn failures(&self) -> u32 {
        self.failures
    }

    /// When an automatic restart may run, if one is held.
    pub fn retry_after(&self) -> Option<f64> {
        self.retry_after
    }

    /// A watcher was started now.
    pub fn started(&mut self, now_ms: f64) {
        self.started_at = Some(now_ms);
    }

    /// Whether an automatic start may run now (`Feed::ensure`).
    pub fn may_start(&self, now_ms: f64) -> bool {
        !self.retry_after.is_some_and(|at| now_ms < at)
    }

    /// A person asked (`wake`, `restart`): the hold is lifted.
    pub fn reset(&mut self) {
        self.failures = 0;
        self.retry_after = None;
    }

    /// An event of `kind` arrived; a machine event from a watcher that has
    /// survived 30 s clears the failures. True when it did.
    pub fn observed(&mut self, kind: &str, now_ms: f64) -> bool {
        if proves_healthy(kind, self.started_at, now_ms) {
            self.reset();
            return true;
        }
        false
    }

    /// Record an exit of the current watcher and decide when to try again.
    /// Returns what to tell the user, if anything: the CLI's stderr tail when
    /// there is one, or the loop itself once it repeats.
    pub fn note_exit(&mut self, now_ms: f64, tail: &str) -> String {
        self.failures = self.failures.saturating_add(1);
        let delay = restart_delay_secs(self.failures);
        self.retry_after = Some(now_ms + delay as f64 * 1_000.0);
        if !tail.is_empty() {
            format!("{tail} · retrying in {delay} s")
        } else if self.failures > 1 {
            format!(
                "exited {} times in a row · retrying in {delay} s",
                self.failures
            )
        } else {
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_details_follow_the_state() {
        assert_eq!(UpdateState::Idle.label(), None);
        assert_eq!(UpdateState::Checking.label(), Some("Checking for updates…"));
        assert_eq!(UpdateState::Current.label(), Some("Up to date"));
        assert_eq!(
            UpdateState::Downloading.label(),
            Some("Downloading update…")
        );
        let ready = UpdateState::Ready {
            commit: "c".into(),
            stage: "s".into(),
        };
        assert_eq!(ready.label(), Some("Restart to update"));
        assert_eq!(UpdateState::Installing.label(), Some("Restarting…"));
        assert_eq!(
            UpdateState::Failed("x".into()).label(),
            Some("Update failed · Retry")
        );
        assert_eq!(
            UpdateState::Failed("boom".into()).detail(),
            "boom\nClick to check again."
        );
        assert_eq!(UpdateState::Idle.detail(), "Check for Ocho updates");
        assert_eq!(UpdateState::Checking.detail(), "Check for Ocho updates");
        assert_eq!(
            ready.detail(),
            "The update is ready. Restart Ocho to install it."
        );
        assert_eq!(
            UpdateState::Current.detail(),
            "Ocho is running the latest published build."
        );
    }

    #[test]
    fn cli_outputs_are_read_as_the_desktop_reads_them() {
        let status =
            parse_status(r#"{"supported":true,"available":true,"latest_commit":"abc"}"#).unwrap();
        assert_eq!(
            status,
            Status {
                supported: true,
                available: true,
                latest_commit: "abc".into(),
                last_error: String::new()
            }
        );
        assert!(parse_status(r#"{"available":true}"#).is_err());
        assert!(parse_status("not json").is_err());
        let receipt = parse_receipt(r#"{"stage":"/tmp/stage","commit":"abc"}"#).unwrap();
        assert_eq!(
            receipt,
            Receipt {
                stage: "/tmp/stage".into(),
                commit: "abc".into()
            }
        );
        assert!(parse_receipt(r#"{"stage":"/tmp/stage"}"#).is_err());
        assert_eq!(
            UpdateCommand::Acknowledge.argv(7),
            ["desktop-update", "acknowledge"]
        );
        assert_eq!(
            UpdateCommand::Check { manual: true }.argv(7),
            ["desktop-update", "check"]
        );
        assert_eq!(
            UpdateCommand::Download {
                commit: "abc".into()
            }
            .argv(7),
            ["desktop-update", "download", "--commit", "abc"]
        );
        assert_eq!(
            UpdateCommand::Install {
                stage: "/s".into(),
                commit: "abc".into()
            }
            .argv(42),
            [
                "desktop-update",
                "install",
                "--stage",
                "/s",
                "--commit",
                "abc",
                "--pid",
                "42"
            ]
        );
    }

    #[test]
    fn the_flow_checks_downloads_and_installs() {
        let mut u = Updater::new(true);
        assert_eq!(
            u.started(1_000.0),
            Some(UpdateCommand::Check { manual: false })
        );
        assert_eq!(u.state, UpdateState::Idle);
        assert!(u.is_checking());
        // A second check waits for the first.
        assert_eq!(u.check(true), None);
        // Nothing available: an automatic check stays quiet.
        assert_eq!(
            u.checked(false, Ok(r#"{"supported":true,"available":false}"#)),
            None
        );
        assert_eq!(u.state, UpdateState::Idle);
        // A manual one says so.
        assert_eq!(u.check(true), Some(UpdateCommand::Check { manual: true }));
        assert_eq!(u.state, UpdateState::Checking);
        assert_eq!(
            u.checked(true, Ok(r#"{"supported":true,"available":false}"#)),
            None
        );
        assert_eq!(u.state, UpdateState::Current);
        // Unsupported build, manual check.
        u.check(true);
        u.checked(true, Ok(r#"{"supported":false,"available":false}"#));
        assert_eq!(u.state, UpdateState::Failed(NOT_A_BUNDLE.into()));
        // The helper's error shows once.
        u.check(true);
        u.checked(
            true,
            Ok(r#"{"supported":true,"available":true,"last_error":"disk full"}"#),
        );
        assert_eq!(u.state, UpdateState::Failed("disk full".into()));
        u.check(true);
        assert_eq!(
            u.checked(true, Ok(r#"{"supported":true,"available":true,"latest_commit":"abc","last_error":"disk full"}"#)),
            Some(UpdateCommand::Download { commit: "abc".into() })
        );
        assert_eq!(u.state, UpdateState::Downloading);
        // No check while downloading; the button does nothing either.
        assert_eq!(u.check(true), None);
        assert_eq!(u.install(), None);
        u.downloaded(Ok(r#"{"stage":"/s","commit":"abc"}"#));
        assert_eq!(
            u.state,
            UpdateState::Ready {
                commit: "abc".into(),
                stage: "/s".into()
            }
        );
        assert_eq!(u.check(false), None);
        assert_eq!(
            u.install(),
            Some(UpdateCommand::Install {
                stage: "/s".into(),
                commit: "abc".into()
            })
        );
        assert_eq!(u.state, UpdateState::Installing);
        // A late check result does not disturb the install.
        u.checked(false, Ok(r#"{"supported":true,"available":false}"#));
        assert_eq!(u.state, UpdateState::Installing);
        assert!(!u.installed(Err("no".into())));
        assert_eq!(u.state, UpdateState::Failed("no".into()));
        // The button on a failure checks again.
        assert_eq!(u.install(), Some(UpdateCommand::Check { manual: true }));
        assert_eq!(u.state, UpdateState::Checking);
        u.checked(true, Err("timeout".into()));
        assert_eq!(u.state, UpdateState::Failed("timeout".into()));
        // An automatic check's failure is silent.
        u.check(false);
        u.checked(false, Err("timeout".into()));
        assert_eq!(u.state, UpdateState::Failed("timeout".into()));
        u.check(false);
        u.checked(false, Ok("garbage"));
        assert_eq!(u.state, UpdateState::Failed("timeout".into()));
        u.check(false);
        assert_eq!(
            u.checked(
                false,
                Ok(r#"{"supported":true,"available":true,"latest_commit":"def"}"#)
            ),
            Some(UpdateCommand::Download {
                commit: "def".into()
            })
        );
        u.downloaded(Err("checksum".into()));
        assert_eq!(u.state, UpdateState::Failed("checksum".into()));
        let mut ready = Updater {
            state: UpdateState::Ready {
                commit: "c".into(),
                stage: "s".into(),
            },
            bundled: true,
            ..Default::default()
        };
        assert!(ready.install().is_some());
        assert!(ready.installed(Ok(())));
    }

    #[test]
    fn outside_a_bundle_only_a_manual_check_says_anything() {
        let mut u = Updater::new(false);
        assert_eq!(u.started(0.0), None);
        assert_eq!(u.state, UpdateState::Idle);
        assert_eq!(u.next_due(), None);
        assert!(u.tick(100_000.0).is_empty());
        assert_eq!(u.check(true), None);
        assert_eq!(u.state, UpdateState::Failed(NOT_A_BUNDLE.into()));
        assert_eq!(u.install(), None);
    }

    #[test]
    fn the_schedule_acknowledges_after_a_second_and_checks_every_quarter_hour() {
        let mut u = Updater::new(true);
        assert_eq!(u.next_due(), None);
        assert!(u.tick(0.0).is_empty());
        u.started(10_000.0);
        u.checked(false, Ok(r#"{"supported":true,"available":false}"#));
        assert_eq!(u.next_due(), Some(11_000.0));
        assert!(u.tick(10_500.0).is_empty());
        assert_eq!(u.tick(11_000.0), vec![UpdateCommand::Acknowledge]);
        assert!(u.tick(11_000.0).is_empty());
        assert_eq!(u.next_due(), Some(10_000.0 + CHECK_INTERVAL_MS));
        assert!(u.tick(10_000.0 + CHECK_INTERVAL_MS - 1.0).is_empty());
        assert_eq!(
            u.tick(10_000.0 + CHECK_INTERVAL_MS),
            vec![UpdateCommand::Check { manual: false }]
        );
        assert_eq!(u.next_due(), Some(10_000.0 + 2.0 * CHECK_INTERVAL_MS));
        // A check that is still running is not started again; the slot passes.
        assert!(u.tick(10_000.0 + 2.0 * CHECK_INTERVAL_MS).is_empty());
        u.checked(false, Ok(r#"{"supported":true,"available":false}"#));
        // A late acknowledgement and a check can come in one tick.
        let mut late = Updater::new(true);
        late.started(0.0);
        late.checked(false, Ok(r#"{"supported":true,"available":false}"#));
        assert_eq!(
            late.tick(CHECK_INTERVAL_MS),
            vec![
                UpdateCommand::Acknowledge,
                UpdateCommand::Check { manual: false }
            ]
        );
    }

    #[test]
    fn only_a_watcher_that_survived_clears_the_failure_count() {
        let now = 100_000.0;
        let fresh = Some(now - 5_000.0);
        let old = Some(now - SURVIVAL_MS);
        // A crashing watcher emits state and even a machine event before it dies.
        assert!(!proves_healthy("state", fresh, now));
        assert!(!proves_healthy("machine", fresh, now));
        assert!(!proves_healthy("state", old, now));
        assert!(proves_healthy("machine", old, now));
        assert!(!proves_healthy("machine", None, now));
    }

    #[test]
    fn restarts_back_off_and_cap_at_a_minute() {
        assert_eq!(restart_delay_secs(0), 2);
        assert_eq!(restart_delay_secs(1), 2);
        assert_eq!(restart_delay_secs(2), 4);
        assert_eq!(restart_delay_secs(3), 8);
        assert_eq!(restart_delay_secs(5), 32);
        assert_eq!(restart_delay_secs(6), 60);
        assert_eq!(restart_delay_secs(40), 60);
    }

    #[test]
    fn the_backoff_holds_restarts_and_resets_after_a_healthy_half_minute() {
        let mut b = Backoff::new();
        assert!(b.may_start(0.0));
        b.started(0.0);
        // The first exit with no stderr says nothing.
        assert_eq!(b.note_exit(1_000.0, ""), "");
        assert_eq!(b.failures(), 1);
        assert_eq!(b.retry_after(), Some(3_000.0));
        assert!(!b.may_start(2_999.0));
        assert!(b.may_start(3_000.0));
        b.started(3_000.0);
        assert_eq!(
            b.note_exit(4_000.0, ""),
            "exited 2 times in a row · retrying in 4 s"
        );
        b.started(6_000.0);
        assert_eq!(b.note_exit(7_000.0, "boom"), "boom · retrying in 8 s");
        assert_eq!(b.retry_after(), Some(15_000.0));
        b.started(15_000.0);
        assert_eq!(
            b.note_exit(12_000.0, ""),
            "exited 4 times in a row · retrying in 16 s"
        );
        for _ in 0..10 {
            b.note_exit(12_000.0, "");
        }
        assert_eq!(b.note_exit(12_000.0, "x"), "x · retrying in 60 s");
        // Events from a young watcher prove nothing.
        b.started(100_000.0);
        assert!(!b.observed("state", 140_000.0));
        assert!(!b.observed("machine", 120_000.0));
        assert_eq!(b.failures(), 15);
        assert!(b.observed("machine", 130_000.0));
        assert_eq!(b.failures(), 0);
        assert_eq!(b.retry_after(), None);
        // A person's restart lifts the hold at once.
        b.note_exit(200_000.0, "");
        assert!(!b.may_start(200_500.0));
        b.reset();
        assert!(b.may_start(200_500.0));
        assert_eq!(b.failures(), 0);
    }
}
