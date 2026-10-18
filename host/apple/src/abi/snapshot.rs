use super::*;
impl<D: DataSource> Bridge<D> {
    /// Unlike a running boot this path never endows I/O, reads secrets, opens
    /// delivery, or installs an executor. The refresh explicitly reads later.
    #[allow(clippy::too_many_arguments)]
    pub fn boot_entry(
        &mut self,
        plan: usize,
        kind: usize,
        props: usize,
        data: D,
        hooks: Hooks,
        width: f32,
        height: f32,
    ) -> u32 {
        let Some(total) = plan.checked_add(kind).and_then(|n| n.checked_add(props)) else {
            return self.refuse("entry input overflow");
        };
        if total > self.input.len() {
            return self.refuse("entry input truncated");
        }
        let bytes = self.input[..total].to_vec();
        let Ok(kind_name) = std::str::from_utf8(&bytes[plan..plan + kind]) else {
            return self.refuse("entry kind is not UTF-8");
        };
        // Snapshot admission has no controls. Stage 2 must install field/button
        // chrome, measurement revisions and control_text::prepare with intents.
        let measurer: Box<dyn TextMeasurer> = match hooks.measure {
            Some(f) => Box::new(
                CallbackMeasurer::new(f, hooks.ctx, hooks.lines)
                    .with_symbol(hooks.symbol)
                    .with_time_format(hooks.time_format),
            ),
            None => Box::new(MonospaceMeasurer::default()),
        };
        let launch = self.launch_facts.clone();
        let fonts = self.fonts;
        let fonts_ctx = self.fonts_ctx;
        let result = exact_runner::time::launching_with(launch, || {
            Host::boot_stored_after_decode(
                PlanBytes::Entry(
                    &bytes[..plan],
                    kind_name,
                    (props > 0).then_some(&bytes[plan + kind..]),
                ),
                data,
                measurer,
                self.launch_layout,
                self.boot_viewport(width, height),
                None,
                Vec::new(),
                None,
                None,
                None,
                None,
                "/",
                false,
                None,
                self.links.clone(),
                self.fonts.map(|f| (f, self.fonts_ctx)),
                move |runner| {
                    if let Some(callback) = fonts {
                        let stacks = runner.runtime_font_stacks();
                        install_fonts(runner.plan(), &stacks, callback, fonts_ctx);
                    }
                    Ok(())
                },
            )
        });
        match result {
            Ok((host, batch)) => {
                self.executor = None;
                let batch = Host::ending(self.host.replace(host), batch);
                self.parked.clear();
                self.emit(batch)
            }
            Err(e) => self.refuse(&format!("{e:?}")),
        }
    }
    /// Query the declared refresh source in the already booted entry.
    pub fn entry_timeline(&mut self, now: f64) -> u32 {
        self.entry_timeline_module(now, 0, 0)
    }
    /// The pinned generation's receipt and HBC, concatenated in the input.
    pub fn entry_timeline_module(&mut self, now: f64, receipt: usize, module: usize) -> u32 {
        if receipt > 1 << 20
            || module > 32 << 20
            || receipt + module > self.input.len()
            || (receipt == 0) != (module == 0)
        {
            return self
                .emit(serde_json::json!({"error":"invalid snapshot module lengths"}).to_string());
        }
        let bytes = self.input[..receipt + module].to_vec();
        let Ok(text) = std::str::from_utf8(&bytes[..receipt]) else {
            return self
                .emit(serde_json::json!({"error":"invalid snapshot module receipt"}).to_string());
        };
        let result = self
            .host
            .as_mut()
            .ok_or_else(|| "not booted".to_owned())
            .and_then(|host| {
                host.entry_timeline(
                    now,
                    self.links.io,
                    (receipt > 0).then(|| (text, bytes[receipt..].to_vec())),
                )
            });
        self.emit(match result {
            Ok(json) => json,
            Err(e) => serde_json::json!({"error":e}).to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::{Answer, DataError, Store, Value};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Data {
        asks: Arc<AtomicUsize>,
        writes: bool,
        unlinked: bool,
    }
    impl DataSource for Data {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
        fn grants(&self) -> &str {
            "secret.keep test"
        }
        fn answer(
            &mut self,
            store: &mut Store,
            name: &str,
            args: &[Value],
        ) -> Result<Answer, DataError> {
            assert_eq!(name, "refresh");
            self.asks.fetch_add(1, Ordering::Relaxed);
            if self.unlinked {
                assert!(store.get("test").is_none());
                assert!(store.set("test", "refused").is_err());
            }
            if self.writes {
                store.set("test", "never persisted").unwrap();
            }
            Ok(Answer::Now(Value::list(vec![Value::record(vec![
                args[0].clone(),
                Value::record(vec![Value::str("fresh")]),
            ])])))
        }
    }
    fn entry() -> Vec<u8> {
        contract::snapshot::compile("shape Props\n  title: string\ncomponent Card\n  props\n    title: string\n  view\n    text title\n", &contract::Widget {
            kind: "card".into(), title: None, description: None, families: vec!["systemSmall".into()], component: "Card".into(), props: "Props".into(), refresh: "refresh".into(), preview: serde_json::json!({"title":"preview"})
        }).unwrap().encode()
    }
    #[test]
    fn refresh_without_io_links_cannot_endow_or_validate_store_grants() {
        let entry = entry();
        let asks = Arc::new(AtomicUsize::new(0));
        let mut bridge = Bridge::with_links(crate::link::Links::of(crate::link::Uses::NONE));
        let mut bytes = entry.clone();
        bytes.extend_from_slice(b"card");
        bridge.input_write(&bytes);
        bridge.boot_entry(
            entry.len(),
            4,
            0,
            Data {
                asks: asks.clone(),
                writes: false,
                unlinked: true,
            },
            Hooks::none(),
            160.0,
            160.0,
        );
        bridge.entry_timeline(1234.0);
        assert_eq!(asks.load(Ordering::Relaxed), 1);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bridge.output).unwrap(),
            serde_json::json!([{"date":1234,"props":{"title":"fresh"}}])
        );
    }
    #[test]
    fn entry_boot_is_pure_and_typed_refresh_never_writes() {
        let entry = entry();
        let asks = Arc::new(AtomicUsize::new(0));
        let mut bridge = Bridge::new();
        for (props, expected) in [("", "preview"), ("{\"title\":\"supplied\"}", "supplied")] {
            let mut bytes = entry.clone();
            bytes.extend_from_slice(b"card");
            bytes.extend_from_slice(props.as_bytes());
            bridge.input_write(&bytes);
            bridge.boot_entry(
                entry.len(),
                4,
                props.len(),
                Data {
                    asks: asks.clone(),
                    writes: false,
                    unlinked: false,
                },
                Hooks::none(),
                160.0,
                160.0,
            );
            assert!(String::from_utf8_lossy(&bridge.output).contains(expected));
            assert_eq!(asks.load(Ordering::Relaxed), 0);
        }
        bridge.entry_timeline(1234.0);
        let timeline: serde_json::Value = serde_json::from_slice(&bridge.output).unwrap();
        assert_eq!(
            timeline,
            serde_json::json!([{"date":1234,"props":{"title":"fresh"}}])
        );
        assert_eq!(asks.load(Ordering::Relaxed), 1);
        bridge.host.as_mut().unwrap().runner_mut().data().writes = true;
        bridge.entry_timeline(1234.0);
        assert!(String::from_utf8_lossy(&bridge.output).contains("cannot write"));
        let mut bytes = entry.clone();
        bytes.extend_from_slice(b"card{\"title\":3}");
        bridge.input_write(&bytes);
        bridge.boot_entry(
            entry.len(),
            4,
            11,
            Data {
                asks: asks.clone(),
                writes: false,
                unlinked: false,
            },
            Hooks::none(),
            160.0,
            160.0,
        );
        assert!(String::from_utf8_lossy(&bridge.output).contains("declared shape"));
        bridge.boot(
            &entry,
            Data {
                asks,
                writes: false,
                unlinked: false,
            },
            Hooks::none(),
            160.0,
            160.0,
        );
        assert!(String::from_utf8_lossy(&bridge.output).contains("error"));
    }
}
