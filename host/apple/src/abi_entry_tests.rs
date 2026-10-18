use super::*;

#[test]
fn entry_refresh_replaces_only_a_typed_first_timeline_value() {
    #[derive(Default)]
    struct Timeline(u8);
    impl exact_runner::DataSource for Timeline {
        fn answer(
            &mut self,
            _: &mut Store,
            source: &str,
            args: &[Value],
        ) -> Result<Answer, DataError> {
            if self.0 == 4 {
                return Ok(Answer::Later(exact_runner::Request::get(
                    "https://example.invalid/timeline",
                )));
            }
            self.query(source, args).map(Answer::Now)
        }
        fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
            assert!(matches!(args, [Value::Number(1000.0)]));
            match self.0 {
                1 => Err(DataError::Unavailable("offline".into())),
                2 => Ok(Value::list(vec![])),
                3 => Ok(Value::str("wrong type")),
                _ => Ok(Value::list(vec![Value::record(vec![
                    Value::Number(1000.0),
                    Value::record(vec![Value::str("Refreshed train")]),
                ])])),
            }
        }
    }
    let widget = contract::Widget {
        kind: "test-entry".into(),
        families: vec!["systemSmall".into()],
        component: "Card".into(),
        props: "Props".into(),
        refresh: "refreshCard".into(),
        preview: serde_json::json!({"title": "Preview train"}),
    };
    let plan = contract::snapshot::compile("shape Props\n  title: string\ncomponent App\n  view\n    text \"App\"\ncomponent Card\n  props\n    title: string\n  view\n    text title\n", &widget).unwrap();
    for response in 0..5 {
        let mut bridge = Bridge::<Timeline>::new();
        let bytes = plan.encode();
        bridge.input_write(&[bytes.as_slice(), b"test-entry"].concat());
        bridge.boot_entry(
            bytes.len(),
            10,
            Timeline(response),
            Hooks::none(),
            158.0,
            158.0,
        );
        let before = bridge
            .host
            .as_ref()
            .unwrap()
            .runner()
            .slot("snapshot_props")
            .cloned();
        let n = bridge.refresh_entry(Timeline(response), Hooks::none(), 1000.0, 158.0, 158.0);
        let result: serde_json::Value =
            serde_json::from_slice(bridge.output_bytes(n as usize)).unwrap();
        if response == 0 {
            assert_eq!(result["entryRefresh"], "timeline");
            assert_eq!(result["timeline"][0]["props"]["title"], "Refreshed train");
            assert_eq!(
                bridge
                    .host
                    .as_ref()
                    .unwrap()
                    .runner()
                    .slot("snapshot_props"),
                Some(&Value::record(vec![Value::str("Refreshed train")]))
            );
        } else {
            assert_eq!(
                bridge
                    .host
                    .as_ref()
                    .unwrap()
                    .runner()
                    .slot("snapshot_props")
                    .cloned(),
                before
            );
            match response {
                1 => assert_eq!(result["entryRefresh"], "unavailable"),
                2 => assert_eq!(result["entryRefresh"], "empty"),
                4 => assert_eq!(result["entryRefresh"], "pending"),
                _ => assert!(result["error"].as_str().unwrap().contains("conform")),
            }
        }
    }
}
