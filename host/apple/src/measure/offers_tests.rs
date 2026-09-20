//! Bounded-offer memo: actual callback answers, real Host layout, and storage.
use super::*;
use crate::measure::{CMetrics, CRequest, CallbackMeasurer};
use exact_kernel::{
    Kernel, MonospaceMeasurer, NodeType, Op, PropId, TextMeasureRequest, TextMeasurer,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

fn fixture() -> Kernel {
    let mut k = Kernel::new(Box::new(MonospaceMeasurer::default()));
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::Text,
            },
            Op::SetProp {
                id: 1,
                prop: PropId::Text,
                value: "Ée\u{301} 🧪 العربية".into(),
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k
}

fn metrics(w: f32, h: f32) -> CMetrics {
    // Deterministic full-tuple oracle, sensitive to BOTH exact bit patterns.
    CMetrics {
        width: 10.0 + (w.to_bits() % 97) as f32 / 8.0,
        height: 20.0 + (h.to_bits() % 53) as f32 / 8.0,
        baseline: 9.0 + ((w.to_bits() ^ h.to_bits()) % 23) as f32 / 8.0,
    }
}

#[derive(Default)]
struct Foreign {
    calls: usize,
}

#[allow(unsafe_code)] // Borrowed synchronous C callback, no retained request/source pointers.
extern "C" fn foreign(ctx: *mut c_void, req: *const CRequest) -> CMetrics {
    let state = unsafe { &*ctx.cast::<RefCell<Foreign>>() };
    let req = unsafe { &*req };
    state.borrow_mut().calls += 1;
    metrics(req.width, req.height)
}

fn callback(state: &RefCell<Foreign>) -> CallbackMeasurer {
    CallbackMeasurer::new(foreign, std::ptr::from_ref(state).cast_mut().cast())
}

fn measure(m: &mut CallbackMeasurer, k: &Kernel, w: AxisOffer, h: AxisOffer) -> TextMetrics {
    let node = k.node(1).unwrap();
    let runs = node.text_runs();
    let req = TextMeasureRequest {
        exclusions: &[],
        runs: &runs,
        paragraph: exact_kernel::text::Paragraph::from_style(node.style),
        width: w,
        height: h,
    };
    let expected = super::super::sanitize(metrics(super::super::offer(w), super::super::offer(h)));
    let actual = m.measure_identified(&node.paragraph_stamp().unwrap(), &req);
    assert_eq!(actual, expected, "full tuple differs for {w:?}/{h:?}");
    actual
}

#[test]
fn all_offers_recur_without_foreign_entry_then_fifo_admission_evicts() {
    let k = fixture();
    let state = RefCell::new(Foreign::default());
    let mut m = callback(&state);
    for w in 100..100 + OFFERS {
        measure(
            &mut m,
            &k,
            AxisOffer::Definite(w as f32),
            AxisOffer::MaxContent,
        );
    }
    assert_eq!(state.borrow().calls, OFFERS);
    for w in 100..100 + OFFERS {
        measure(
            &mut m,
            &k,
            AxisOffer::Definite(w as f32),
            AxisOffer::MaxContent,
        );
    }
    assert_eq!(
        state.borrow().calls,
        OFFERS,
        "all exact tuples must remain resident"
    );
    // A hit on oldest must NOT refresh insertion order.
    measure(
        &mut m,
        &k,
        AxisOffer::Definite(100.0),
        AxisOffer::MaxContent,
    );
    measure(
        &mut m,
        &k,
        AxisOffer::Definite((100 + OFFERS) as f32),
        AxisOffer::MaxContent,
    );
    measure(
        &mut m,
        &k,
        AxisOffer::Definite(101.0),
        AxisOffer::MaxContent,
    );
    assert_eq!(state.borrow().calls, OFFERS + 1);
    measure(
        &mut m,
        &k,
        AxisOffer::Definite(100.0),
        AxisOffer::MaxContent,
    );
    assert_eq!(
        state.borrow().calls,
        OFFERS + 2,
        "overflow admission evicts oldest despite its hit"
    );
}

#[test]
fn eight_slots_keep_full_axis_bits_intrinsics_zero_sign_and_nan_payloads() {
    let k = fixture();
    let state = RefCell::new(Foreign::default());
    let mut m = callback(&state);
    let d = AxisOffer::Definite;
    let offers = [
        (d(10.0), d(20.0)),
        (d(f32::from_bits(10.0f32.to_bits() + 1)), d(20.0)),
        (d(10.0), d(f32::from_bits(20.0f32.to_bits() + 1))),
        (AxisOffer::MinContent, AxisOffer::MaxContent),
        (AxisOffer::MaxContent, AxisOffer::MinContent),
        (d(0.0), d(0.0)),
        (d(-0.0), d(0.0)),
        (d(f32::from_bits(0x7fc00001)), d(20.0)),
    ];
    for _ in 0..2 {
        for (w, h) in offers {
            measure(&mut m, &k, w, h);
        }
    }
    assert_eq!(state.borrow().calls, 8);
    // This is an internal memo key test, NOT admission of NaN layout viewports.
    measure(&mut m, &k, d(f32::from_bits(0x7fc00002)), d(20.0));
    assert_eq!(state.borrow().calls, 9);
}

#[test]
fn catalog_replacement_is_cold_and_retained_stamp_does_not_keep_kernel_alive() {
    struct Lifetime(Rc<()>);
    impl TextMeasurer for Lifetime {
        fn measure(&mut self, _: &TextMeasureRequest<'_>) -> TextMetrics {
            assert_eq!(Rc::strong_count(&self.0), 1);
            TextMetrics::default()
        }
    }
    let owner = Rc::new(());
    let weak = Rc::downgrade(&owner);
    let mut k = Kernel::new(Box::new(Lifetime(owner)));
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::Text,
            },
            Op::SetProp {
                id: 1,
                prop: PropId::Text,
                value: "source".repeat(1000).into(),
            },
        ],
    )
    .unwrap();
    let state = RefCell::new(Foreign::default());
    let mut old = callback(&state);
    for w in 100..108 {
        measure(
            &mut old,
            &k,
            AxisOffer::Definite(w as f32),
            AxisOffer::MaxContent,
        );
    }
    let mut new_catalog = callback(&state);
    measure(
        &mut new_catalog,
        &k,
        AxisOffer::Definite(107.0),
        AxisOffer::MaxContent,
    );
    assert_eq!(
        state.borrow().calls,
        9,
        "new catalog cannot reuse old metrics"
    );
    drop(k);
    assert!(
        weak.upgrade().is_none(),
        "memo must not retain kernel/font owner"
    );
    assert_eq!(old.memo.counts(), (1, 1, 8));
    assert_eq!(new_catalog.memo.counts(), (1, 1, 1));
}

#[derive(Default)]
struct Answers(Vec<(u32, u32, TextMetrics)>);
struct Recorded {
    callback: CallbackMeasurer,
    answers: Rc<RefCell<Answers>>,
    memoize: bool,
}
impl TextMeasurer for Recorded {
    fn measure(&mut self, req: &TextMeasureRequest<'_>) -> TextMetrics {
        self.callback.measure(req)
    }
    fn measure_identified(
        &mut self,
        stamp: &ParagraphStamp,
        req: &TextMeasureRequest<'_>,
    ) -> TextMetrics {
        let value = if self.memoize {
            self.callback.measure_identified(stamp, req)
        } else {
            self.callback.measure(req)
        };
        self.answers.borrow_mut().0.push((
            super::super::offer(req.width).to_bits(),
            super::super::offer(req.height).to_bits(),
            value,
        ));
        value
    }
}

#[test]
fn real_host_revisited_widths_avoid_callbacks_and_match_uncached_layout_and_baselines() {
    struct Empty;
    impl exact_runner::DataSource for Empty {
        fn query(
            &mut self,
            name: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            panic!("unexpected data query {name}")
        }
    }
    let plan = contract::bake(
        contract::compile(
            r#"component App
  view
    row align-items="baseline" testId="root"
      text "Éé 🧪 العربية" testId="text" flex=1
      text "peer" testId="peer" font-size=23
"#,
        )
        .unwrap(),
        Empty,
    )
    .unwrap()
    .encode();
    let foreign_cached = RefCell::new(Foreign::default());
    let foreign_reference = RefCell::new(Foreign::default());
    let cached_answers = Rc::new(RefCell::new(Answers::default()));
    let reference_answers = Rc::new(RefCell::new(Answers::default()));
    let (mut cached, _) = crate::Host::boot(
        &plan,
        Empty,
        Box::new(Recorded {
            callback: callback(&foreign_cached),
            answers: cached_answers.clone(),
            memoize: true,
        }),
        600.0,
        300.0,
    )
    .unwrap();
    let (mut reference, _) = crate::Host::boot(
        &plan,
        Empty,
        Box::new(Recorded {
            callback: callback(&foreign_reference),
            answers: reference_answers.clone(),
            memoize: false,
        }),
        600.0,
        300.0,
    )
    .unwrap();
    let mut revisits = Vec::new();
    for width in [610.0, 620.0, 600.0, 610.0, 620.0] {
        let before = foreign_cached.borrow().calls;
        let a = cached.resize(width, 300.0);
        let b = reference.resize(width, 300.0);
        assert_eq!(a, b, "entire returned layout batches must match");
        assert!(a.contains("\"error\":null"));
        for name in ["root", "text", "peer"] {
            let ak = cached.runner().kernel();
            let bk = reference.runner().kernel();
            let aid = ak.find_by_test_id(name)[0];
            let bid = bk.find_by_test_id(name)[0];
            assert_eq!(
                ak.node_by_key(aid).unwrap().frame,
                bk.node_by_key(bid).unwrap().frame
            );
        }
        revisits.push(foreign_cached.borrow().calls - before);
        assert_eq!(
            cached_answers.borrow().0,
            reference_answers.borrow().0,
            "every actual kernel request/answer/baseline must match"
        );
    }
    eprintln!(
        "realHost callback total={} uncached={} per_resize={revisits:?} answers={}",
        foreign_cached.borrow().calls,
        foreign_reference.borrow().calls,
        cached_answers.borrow().0.len()
    );
    assert_eq!(
        &revisits[2..],
        &[0, 0, 0],
        "three visited widths should fit with intrinsic offers in eight slots"
    );
}

// Test-binary allocator accounting, enabled only on the storage test's thread.
// Production has no allocator hook or storage instrumentation.
#[allow(unsafe_code)]
mod storage {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    #[derive(Clone, Copy, Default)]
    struct Counts {
        active: bool,
        live: usize,
        max_allocation: usize,
        allocations: usize,
        frees: usize,
    }
    thread_local! { static TRACK: Cell<Counts> = const { Cell::new(Counts {
        active: false, live: 0, max_allocation: 0, allocations: 0, frees: 0,
    }) }; }
    struct Allocator;
    #[global_allocator]
    static ALLOCATOR: Allocator = Allocator;
    unsafe impl GlobalAlloc for Allocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let p = unsafe { System.alloc(layout) };
            if !p.is_null() {
                let _ = TRACK.try_with(|s| {
                    let mut c = s.get();
                    if c.active {
                        c.live += layout.size();
                        c.max_allocation = c.max_allocation.max(layout.size());
                        c.allocations += 1;
                        s.set(c);
                    }
                });
            }
            p
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            let _ = TRACK.try_with(|s| {
                let mut c = s.get();
                if c.active {
                    c.live -= layout.size();
                    c.frees += 1;
                    s.set(c);
                }
            });
            unsafe { System.dealloc(ptr, layout) };
        }
    }

    struct RequestProbe {
        expected: CRequest,
        calls: usize,
        exact: bool,
    }

    fn same_run(actual: &crate::measure::CRun, expected: &crate::measure::CRun) -> bool {
        actual.text == expected.text
            && actual.len == expected.len
            && actual.font_size.to_bits() == expected.font_size.to_bits()
            && actual.font_weight == expected.font_weight
            && actual.font_family == expected.font_family
            && actual.italic == expected.italic
            && actual.has_line_height == expected.has_line_height
            && actual.line_height.to_bits() == expected.line_height.to_bits()
            && actual.letter_spacing.to_bits() == expected.letter_spacing.to_bits()
    }

    extern "C" fn inspect_request(ctx: *mut c_void, request: *const CRequest) -> CMetrics {
        // Caller-owned inputs and probe remain alive for this synchronous call.
        // Record scalars only: never retain the actual request or run pointers.
        let probe = unsafe { &mut *ctx.cast::<RequestProbe>() };
        let actual = unsafe { &*request };
        let expected = &probe.expected;
        probe.calls += 1;
        probe.exact &= actual.count == expected.count
            && actual.width.to_bits() == expected.width.to_bits()
            && actual.height.to_bits() == expected.height.to_bits()
            && actual.align == expected.align
            && actual.line_clamp == expected.line_clamp
            && actual.overflow_wrap == expected.overflow_wrap
            && same_run(&actual.strut, &expected.strut)
            && !actual.runs.is_null();
        if probe.exact {
            let actual = unsafe { std::slice::from_raw_parts(actual.runs, actual.count) };
            let expected = unsafe { std::slice::from_raw_parts(expected.runs, expected.count) };
            for (a, b) in actual.iter().zip(expected) {
                probe.exact &= same_run(a, b);
                if a.text == b.text && a.len == b.len {
                    let a = unsafe { std::slice::from_raw_parts(a.text, a.len) };
                    let b = unsafe { std::slice::from_raw_parts(b.text, b.len) };
                    probe.exact &= a == b;
                }
            }
        }
        CMetrics {
            width: 73.125,
            height: 17.5,
            baseline: 9.25,
        }
    }

    fn request_storage(count: usize, variant: usize) -> Counts {
        use crate::measure::CRun;
        use exact_kernel::{FontStyle, OverflowWrap, StyleProps, TextAlign, TextRun, TextStyle};

        // No input construction or assertion formatting occurs while tracking.
        let source = String::from("Ée\u{301} 👩‍🔬 العربية\0尾");
        let heights = [
            None,
            Some(-0.0),
            Some(22.75),
            Some(f32::from_bits(0x7fc00023)),
        ];
        let sizes = [14.25, -0.0, 31.5, f32::INFINITY];
        let style = TextStyle {
            font_size: sizes[variant],
            font_weight: 537,
            font_style: FontStyle::Italic,
            font_family: 23,
            line_height: heights[variant],
            letter_spacing: -0.125,
            font_variant_numeric: 0,
        };
        let strut = TextStyle {
            font_size: 19.5,
            font_weight: 411,
            font_style: FontStyle::Normal,
            font_family: 7,
            line_height: Some(30.25),
            letter_spacing: -0.0,
            font_variant_numeric: 0,
        };
        let runs = [
            TextRun {
                text: &source,
                style,
            },
            TextRun {
                text: "",
                style: strut,
            },
        ];
        let expected_runs = [
            CRun {
                text: source.as_ptr(),
                len: source.len(),
                font_size: sizes[variant],
                font_weight: 537,
                font_family: 23,
                italic: 1,
                has_line_height: u8::from(variant != 0),
                line_height: heights[variant].unwrap_or(0.0),
                letter_spacing: -0.125,
            },
            CRun {
                text: "".as_ptr(),
                len: 0,
                font_size: 19.5,
                font_weight: 411,
                font_family: 7,
                italic: 0,
                has_line_height: 1,
                line_height: 30.25,
                letter_spacing: -0.0,
            },
        ];
        let widths = [
            AxisOffer::Definite(-0.0),
            AxisOffer::MinContent,
            AxisOffer::MaxContent,
            AxisOffer::Definite(127.25),
        ];
        let heights = [
            AxisOffer::MaxContent,
            AxisOffer::Definite(19.75),
            AxisOffer::MinContent,
            AxisOffer::Definite(-0.0),
        ];
        let mut paragraph = exact_kernel::text::Paragraph::from_style(&StyleProps::default());
        paragraph.strut = strut;
        paragraph.text_align = [
            TextAlign::Left,
            TextAlign::Center,
            TextAlign::Right,
            TextAlign::Justify,
        ][variant];
        paragraph.line_clamp = 3;
        paragraph.overflow_wrap = OverflowWrap::Anywhere;
        let request = TextMeasureRequest {
            exclusions: &[],
            runs: &runs[..count],
            paragraph,
            width: widths[variant],
            height: heights[variant],
        };
        let mut probe = RequestProbe {
            expected: CRequest {
                runs: expected_runs.as_ptr(),
                count,
                strut: expected_runs[1],
                width: [-0.0, -2.0, -1.0, 127.25][variant],
                height: [-1.0, 19.75, -2.0, -0.0][variant],
                align: variant as u8,
                line_clamp: 3,
                overflow_wrap: 2,
                white_space: 0,
                direction: 0,
                exclusions: std::ptr::null(),
                exclusion_count: 0,
            },
            calls: 0,
            exact: true,
        };
        let mut m = CallbackMeasurer::new(inspect_request, std::ptr::from_mut(&mut probe).cast());
        TRACK.with(|s| {
            s.set(Counts {
                active: true,
                ..Counts::default()
            })
        });
        let metrics = m.measure(&request);
        let counted = TRACK.with(|s| {
            let c = s.get();
            s.set(Counts::default());
            c
        });
        drop(m);
        assert_eq!(probe.calls, 1);
        assert!(
            probe.exact,
            "every C field and original Unicode pointer must match"
        );
        assert_eq!(
            metrics,
            TextMetrics {
                width: 73.125,
                height: 17.5,
                first_baseline: Some(9.25),
            }
        );
        assert_eq!(
            counted.live, 0,
            "request storage must end with the foreign call"
        );
        assert_eq!(counted.allocations, counted.frees);
        eprintln!(
            "request runs={count} variant={variant} slot={} allocations={} frees={} largest_requested={} final_live={} exact=true calls=1",
            size_of::<CRun>(),
            counted.allocations,
            counted.frees,
            counted.max_allocation,
            counted.live
        );
        counted
    }

    #[test]
    fn single_run_callback_uses_no_heap_request_storage() {
        let counted = request_storage(1, 0);
        assert_eq!(
            counted.allocations, 0,
            "single borrowed run needs no heap request array"
        );
    }

    #[test]
    fn callback_fields_and_unicode_lifetimes_match_for_empty_single_and_multiple_runs() {
        for count in 0..=2 {
            for variant in 0..4 {
                let counted = request_storage(count, variant);
                if count == 0 {
                    assert_eq!(counted.allocations, 0);
                }
                if count == 2 {
                    assert_eq!(
                        counted.allocations, 1,
                        "existing multi-run Vec path retained"
                    );
                    assert_eq!(
                        counted.max_allocation,
                        2 * size_of::<crate::measure::CRun>()
                    );
                }
            }
        }
    }

    #[test]
    fn occupied_payload_cold_owner_and_actual_hash_table_allocation_are_separate() {
        let mut k = fixture();
        for id in 2..=256 {
            k.apply(
                0,
                0,
                &[Op::CreateView {
                    id,
                    node_type: NodeType::Text,
                }],
            )
            .unwrap();
        }
        let stamps: Vec<_> = (1..=256)
            .map(|id| k.node(id).unwrap().paragraph_stamp().unwrap())
            .collect();
        // Initialize RandomState/TLS before counting only Memo-owned allocations.
        drop(Memo::default());
        TRACK.with(|s| {
            s.set(Counts {
                active: true,
                ..Counts::default()
            })
        });
        let mut memo = Memo::default();
        memo.put(
            &stamps[0],
            AxisOffer::MinContent,
            AxisOffer::MaxContent,
            TextMetrics::default(),
        );
        let cold = TRACK.with(Cell::get);
        for stamp in &stamps[1..] {
            memo.put(
                stamp,
                AxisOffer::MinContent,
                AxisOffer::MaxContent,
                TextMetrics::default(),
            );
        }
        let capacity = memo.owners.capacity();
        let order_capacity = memo.order.capacity();
        let full = TRACK.with(Cell::get);
        // Filling the remaining offers allocates nothing: every cold owner
        // already contains the full array, even with one occupied entry.
        for stamp in &stamps {
            for w in 1..OFFERS {
                memo.put(
                    stamp,
                    AxisOffer::Definite(w as f32),
                    AxisOffer::MaxContent,
                    TextMetrics::default(),
                );
            }
        }
        let filled = TRACK.with(Cell::get);
        let slots = memo.counts().2;
        drop(memo);
        let dropped = TRACK.with(|s| {
            let c = s.get();
            s.set(Counts::default());
            c
        });
        let entry = size_of::<(NodeKey, Owner)>();
        let buckets = (capacity + 1).next_power_of_two();
        let controls_and_padding = full.max_allocation - buckets * entry;
        eprintln!(
            "storage offers={OFFERS} slot={} owner={} map_entry={entry} occupied_256_bytes={} cold_requested_live={} map_capacity={capacity} inferred_buckets={buckets} actual_largest_allocation={} control_and_padding={controls_and_padding} order_capacity={order_capacity} total_requested_live={} all_filled_slots={slots} allocations={} frees={} final_live={}",
            size_of::<Option<(Offers, TextMetrics)>>(),
            size_of::<Owner>(),
            256 * size_of::<Owner>(),
            cold.live,
            full.max_allocation,
            full.live,
            dropped.allocations,
            dropped.frees,
            dropped.live
        );
        assert_eq!(slots, 256 * OFFERS);
        assert_eq!(full.live, filled.live);
        assert_eq!(full.allocations, filled.allocations);
        assert_eq!(
            full.live,
            full.max_allocation + order_capacity * size_of::<NodeKey>()
        );
        assert_eq!(
            capacity, 448,
            "record compiler-specific table capacity explicitly"
        );
        assert_eq!(buckets, 512);
        assert!(controls_and_padding == buckets + 8 || controls_and_padding == buckets + 16);
        assert_eq!(dropped.live, 0);
        assert_eq!(dropped.allocations, dropped.frees);
    }
}
