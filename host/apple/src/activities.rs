//! @ref LLP 1126.001 WP9 — the demand-loaded arm's reports and send replies.
use super::{Batch, Host};
use exact_runner::{DataSource, Outcome, Response, Timed};
impl<D: DataSource> Host<D> {
    /// A source snapshot, or the completion of one ticketed send.
    pub fn activities(&mut self, json: &str) -> String {
        let value: serde_json::Value = match serde_json::from_str(json) {
            Ok(v) => v,
            Err(e) => return self.finish(Batch::new(), Some(e.to_string())),
        };
        if let Some(ticket) = value["ticket"].as_u64() {
            let status = value["status"].as_u64().unwrap_or(500) as u16;
            let body = if status == 200 {
                serde_json::to_vec(&value["body"]).unwrap()
            } else {
                value["error"]
                    .as_str()
                    .unwrap_or("activity failed")
                    .as_bytes()
                    .to_vec()
            };
            return self.fulfill_all(
                vec![(
                    ticket,
                    Outcome::Response(Response {
                        status,
                        headers: vec![],
                        body,
                    }),
                    None,
                )],
                self.now_ms,
            );
        }
        if let Some(error) = value["error"].as_str() {
            self.runner.activities_failed();
            self.log(&format!("activities: {error}"));
            return self.finish(Batch::new(), Some(format!("activities: {error}")));
        }
        match self.runner.set_activities(json) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => self.finish(Batch::new(), None),
            Err(e) => self.finish(Batch::new(), Some(format!("activities: {e:?}"))),
        }
    }
}
