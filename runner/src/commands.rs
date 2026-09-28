//! The one door a host asks the runner through before it runs a command
//! that shows system UI (`exact_command` on the web and Apple): the
//! runner's ruling, refused, held for the agent, or present it. `command`
//! names which: `share` (LLP 1069.003), `saveFile` (LLP 1069.010 D3), and
//! the three pickers (D2).

use crate::agent::{error, field_str};
use crate::{DataSource, Runner};

/// `{"command":"share"|"saveFile",…}`: the named command's ruling, as its
/// module's `request` writes it.
pub fn request<D: DataSource>(runner: &mut Runner<D>, json: &str) -> String {
    match field_str(json, "command").as_deref() {
        Some("share") => crate::share::request(runner, json),
        Some("saveFile") => crate::save_file::request(runner, json),
        Some("showOpenFilePicker" | "showDirectoryPicker" | "showSaveFilePicker") => {
            crate::file_pickers::request(runner, json)
        }
        Some(other) => error(&format!("no ruling for command {other}")),
        None => error("no command"),
    }
}
