//! Updating a provider on a machine (workspace.rs `update_provider` and
//! `Overlay::ProviderPathFix`): `fleet machine update-provider M P`, then
//! `fleet machine fix-path M P` for a plan either way. A failed update
//! reports the shadow as its reason, and a successful one can still be
//! shadowed by an install already earlier on PATH; either way, a plan not
//! yet applied is offered at once.

use super::*;
use crate::types::PathFix;

impl Workspace {
    /// Yes on the provider update confirmation.
    pub(super) fn update_provider(&mut self, machine: Machine, provider: String) {
        let label = crate::session::provider_label(&provider).to_string();
        self.busy += 1;
        self.set_message(format!("Updating {label} on {}…", machine.name));
        let argv = vec![
            "machine".into(),
            "update-provider".into(),
            machine.id.clone(),
            provider.clone(),
        ];
        self.queue(
            "cli",
            argv,
            String::new(),
            Reply::ProviderUpdated { machine, provider },
        );
    }

    /// The update finished: ask for the PATH plan.
    pub(super) fn provider_updated(
        &mut self,
        machine: Machine,
        provider: String,
        result: Result<String, String>,
    ) {
        let argv = vec![
            "machine".into(),
            "fix-path".into(),
            machine.id.clone(),
            provider.clone(),
        ];
        self.queue(
            "cli",
            argv,
            String::new(),
            Reply::PathPlan {
                machine,
                provider,
                updated: result.map(|_| ()),
            },
        );
    }

    /// The PATH plan arrived: offer it, or report the update.
    pub(super) fn path_plan_arrived(
        &mut self,
        machine: Machine,
        provider: String,
        updated: Result<(), String>,
        result: Result<String, String>,
    ) {
        self.busy = self.busy.saturating_sub(1);
        let fix = result
            .ok()
            .and_then(|out| serde_json::from_str::<PathFix>(&out).ok())
            .filter(|fix| !fix.applied);
        let label = crate::session::provider_label(&provider).to_string();
        match (updated, fix) {
            (_, Some(fix)) => {
                self.overlay = Overlay::ProviderPathFix {
                    machine,
                    provider,
                    fix,
                };
            }
            (Ok(()), None) => {
                self.set_message(format!("{label} update completed on {}", machine.name))
            }
            (Err(error), None) => self.set_error(error),
        }
        self.refresh_feed();
    }

    /// Yes on the PATH fix.
    pub(super) fn apply_path_fix(&mut self, machine: Machine, provider: String) {
        let label = crate::session::provider_label(&provider).to_string();
        self.run_cli(
            vec![
                "machine".into(),
                "fix-path".into(),
                machine.id,
                provider,
                "--yes".into(),
            ],
            format!("PATH updated; open a new terminal to pick up {label}"),
        );
    }
}
