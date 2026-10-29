//! @exact/global-shortcuts on Windows (LLP 1133.025 D3): a chord registered
//! system-wide while something reads it, through the host's one hot-key
//! table (`exact_linux::desktop`, `RegisterHotKey`), which raises
//! `globalshortcut` as a person's activation when it is pressed. Under the
//! agent the runner's substitute answers, unless the drive binds it.

use exact_linux::package::{json, Context, Json, Package, Reply};

pub struct GlobalShortcuts {
    context: Context,
    /// The chords this session reads: `(id, keys)`.
    reading: Vec<(String, String)>,
}

impl GlobalShortcuts {
    fn answer(keys: &str, outcome: Result<(), &str>) -> Json {
        match outcome {
            Ok(()) => json!({ "keys": keys, "registered": true, "reason": null }),
            Err(reason) => json!({ "keys": keys, "registered": false, "reason": reason }),
        }
    }
}

impl Package for GlobalShortcuts {
    fn new(context: Context) -> Self {
        GlobalShortcuts {
            context,
            reading: Vec::new(),
        }
    }

    fn later(&mut self, name: &str, args: &Json, _preferred: Option<&Json>, reply: Reply) {
        let Some(host) = self.context.desktop() else {
            return reply.error("NotSupportedError", None);
        };
        let keys = args["keys"].as_str().unwrap_or_default();
        match name {
            "globalShortcut" => {
                let id = args["id"].as_str().unwrap_or_default();
                let (session, extension) =
                    (self.context.session().to_owned(), self.context.extension());
                let outcome = host.register_hotkey(&session, id, keys, extension, "globalshortcut");
                let reading = (id.to_owned(), keys.to_owned());
                if outcome.is_ok() && !self.reading.contains(&reading) {
                    self.reading.push(reading);
                }
                let registered = match outcome {
                    Ok(()) => "registered".to_owned(),
                    Err(reason) => format!("not registered ({reason})"),
                };
                self.context.log(format!("{keys} for {id}: {registered}"));
                let mut answer = Self::answer(keys, outcome);
                answer["id"] = json!(id);
                reply.send(answer);
            }
            "checkGlobalShortcut" => reply.send(Self::answer(keys, host.check_hotkey(keys))),
            "globalShortcutStatus" => reply.send(json!({ "available": true })),
            _ => reply.error("NotSupportedError", None),
        }
    }

    fn call(&mut self, name: &str, _args: &Json) -> Json {
        match (name, self.context.desktop()) {
            ("globalShortcutStatusSync", Some(_)) => json!({ "available": true }),
            _ => json!({ "error": "NotSupportedError" }),
        }
    }

    fn release(&mut self, name: &str, args: Option<&Json>) {
        let Some(host) = self.context.desktop().filter(|_| name == "globalShortcut") else {
            return;
        };
        let session = self.context.session().to_owned();
        self.reading.retain(|(id, keys)| {
            let gone = args.is_none_or(|a| a["id"] == id.as_str() && a["keys"] == keys.as_str());
            if gone {
                host.unregister_hotkey(&session, id, keys);
            }
            !gone
        });
    }

    fn destroy(&mut self) {
        self.release("globalShortcut", None);
    }
}
