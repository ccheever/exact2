//! @exact/login-item on Windows (LLP 1133.025 D2): the host's login item
//! (`exact_linux::desktop`), a `Run` value whose command line carries
//! `--login`, and `hidden` with it. Settings › Apps › Startup turning it off
//! reads `requires-approval`. Under the agent the runner's substitute
//! answers, unless the drive binds it.

use exact_linux::desktop::{Login, Services};
use exact_linux::package::{json, Context, Json, Package, Reply};

pub struct LoginItem {
    context: Context,
}

fn status(login: &Login, host: &dyn Services) -> Json {
    json!({
        "available": true,
        "state": login.state,
        "hidden": login.hidden,
        "launchedAtLogin": host.login_launch().is_some(),
        "apple": null,
        "windows": { "startupTaskState": null },
    })
}

impl Package for LoginItem {
    fn new(context: Context) -> Self {
        LoginItem { context }
    }

    fn later(&mut self, name: &str, args: &Json, _preferred: Option<&Json>, reply: Reply) {
        let Some(host) = self.context.desktop() else {
            return reply.error("NotSupportedError", None);
        };
        let change = match name {
            "loginItemStatus" => return reply.send(status(&host.login(), &*host)),
            "openLoginItemSettings" => {
                if let Err(why) = host.open_login_settings() {
                    self.context.log(format!("openLoginItemSettings: {why}"));
                }
                return reply.send(json!({}));
            }
            "registerLoginItem" => Some(args["hidden"] == true),
            "unregisterLoginItem" => None,
            _ => return reply.error("NotSupportedError", None),
        };
        match host.set_login(change) {
            Ok(login) => {
                self.context.changed("login-item.loginItemStatus");
                reply.send(status(&login, &*host));
            }
            Err(why) => {
                self.context.log(format!("{name}: {why}"));
                reply.error("NotAllowedError", Some(&why));
            }
        }
    }

    fn call(&mut self, name: &str, _args: &Json) -> Json {
        match (name, self.context.desktop()) {
            ("loginItemStatusSync", Some(host)) => status(&host.login(), &*host),
            _ => json!({ "error": "NotSupportedError" }),
        }
    }

    /// The person may turn the item off in Settings while the app is away.
    fn foreground(&mut self, shown: bool) {
        if shown {
            self.context.changed("login-item.loginItemStatus");
        }
    }
}
