//! The one door a host asks the runner through before it runs a command
//! that shows system UI (`exact_command` on the web and Apple): the
//! runner's ruling, refused, held for the agent, or present it. `command`
//! names which: `share` (LLP 1069.003), `saveFile` (LLP 1069.010 D3), the
//! three pickers (D2), and `showNotification`/`closeNotification`.

use crate::agent::{error, field_str};
use crate::{DataSource, Runner};

/// `{"command":"share"|"saveFile",…}`: the named command's ruling, as its
/// module's `request` writes it.
pub fn request<D: DataSource>(runner: &mut Runner<D>, json: &str) -> String {
    // Each command's ruling is reached through what the runner links (LLP
    // 1047 D3): a plan that runs one is refused at boot where it isn't.
    let links = runner.device_links();
    match field_str(json, "command").as_deref() {
        Some("share") => match links.share {
            Some((request, _)) => request(runner, json),
            None => error("share is not linked into this artifact"),
        },
        Some("saveFile") => match links.documents {
            Some((save, _)) => save(runner, json),
            None => error("saveFile is not linked into this artifact"),
        },
        Some("showOpenFilePicker" | "showDirectoryPicker" | "showSaveFilePicker") => {
            match links.documents {
                Some((_, pick)) => pick(runner, json),
                None => error("the file pickers are not linked into this artifact"),
            }
        }
        // Local notifications are the core's: a small rule and no answer.
        Some("showNotification" | "closeNotification") => crate::notify::request(runner, json),
        Some(other) => error(&format!("no ruling for command {other}")),
        None => error("no command"),
    }
}
