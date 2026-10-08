//! Presenter-owned row reuse policy, kept across fresh and prepared boots.
use super::Bridge;
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// Opt into bounded row reuse only when the presenter resets every `renew`
    /// carrier to a fresh logical incarnation. Off by default; shared Apple
    /// presenters continue to mount fresh rows until they support that protocol.
    pub fn set_row_reuse(&mut self, on: bool) {
        self.row_reuse = on;
        if let Some(host) = &mut self.host {
            host.set_row_reuse(on);
        }
        if let Some(prepared) = &mut self.prepared {
            prepared.host.set_row_reuse(on);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::Hooks;
    use exact_plan::Value;
    use exact_runner::{CollectionFeedback, DataError};
    use serde_json::Value as Json;
    struct Rows;
    impl DataSource for Rows {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(
                (0..400).map(|i| Value::Number(i as f64)).collect(),
            ))
        }
    }
    const SOURCE:&str="component App\n  resource rows = rows() as shape list<number>\n  view\n    list virtualized=true height=320 estimated-item-height=64\n      each row in rows key=row\n        text `${row}` height=64\n";
    fn feedback(bridge: &mut Bridge<Rows>, offset: f64) -> Json {
        let c = &bridge.host.as_ref().unwrap().runner().collections()[0];
        let bytes = CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            offset,
            port_cross: 240.,
            port_main: 320.,
            cross: 240.,
            measurements: vec![],
            focus_view: None,
            interaction_view: None,
        }
        .encode()
        .unwrap();
        let len = bridge.input_write(&bytes);
        let len = bridge.collection_feedback(len, 0.);
        let batch: Json = serde_json::from_slice(bridge.output_bytes(len as usize)).unwrap();
        assert!(batch["error"].is_null(), "{batch}");
        batch
    }
    fn renew(batch: &Json) -> bool {
        batch["ops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op["op"] == "renew")
    }
    #[test]
    fn row_reuse_policy_survives_fresh_prepared_committed_and_live_toggles() {
        let mut links = crate::link::Links::ALL;
        links.io = None;
        let mut b = Bridge::with_links(links);
        let bytes = contract::compile(SOURCE).unwrap().encode();
        b.set_row_reuse(true);
        b.boot(&bytes, Rows, Hooks::none(), 240., 320.);
        feedback(&mut b, 0.);
        assert!(renew(&feedback(&mut b, 2000.)));
        let len = b.input_write(&bytes);
        let len = b.prepare_plan(len, Rows, Hooks::none(), 240., 320.);
        let prepared: Json = serde_json::from_slice(b.output_bytes(len as usize)).unwrap();
        assert!(prepared["error"].is_null(), "{prepared}");
        assert!(b.prepared.is_some());
        // Changing policy between prepare/commit must update the candidate too.
        b.set_row_reuse(false);
        b.commit_plan();
        feedback(&mut b, 0.);
        assert!(!renew(&feedback(&mut b, 2000.)));
        b.set_row_reuse(true);
        assert!(renew(&feedback(&mut b, 6000.)));
        b.boot(&bytes, Rows, Hooks::none(), 240., 320.);
        feedback(&mut b, 0.);
        assert!(renew(&feedback(&mut b, 2000.)));
    }
}
