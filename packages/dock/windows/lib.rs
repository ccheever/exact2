//! @exact/dock on Windows (LLP 1133.025 D5): `requestAttention` flashes the
//! window's taskbar button through the host (`exact_linux::desktop`,
//! `FlashWindowEx`): until the window comes forward for a critical request,
//! three times otherwise. A hidden window has no taskbar button, so nothing
//! is asked, and the journal says why.

use exact_linux::package::{json, Context, Json, Package, Reply};

pub struct Dock {
    context: Context,
}

impl Package for Dock {
    fn new(context: Context) -> Self {
        Dock { context }
    }

    fn later(&mut self, name: &str, args: &Json, _preferred: Option<&Json>, reply: Reply) {
        let (Some(host), "requestAttention") = (self.context.desktop(), name) else {
            return reply.error("NotSupportedError", None);
        };
        let critical = args["critical"] == true;
        match host.attention(critical) {
            Ok(()) => self.context.log(format!(
                "requestAttention: {}",
                if critical {
                    "critical"
                } else {
                    "informational"
                }
            )),
            Err(why) => self.context.log(format!("requestAttention: {why}")),
        }
        reply.send(json!({}));
    }
}
