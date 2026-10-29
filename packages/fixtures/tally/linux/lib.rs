//! @fixture/tally on the Linux and Windows hosts: a count kept by the
//! session's instance, which only an implementation that runs can answer.
//! `tallyStatus` reads the platform it was compiled for; `tally` adds to
//! the count, answers it with the call's origin, emits `tallied` and asks
//! the status's readers again.

use exact_linux::package::{json, Context, Json, Package, Reply};

pub struct Tally {
    context: Context,
    count: f64,
    released: u32,
}

impl Package for Tally {
    fn new(context: Context) -> Self {
        context.log("made");
        Tally {
            context,
            count: 0.0,
            released: 0,
        }
    }

    fn later(&mut self, name: &str, args: &Json, _preferred: Option<&Json>, reply: Reply) {
        match name {
            "tallyStatus" => reply.send(json!({
                "available": true,
                "platform": std::env::consts::OS,
                "count": self.count,
                "foreground": self.context.foreground(),
            })),
            "tally" => {
                self.count += args["by"].as_f64().unwrap_or(1.0);
                let origin = self.context.origin();
                reply.send(json!({ "count": self.count, "origin": origin }));
                self.context.emit("tallied", json!([self.count]));
                self.context.changed("tally.tallyStatus");
            }
            _ => reply.error("NotSupportedError", None),
        }
    }

    fn release(&mut self, name: &str, _args: Option<&Json>) {
        self.released += 1;
        self.context.log(format!("release {name} ({})", self.released));
    }

    fn foreground(&mut self, shown: bool) {
        self.context.log(format!("foreground {shown}"));
        self.context.changed("tally.tallyStatus");
    }

    fn destroy(&mut self) {
        self.context.log(format!("destroyed at {}", self.count));
    }
}
