use super::*;

#[test]
fn entry_refresh_answers_without_mutating_the_preview() {
    // Picker roots are process-global; isolate this check from parallel picker tests.
    if std::env::var_os("EXACT_ENTRY_ROOTS_TEST").is_none() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "entry_refresh_answers_without_mutating_the_preview",
                "--test-threads=1",
            ])
            .env("EXACT_ENTRY_ROOTS_TEST", "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    #[derive(Default)]
    struct Timeline(u8, String);
    impl exact_runner::DataSource for Timeline {
        fn app_id(&self) -> &str {
            &self.1
        }
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
    let app_id = format!("org.exact.wp6-refresh-{}", std::process::id());
    let (roots, _) = crate::picker::app_dirs(&app_id).unwrap().unwrap();
    let picked = roots[2].join("picked/survives");
    std::fs::create_dir_all(picked.parent().unwrap()).unwrap();
    std::fs::write(&picked, b"running app pick").unwrap();
    let widget = contract::Widget {
        kind: "test-entry".into(),
        title: None,
        description: None,
        families: vec!["systemSmall".into()],
        watch_families: vec![],
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
            0,
            Timeline(response, app_id.clone()),
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
        let picker_roots = crate::picker::roots();
        let n = bridge.refresh_entry(Timeline(response, app_id.clone()), 1000.0);
        let result: serde_json::Value =
            serde_json::from_slice(bridge.output_bytes(n as usize)).unwrap();
        assert_eq!(crate::picker::roots(), picker_roots);
        assert_eq!(std::fs::read(&picked).unwrap(), b"running app pick");
        if response == 0 {
            assert_eq!(result["entryRefresh"], "timeline");
            assert_eq!(result["timeline"][0]["props"]["title"], "Refreshed train");
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
                1 => {
                    assert_eq!(result["entryRefresh"], "unavailable");
                    assert_eq!(result["reason"], "offline");
                }
                2 => assert_eq!(result["timeline"], serde_json::json!([])),
                4 => assert_eq!(result["entryRefresh"], "pending"),
                _ => assert!(result["error"].as_str().unwrap().contains("conform")),
            }
        }
    }
    std::fs::remove_dir_all(roots[2].parent().unwrap()).unwrap();
}
