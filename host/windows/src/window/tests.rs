use super::*;
use winit::platform::windows::WindowAttributesExtWindows;

#[test]
fn dirty_redraws_respect_the_present_retry_and_one_paint_per_refresh() {
    let start = Instant::now();
    for hz in [60, 120, 144] {
        let period = Duration::from_secs_f64(1. / hz as f64);
        let next = start + period;
        let mut schedule = FrameSchedule {
            cadence: Some(next),
            retry: None,
            present: None,
        };
        for step in 0..100 {
            let now = start + period * step / 100;
            assert_eq!(schedule.redraw(now, true), Some(next));
            assert_eq!(schedule.admit(now), next);
        }
        // Refusal can arrive after the cadence tick. New dirty events cannot
        // shorten its deadline, and a clean picture must still retry there.
        let refused_until = next + period / 2;
        schedule.present = Some(refused_until);
        schedule.retry = Some(refused_until);
        for dirty in [false, true] {
            assert_eq!(schedule.redraw(next, dirty), Some(refused_until));
            assert_eq!(schedule.redraw(refused_until, dirty), Some(refused_until));
        }
        assert_eq!(schedule.admit(next), refused_until);
        assert_eq!(schedule.admit(refused_until), refused_until);
        schedule.present = None;
        schedule.retry = None;
        assert_eq!(schedule.redraw(refused_until, false), None);
    }
}

#[test]
fn recovery_backoff_keeps_idle_wakes_without_stalling_changed_content() {
    let now = Instant::now();
    let recovery = now + Duration::from_secs(1);
    let mut schedule = FrameSchedule {
        cadence: None,
        retry: Some(recovery),
        present: None,
    };
    assert_eq!(schedule.redraw(now, false), Some(recovery));
    assert_eq!(schedule.redraw(now, true), Some(now));
    assert_eq!(schedule.admit(now), now);
    let tick = now + Duration::from_millis(16);
    schedule.cadence = Some(tick);
    assert_eq!(schedule.redraw(now, true), Some(tick));
    assert_eq!(schedule.redraw(now, false), Some(recovery));
}

#[test]
fn an_early_image_or_window_redraw_survives_until_the_next_tick() {
    let now = Instant::now();
    let tick = now + Duration::from_millis(16);
    let schedule = FrameSchedule {
        cadence: Some(tick),
        retry: None,
        present: None,
    };
    let mut pending = false;
    // Image completion requests a redraw before poll_images marks the picture
    // dirty. Deferring that event must preserve the request, not just damage.
    assert!(schedule.defer(now, &mut pending));
    assert_eq!(schedule.redraw(now, pending), Some(tick));
    assert!(!schedule.defer(tick, &mut pending));
    assert_eq!(schedule.redraw(tick, pending), None);
}
#[test]
fn only_direct_ink_devices_prepare_asynchronously() {
    use exact_linux::presenter::PainterChoice::*;
    assert!(direct_device(Ink, None));
    assert!(!direct_device(Ink, Some("wgpu")));
    assert!(direct_device(Ink, Some("d3d11")));
    assert!(direct_device(Ink, Some("warp")));
    for painter in [Auto, Cpu, Gpu, Custom] {
        assert!(!direct_device(painter, None));
        assert!(!direct_device(painter, Some("d3d11")));
    }
}

#[test]
#[ignore = "native Windows desktop, system caret blink enabled, EXACT_INK_DEVICE=d3d11"]
#[allow(deprecated)]
fn direct_window_blinks_without_a_softbuffer_surface() {
    fn present(p: &mut Presenter<()>, window: &exact_ink::device::d3d11::Window) {
        let limit = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(p.window_frame(window.clone()).unwrap().is_none());
            if p.last_frame_succeeded() && !p.dirty() {
                break;
            }
            assert!(Instant::now() < limit, "caret frame was not presented");
            // A full swap chain defers this frame to the carrier's retry deadline.
            let due = p
                .painter_retry_deadline()
                .expect("deferred present retries");
            std::thread::sleep(due.min(limit).saturating_duration_since(Instant::now()));
        }
    }

    let events = EventLoop::<crate::accessibility::Event>::with_user_event()
        .with_any_thread(true)
        .build()
        .unwrap();
    let window = Rc::new(
        events
            .create_window(
                Window::default_attributes()
                    .with_title("Ink caret regression")
                    .with_no_redirection_bitmap(true)
                    .with_inner_size(LogicalSize::new(160., 80.)),
            )
            .unwrap(),
    );
    let owned = exact_ink::device::d3d11::Window::from_owner(window.clone()).unwrap();
    let plan = contract::compile(
        "component App\n  view\n    input testId=\"field\" value=\"blink\" width=140 height=40\n",
    )
    .unwrap()
    .encode();
    let (mut p, warning) = Presenter::boot_with(
        &plan,
        (),
        (160., 80.),
        1.,
        Default::default(),
        exact_linux::presenter::PainterChoice::Ink,
    )
    .unwrap();
    assert!(warning.is_none());
    assert!(p.attach_painter_window(owned.clone(), [160, 80]).unwrap());
    let k = p.host().kernel();
    let field = k.node_by_key(k.find_by_test_id("field")[0]).unwrap().id;
    p.tap(field).unwrap();
    present(&mut p, &owned);
    let started = Instant::now();
    let mut caret = crate::caret::Caret::default();
    caret.presented(owned.hwnd() as isize, p.caret_frame(), 1., None, started);
    let due = caret.deadline().expect("system caret blink enabled");
    let mut state = App {
        proxy: events.create_proxy(),
        accessibility: None,
        name: "Ink caret regression".into(),
        min_size: (1., 1.),
        config: Config::from_env(&plan, ""),
        window: Some(window),
        surface: None,
        preparing: None,
        jump_list_started: false,
        direct: true,
        presenter: Some(p),
        timers: crate::timers::Timers::new(|_| {}),
        started,
        pointer: (0., 0.),
        pointer_inside: false,
        focused: true,
        cursor: CursorIcon::Default,
        buttons: 0,
        clicks: Default::default(),
        scale: 1.,
        refit_after_scale: false,
        next_frame: started,
        redraw_pending: false,
        failed: false,
        minimized: false,
        occluded: false,
        frames: 1,
        measured_first_screen: false,
        measure_first_composition: false,
        paint_ms: 0.,
        present_ms: 0.,
        previous_frame: None,
        samples: Vec::new(),
        frame_logs: Vec::new(),
        frame_limit: None,
        first_tap: None,
        pads: Default::default(),
        pads_active: false,
        caret,
        edit_menu: None,
        ime: Default::default(),
        appearance: Default::default(),
        modal: Default::default(),
        desktop: crate::desktop::Desktop::install("Ink caret regression"),
    };
    assert!(!state.presenter.as_ref().unwrap().dirty());
    state.blink(due).unwrap();
    assert!(!state.caret.visible());
    let next = state.caret.deadline().unwrap();
    assert!(next > due);
    assert!(state.presenter.as_ref().unwrap().dirty());
    present(state.presenter.as_mut().unwrap(), &owned);
    state.blink(due).unwrap();
    assert_eq!(state.caret.deadline(), Some(next));
    state.blink(next).unwrap();
    assert!(state.caret.visible());
    present(state.presenter.as_mut().unwrap(), &owned);
}
#[test]
fn text_without_a_physical_key_reaches_the_editor() {
    use winit::keyboard::{KeyCode, NamedKey, NativeKey, NativeKeyCode};
    let packet = PhysicalKey::Unidentified(NativeKeyCode::Windows(0));
    for text in ["a", "é", "界", "🦀"] {
        assert_eq!(
            key_names(packet, &Key::Character(text.into()), Some(text)),
            ("Unidentified".into(), text.into())
        );
        assert_eq!(
            key_names(
                packet,
                &Key::Unidentified(NativeKey::Windows(0xe7)),
                Some(text)
            ),
            ("Unidentified".into(), text.into())
        );
    }
    assert_eq!(
        key_names(
            PhysicalKey::Code(KeyCode::Enter),
            &Key::Named(NamedKey::Enter),
            Some("\r")
        ),
        ("Enter".into(), "Enter".into())
    );
    assert_eq!(
        key_names(
            PhysicalKey::Code(KeyCode::SuperLeft),
            &Key::Named(NamedKey::Meta),
            None
        ),
        ("MetaLeft".into(), "Meta".into())
    );
}
#[test]
fn frame_cadence_does_not_accumulate_timer_latency() {
    let start = Instant::now();
    for hz in [60, 120, 144] {
        let period = Duration::from_secs_f64(1.0 / hz as f64);
        let mut deadline = start;
        for frame in 1..=10_000 {
            deadline = next_frame_deadline(deadline, deadline + Duration::from_micros(350), period);
            assert_eq!(deadline, start + period * frame);
        }
    }
}
#[test]
fn early_redraws_preserve_cadence_and_stalls_resume_without_catch_up() {
    let start = Instant::now();
    let period = Duration::from_millis(10);
    let deadline = start + period;
    assert_eq!(next_frame_deadline(deadline, start, period), deadline);
    let late = deadline + Duration::from_secs(5);
    assert_eq!(next_frame_deadline(deadline, late, period), late + period);
    assert_eq!(
        next_frame_deadline(deadline, deadline + period, period),
        deadline + period * 2
    );
}

fn windowless_app(events: &EventLoop<crate::accessibility::Event>, plan: &[u8]) -> App<()> {
    let started = Instant::now();
    let (p, _) = Presenter::boot_with(
        plan,
        (),
        (160., 80.),
        1.,
        Default::default(),
        exact_linux::presenter::PainterChoice::Cpu,
    )
    .unwrap();
    App {
        proxy: events.create_proxy(),
        accessibility: None,
        name: "Frame scheduling regression".into(),
        min_size: (1., 1.),
        config: Config::from_env(&plan, ""),
        window: None,
        surface: None,
        preparing: None,
        jump_list_started: false,
        direct: true,
        presenter: Some(p),
        timers: crate::timers::Timers::new(|_| {}),
        started,
        pointer: (0., 0.),
        pointer_inside: false,
        focused: true,
        cursor: CursorIcon::Default,
        buttons: 0,
        clicks: Default::default(),
        scale: 1.,
        refit_after_scale: false,
        next_frame: started,
        redraw_pending: false,
        failed: false,
        minimized: false,
        occluded: false,
        frames: 1,
        measured_first_screen: false,
        measure_first_composition: false,
        paint_ms: 0.,
        present_ms: 0.,
        previous_frame: None,
        samples: Vec::new(),
        frame_logs: Vec::new(),
        frame_limit: None,
        first_tap: None,
        pads: Default::default(),
        pads_active: false,
        caret: Default::default(),
        edit_menu: None,
        ime: Default::default(),
        appearance: Default::default(),
        modal: Default::default(),
        desktop: crate::desktop::Desktop::install("Frame scheduling regression"),
    }
}

#[allow(deprecated)]
pub(crate) fn about_to_wait_honors_retry_pending_redraw_and_earlier_timer(
    events: EventLoop<crate::accessibility::Event>,
) {
    let plan = contract::compile("component App\n  view\n    text \"idle\"\n")
        .unwrap()
        .encode();
    let mut app = windowless_app(&events, &plan);
    let mut checked = false;
    events.run(|event, events| {
        if !matches!(event, winit::event::Event::AboutToWait) { return; }
        let now = Instant::now();
        let retry = now + Duration::from_secs(1);
        app.presenter.as_mut().unwrap().frame();
        assert!(!app.presenter.as_ref().unwrap().dirty());
        app.wait(events, |_| (Some(retry), Some(retry)));
        assert_eq!(events.control_flow(), ControlFlow::WaitUntil(retry));
        app.redraw_pending = true;
        app.wait(events, |_| (Some(retry), Some(retry)));
        assert_eq!(events.control_flow(), ControlFlow::WaitUntil(retry));
        assert!(app.redraw_pending);
        app.next_frame = now + Duration::from_millis(500);
        app.frame_limit = Some(10);
        app.wait(events, |_| (None, None));
        assert_eq!(events.control_flow(), ControlFlow::WaitUntil(app.next_frame));
        app.frame_limit = None;
        app.redraw_pending = false;
        app.about_to_wait(events);
        assert_eq!(events.control_flow(), ControlFlow::Wait);
        // A runner wake must win over the presenter's later retry.
        let timed = contract::compile("component App\n  state n = 0\n  action bump\n    n = n + 1\n  task timer mount\n    every(500, bump)\n  view\n    text \"idle\"\n").unwrap().encode();
        let (mut p, _) = Presenter::boot_with(&timed, (), (160., 80.), 1., Default::default(),
            exact_linux::presenter::PainterChoice::Cpu).unwrap();
        p.frame();
        let wake = p.wake_due_ms().unwrap();
        app.presenter = Some(p);
        app.wait(events, |_| (Some(retry), Some(retry)));
        assert_eq!(events.control_flow(), ControlFlow::WaitUntil(app.started + Duration::from_secs_f64(wake / 1000.)));
        checked = true;
        events.exit();
    }).unwrap();
    assert!(checked);
}

#[test]
fn newly_drawable_windows_discard_occlusion_deadlines_but_keep_cadence() {
    let now = Instant::now();
    let period = Duration::from_secs_f64(1. / 60.);
    for event in [
        WindowEvent::Occluded(false),
        WindowEvent::Focused(true),
        WindowEvent::Resized(winit::dpi::PhysicalSize::new(800, 800)),
    ] {
        assert!(presentation_resumes(&event));
        let schedule = FrameSchedule {
            cadence: Some(now + period),
            retry: None,
            present: None,
        };
        assert_eq!(schedule.admit(now), now + period);
        assert_eq!(schedule.redraw(now, true), Some(now + period));
    }
    assert!(!presentation_resumes(&WindowEvent::Focused(false)));
    assert!(!presentation_resumes(&WindowEvent::Occluded(true)));
}
