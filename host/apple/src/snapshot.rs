//! Typed props replace only the snapshot entry's root slot (LLP 1126 D2).
use exact_plan::{Plan, TypeKind, TypesId, Value};
use exact_runner::Carried;

pub(crate) fn props(plan: &Plan, bytes: &[u8]) -> Result<Carried, String> {
    let row = plan
        .slots
        .iter()
        .find(|s| {
            (plan.str(s.name)
                == if plan.activity_slots.is_empty() {
                    "snapshot_props"
                } else {
                    "snapshot_state"
                })
                && s.owner.is_none()
        })
        .ok_or("entry has no props slot")?;
    let json = serde_json::from_slice(bytes).map_err(|e| format!("entry props: {e}"))?;
    let value =
        decode(plan, row.ty, &json).ok_or("entry props do not match their declared shape")?;
    Ok(Carried {
        slots: vec![(plan.str(row.name).into(), value)],
        ..Carried::default()
    })
}
fn decode(plan: &Plan, ty: TypesId, json: &serde_json::Value) -> Option<Value> {
    let row = plan.type_(ty);
    let value = match row.kind {
        TypeKind::Number => Value::Number(json.as_f64().filter(|n| n.is_finite())?),
        TypeKind::Bool => Value::Bool(json.as_bool()?),
        TypeKind::String => Value::str(json.as_str()?),
        TypeKind::Unit if json.is_null() => Value::Unit,
        TypeKind::Option => Value::Option(if json.is_null() {
            None
        } else {
            Some(std::rc::Rc::new(decode(plan, row.elem?, json)?))
        }),
        TypeKind::List => Value::list(
            json.as_array()?
                .iter()
                .map(|v| decode(plan, row.elem?, v))
                .collect::<Option<Vec<_>>>()?,
        ),
        TypeKind::Record => {
            let fields = json.as_object()?;
            if fields.len() != row.fields.len as usize {
                return None;
            }
            Value::record(
                row.fields
                    .iter()
                    .map(|id| {
                        let field = plan.field(id);
                        decode(plan, field.ty, fields.get(plan.str(field.name))?)
                    })
                    .collect::<Option<Vec<_>>>()?,
            )
        }
        _ => return None,
    };
    value.conforms(plan, ty).then_some(value)
}

impl<D: exact_runner::DataSource> crate::Host<D> {
    pub(crate) fn entry_timeline(
        &mut self,
        now: f64,
        io: Option<crate::link::IoLinks>,
        module: Option<(&str, &[u8])>,
        deadline: std::time::Instant,
    ) -> Result<String, String> {
        use std::time::Instant;
        if self.runner.plan().snapshot_entry.is_empty() {
            return Err("not a snapshot entry".into());
        }
        if Instant::now() >= deadline {
            return Err("widget refresh timed out".into());
        }
        let plan = self.runner.plan().encode();
        let kind = self.runner.plan().snapshot_entry.clone();
        let data = self.runner.data();
        data.prepare_snapshot(
            module.map(|(receipt, bytecode)| exact_runner::SnapshotModule {
                receipt,
                bytecode,
                plan: &plan,
                kind: &kind,
            }),
            deadline,
        )
        .map_err(|e| format!("snapshot module: {e:?}"))?;
        let app = data.app_id().to_owned();
        let grants = data.grants().to_owned();
        let roots = crate::picker::app_dirs(&app)
            .map_err(|e| format!("snapshot storage: {e:?}"))?
            .map(|(roots, _)| roots);
        if let Some([data_root, cache, temporary]) = roots.clone() {
            data.configure_storage(data_root, cache, temporary)
                .map_err(|e| format!("snapshot storage: {e:?}"))?;
        }
        let interrupt = data.interrupt();
        let (cancel, wait) = std::sync::mpsc::channel();
        let watchdog = interrupt.map(|interrupt| {
            std::thread::spawn(move || {
                if wait
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .is_err()
                {
                    interrupt.trigger();
                }
            })
        });
        let result = (|| {
            let endowed = io
                .map(|io| (io.endow)(&grants, &app, false))
                .unwrap_or_else(crate::store::Endowed::none);
            if let Some(error) = endowed.unbound {
                return Err(format!("{error}; every request is refused"));
            }
            let snapshot = io
                .map(|io| (io.snapshot)(endowed.bindings.as_ref(), &app))
                .unwrap_or_default();
            let wake = Box::new(std::thread::current());
            #[allow(unsafe_code)]
            extern "C" fn notify(ctx: *mut std::ffi::c_void) {
                // Executor retirement fences callbacks before the box is freed.
                unsafe { &*(ctx as *const std::thread::Thread) }.unpark();
            }
            let mut executor = None;
            let mut bindings = endowed.bindings;
            let result =
                self.runner
                    .snapshot_timeline(now, snapshot, deadline, |mut request, work| {
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() {
                            return Err("widget refresh timed out".into());
                        }
                        let io = io.ok_or("this build links no I/O; every request is refused")?;
                        let executor = executor.get_or_insert_with(|| {
                            let executor = (io.start)(
                                bindings.take(),
                                &grants,
                                Some((
                                    notify,
                                    (&*wake as *const std::thread::Thread).cast_mut().cast(),
                                )),
                            );
                            if let Some(roots) = roots.clone() {
                                executor.set_app_roots(roots);
                            }
                            executor
                        });
                        if request.continuation.is_none() {
                            let ms = remaining.as_millis().max(1).min(u32::MAX as u128) as u32;
                            request.timeout_ms =
                                Some(request.timeout_ms.map_or(ms, |own| own.min(ms)));
                        }
                        executor
                            .run(
                                exact_runner::RequestOut {
                                    ticket: 1,
                                    target: "widget refresh".into(),
                                    request,
                                    forced: false,
                                },
                                work,
                            )
                            .map_err(str::to_owned)?;
                        loop {
                            executor.begin_pump();
                            if let Some((_, outcome, _)) = executor.drain().into_iter().next() {
                                return Ok(outcome);
                            }
                            let remaining = deadline.saturating_duration_since(Instant::now());
                            if remaining.is_zero() {
                                executor.forget(&|_| false);
                                return Err("widget refresh timed out".into());
                            }
                            std::thread::park_timeout(remaining);
                        }
                    });
            drop(executor);
            drop(wake);
            result
        })();
        let _ = cancel.send(());
        if let Some(watchdog) = watchdog {
            let _ = watchdog.join();
        }
        result
    }
}
