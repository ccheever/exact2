use super::*;
use crate::types::Snapshot;

fn launch() -> RecoveryLaunch {
    RecoveryLaunch {
        claude_remote: false,
        provider: "codex".into(),
        account: "work".into(),
        cwd: "/repo".into(),
        model: "gpt".into(),
        effort: "high".into(),
        permissions: "read-only".into(),
        resume: "conversation".into(),
    }
}

fn no_hint(_: Command) -> String {
    String::new()
}

fn machine_with(sessions: Vec<Session>) -> Machine {
    Machine {
        id: "studio".into(),
        name: "Studio".into(),
        last: Some(Snapshot {
            sessions,
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn chrome<'a>(recovery: &'a Recovery, observed: Option<(&'a Machine, &'a Session)>) -> Chrome<'a> {
    Chrome {
        recovery,
        observed,
        has_session: true,
        remote_machine: false,
        connection: None,
        exited: false,
        exit_code: None,
        resuming: false,
    }
}

fn ids(view: &Json) -> Vec<String> {
    view["overlayButtons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["id"].as_str().unwrap().to_string())
        .collect()
}

// ----- ported from workspace.rs `mod tests` ---------------------------------

#[test]
fn codex_app_server_exit_keeps_terminal_error_visible() {
    assert!(!automatic_terminal_recovery_allowed(None));
    assert!(!automatic_terminal_recovery_allowed(Some(&Session {
        codex_socket: "/private/codex.sock".into(),
        ..Default::default()
    })));
    assert!(automatic_terminal_recovery_allowed(Some(
        &Session::default()
    )));
}

#[test]
fn eas_sessions_are_never_reopened_automatically() {
    let cloud = Session {
        eas: true,
        provider: "codex".into(),
        state: "idle".into(),
        ..Default::default()
    };
    assert!(!automatic_terminal_recovery_allowed(Some(&cloud)));
}

#[test]
fn recovery_launches_never_replay_the_original_prompt() {
    let launch = launch();
    let resume = recovery_launch_args("studio", "request", &launch, LaunchMode::Resume);
    assert!(resume
        .windows(2)
        .any(|pair| pair == ["--resume", "conversation"]));
    assert!(!resume
        .iter()
        .any(|arg| arg == "--fork" || arg == "--prompt"));
    let fork = recovery_launch_args("studio", "request", &launch, LaunchMode::Fork);
    assert!(fork.iter().any(|arg| arg == "--fork"));
    assert!(fork
        .windows(2)
        .any(|pair| pair == ["--resume", "conversation"]));
    let replacement = recovery_launch_args("studio", "request", &launch, LaunchMode::Replace);
    assert!(!replacement
        .iter()
        .any(|arg| arg == "--resume" || arg == "--fork" || arg == "--prompt"));
    let remote = RecoveryLaunch {
        claude_remote: true,
        provider: "claude".into(),
        ..launch
    };
    let resume = recovery_launch_args("studio", "request", &remote, LaunchMode::Resume);
    assert!(resume.iter().any(|arg| arg == "--claude-remote"));
}

#[test]
fn claude_remote_viewer_exit_does_not_reopen_it_automatically() {
    assert!(!automatic_terminal_recovery_allowed(Some(&Session {
        claude_remote: true,
        ..Session::default()
    })));
    assert!(automatic_terminal_recovery_allowed(Some(
        &Session::default()
    )));
}

#[test]
fn automatic_recovery_only_runs_for_a_writable_active_tab_and_backs_off() {
    let now = 1_000_000.0;
    assert!(automatic_recovery_ready(true, false, None, now));
    assert!(!automatic_recovery_ready(false, false, None, now));
    assert!(!automatic_recovery_ready(true, true, None, now));
    assert!(!automatic_recovery_ready(
        true,
        false,
        Some(now - 59_000.0),
        now
    ));
    assert!(automatic_recovery_ready(
        true,
        false,
        Some(now - 60_000.0),
        now
    ));
}

#[test]
fn unreachable_transport_errors_are_distinct_from_uncertain_session_errors() {
    for error in [
        "ssh: connect to host studio port 22: Operation timed out",
        "connect: No route to host",
        "Permission denied (publickey)",
        "Host key verification failed",
    ] {
        assert!(machine_unreachable(error), "{error}");
    }
    assert!(!machine_unreachable(
        "tmux returned an incomplete terminal identity"
    ));
    assert!(!machine_unreachable("process inventory is partial"));
}

// ----- the state machine ----------------------------------------------------

#[test]
fn the_full_argv_orders_permissions_before_attach() {
    let argv = recovery_launch_args("studio", "r1", &launch(), LaunchMode::Fork);
    assert_eq!(
        argv,
        [
            "run",
            "studio",
            "--request-id",
            "r1",
            "--provider",
            "codex",
            "--account",
            "work",
            "--cwd",
            "/repo",
            "--model",
            "gpt",
            "--effort",
            "high",
            "--resume",
            "conversation",
            "--fork",
            "--permissions",
            "read-only",
            "--attach",
        ]
    );
}

#[test]
fn assess_builds_the_argv_and_ignores_a_busy_tab() {
    let mut r = Recovery {
        details: true,
        ..Default::default()
    };
    assert_eq!(
        r.assess("studio", "s1", false).unwrap(),
        ["recovery", "studio", "s1"]
    );
    assert_eq!(r.state, RecoveryState::Checking);
    assert!(!r.details);
    assert_eq!(r.assess("studio", "s1", true), None);
    r.state = RecoveryState::Restoring;
    assert_eq!(r.assess("studio", "s1", true), None);
    r.state = RecoveryState::Unreachable("x".into());
    assert_eq!(
        r.assess("studio", "s1", true).unwrap(),
        ["recovery", "studio", "s1", "--continue"]
    );
}

#[test]
fn replies_become_states() {
    let reply = |status: &str| {
        Ok(json!({ "status": status, "reason": "why", "launch": { "provider": "codex", "resume": "c" } })
            .to_string())
    };
    let mut r = Recovery::default();
    assert_eq!(
        r.set_assessment(reply("terminal-uncertain"), "m", false, false, 0.0),
        Assessed::Shown
    );
    assert_eq!(r.state, RecoveryState::TerminalUncertain("why".into()));
    r.set_assessment(reply("agent-uncertain"), "m", false, false, 0.0);
    assert!(
        matches!(r.state, RecoveryState::AgentUncertain(ref w, ref l) if w == "why" && l.resume == "c")
    );
    r.set_assessment(reply("weird"), "m", false, false, 0.0);
    assert!(matches!(r.state, RecoveryState::AgentUncertain(_, _)));
    r.set_assessment(reply("restorable"), "m", false, false, 0.0);
    assert!(matches!(r.state, RecoveryState::Restorable(_)));
    r.set_assessment(reply("ended"), "m", false, false, 0.0);
    assert!(matches!(r.state, RecoveryState::Ended(_)));
    r.set_assessment(reply("live"), "m", false, false, 0.0);
    assert_eq!(
        r.state,
        RecoveryState::AgentUncertain(
            "Fleet found the agent but not its terminal identity".into(),
            RecoveryLaunch {
                provider: "codex".into(),
                resume: "c".into(),
                ..Default::default()
            }
        )
    );
    r.set_assessment(
        Err("ssh: Connection refused".into()),
        "m",
        false,
        false,
        0.0,
    );
    assert!(matches!(r.state, RecoveryState::Unreachable(_)));
    r.set_assessment(Err("partial inventory".into()), "m", false, false, 0.0);
    assert_eq!(
        r.state,
        RecoveryState::TerminalUncertain("partial inventory".into())
    );
    r.set_assessment(Ok("not json".into()), "m", false, false, 0.0);
    assert!(
        matches!(r.state, RecoveryState::TerminalUncertain(ref e) if e.starts_with("Invalid recovery response: "))
    );
}

#[test]
fn a_live_agent_is_reattached_under_its_new_id() {
    let mut r = Recovery {
        state: RecoveryState::Checking,
        ..Default::default()
    };
    let reply = json!({ "status": "running", "session": { "id": "s2" } }).to_string();
    assert_eq!(
        r.set_assessment(Ok(reply), "studio", true, true, 0.0),
        Assessed::Reattach {
            session: "s2".into(),
            key: "studio:s2:true".into(),
            argv: vec![
                "attach".into(),
                "studio".into(),
                "s2".into(),
                "--read-only".into()
            ],
        }
    );
    assert_eq!(r.state, RecoveryState::None);
    r.attach_failed("spawn failed");
    assert_eq!(
        r.state,
        RecoveryState::TerminalUncertain("spawn failed".into())
    );
}

#[test]
fn restorable_and_ended_relaunch_automatically_once_a_minute() {
    let restorable = json!({ "status": "restorable", "launch": { "resume": "c" } }).to_string();
    let mut r = Recovery::default();
    assert_eq!(
        r.set_assessment(Ok(restorable.clone()), "m", false, true, 5_000.0),
        Assessed::Launch(LaunchMode::Resume)
    );
    assert_eq!(r.automatic_at, Some(5_000.0));
    assert_eq!(
        r.set_assessment(Ok(restorable.clone()), "m", false, true, 30_000.0),
        Assessed::Shown
    );
    assert_eq!(r.automatic_at, Some(5_000.0));
    let ended = json!({ "status": "ended" }).to_string();
    assert_eq!(
        r.set_assessment(Ok(ended.clone()), "m", false, true, 65_000.0),
        Assessed::Launch(LaunchMode::Replace)
    );
    // Background and read-only tabs wait for a press.
    let mut r = Recovery::default();
    assert_eq!(
        r.set_assessment(Ok(restorable), "m", false, false, 0.0),
        Assessed::Shown
    );
    assert_eq!(
        r.set_assessment(Ok(ended), "m", true, true, 0.0),
        Assessed::Shown
    );
    assert_eq!(r.automatic_at, None);
}

#[test]
fn launch_offers_only_what_the_state_allows() {
    let restorable = RecoveryState::Restorable(launch());
    let uncertain = RecoveryState::AgentUncertain("x".into(), launch());
    let ended = RecoveryState::Ended(launch());
    for (state, mode, ok) in [
        (&restorable, LaunchMode::Resume, true),
        (&restorable, LaunchMode::Fork, true),
        (&restorable, LaunchMode::Replace, false),
        (&uncertain, LaunchMode::Resume, false),
        (&uncertain, LaunchMode::Fork, true),
        (&uncertain, LaunchMode::Replace, false),
        (&ended, LaunchMode::Replace, true),
        (&ended, LaunchMode::Resume, false),
        (&RecoveryState::None, LaunchMode::Resume, false),
        (&RecoveryState::Checking, LaunchMode::Replace, false),
    ] {
        let mut r = Recovery {
            state: state.clone(),
            ..Default::default()
        };
        assert_eq!(
            r.launch(mode, "m", "req", false).is_some(),
            ok,
            "{state:?} {mode:?}"
        );
    }
}

#[test]
fn a_relaunch_rekeys_and_rolls_back_on_failure() {
    let mut r = Recovery {
        state: RecoveryState::Restorable(launch()),
        details: true,
        ..Default::default()
    };
    let plan = r
        .launch(LaunchMode::Resume, "studio", "req", false)
        .unwrap();
    assert_eq!(plan.key, "studio:req:false");
    assert_eq!(plan.session, ("studio".into(), "req".into(), false));
    assert_eq!(plan.message, "Restoring conversation…");
    assert_eq!(plan.argv[0], "run");
    assert_eq!(r.state, RecoveryState::Restoring);
    assert_eq!(
        r.launch_failed("boom"),
        "Could not open recovery terminal: boom"
    );
    assert_eq!(r.state, RecoveryState::Restorable(launch()));
    r.launch(LaunchMode::Fork, "studio", "req2", false).unwrap();
    r.launched();
    assert_eq!(r.state, RecoveryState::None);
    assert!(!r.details);
}

#[test]
fn details_toggle() {
    let mut r = Recovery::default();
    r.toggle_details();
    assert!(r.details);
    r.toggle_details();
    assert!(!r.details);
}

#[test]
fn triggers_follow_upstream() {
    let managed = Session {
        managed: true,
        ..Default::default()
    };
    let idle = Recovery::default();
    let checking = Recovery {
        state: RecoveryState::Checking,
        ..Default::default()
    };
    assert!(tab_supports_recovery(Some(&managed)));
    assert!(!tab_supports_recovery(Some(&Session::default())));
    assert!(!tab_supports_recovery(None));
    assert!(should_assess_on_exit(Some(&managed), &idle));
    assert!(!should_assess_on_exit(Some(&managed), &checking));
    let eas = Session {
        eas: true,
        ..managed.clone()
    };
    assert!(!should_assess_on_exit(Some(&eas), &idle));
    assert!(reconnect_assesses(Some(&managed)));
    assert!(!reconnect_assesses(Some(&Session {
        codex_socket: "/s".into(),
        ..managed.clone()
    })));

    let exited = Session {
        id: "s".into(),
        managed: true,
        state: "exited".into(),
        observation_received_at: Some(0.0),
        ..Default::default()
    };
    let m = machine_with(vec![exited.clone()]);
    let tab = ("studio".to_string(), "s".to_string(), false);
    assert!(should_check_active(
        Some(&tab),
        Some((&m, &exited)),
        &idle,
        1_000.0
    ));
    // Stale observation, read-only tab, live pid, historical, failing machine.
    assert!(!should_check_active(
        Some(&tab),
        Some((&m, &exited)),
        &idle,
        31_000.0
    ));
    let ro = ("studio".to_string(), "s".to_string(), true);
    assert!(!should_check_active(
        Some(&ro),
        Some((&m, &exited)),
        &idle,
        1_000.0
    ));
    let alive = Session {
        pid: 4,
        ..exited.clone()
    };
    assert!(!should_check_active(
        Some(&tab),
        Some((&m, &alive)),
        &idle,
        1_000.0
    ));
    let failing = Machine {
        error: "down".into(),
        ..m.clone()
    };
    assert!(!should_check_active(
        Some(&tab),
        Some((&failing, &exited)),
        &idle,
        1_000.0
    ));
    // An unreachable tab retries even while the session looks alive.
    let unreachable = Recovery {
        state: RecoveryState::Unreachable("ssh".into()),
        ..Default::default()
    };
    assert!(should_check_active(
        Some(&tab),
        Some((&m, &alive)),
        &unreachable,
        1_000.0
    ));
}

#[test]
fn unpause_targets_only_a_paused_session_and_resuming_ends_with_it() {
    let paused = Session {
        id: "s".into(),
        state: "paused".into(),
        ..Default::default()
    };
    let m = machine_with(vec![paused.clone()]);
    assert_eq!(
        unpause_argv(Some((&m, &paused))).unwrap(),
        ["unpause", "studio", "s"]
    );
    let running = Session {
        state: "running".into(),
        ..paused.clone()
    };
    assert_eq!(unpause_argv(Some((&m, &running))), Err(NOT_PAUSED));
    assert_eq!(unpause_argv(None), Err(NOT_PAUSED));

    let tab = ("studio".to_string(), "s".to_string(), false);
    assert!(still_resuming(Some(&tab), &m));
    assert!(!still_resuming(Some(&tab), &machine_with(vec![running])));
    let other = ("elsewhere".to_string(), "s".to_string(), false);
    assert!(still_resuming(Some(&other), &machine_with(vec![])));
    assert!(!still_resuming(None, &m));
}

#[test]
fn presses_map_to_commands() {
    assert_eq!(press_command("recovery-retry"), Some(Command::ReconnectTab));
    assert_eq!(
        press_command("recovery-continue"),
        Some(Command::RecoveryContinue)
    );
    assert_eq!(
        press_command("recovery-restore"),
        Some(Command::RecoveryRestore)
    );
    assert_eq!(
        press_command("recovery-replace"),
        Some(Command::RecoveryReplace)
    );
    assert_eq!(press_command("recovery-fork"), Some(Command::RecoveryFork));
    assert_eq!(
        press_command("recovery-details"),
        Some(Command::RecoveryDetails)
    );
    assert_eq!(
        press_command("recovery-view"),
        Some(Command::ToggleTranscript)
    );
    assert_eq!(press_command("recovery-close"), Some(Command::CloseTab));
    assert_eq!(press_command("unpause-button"), Some(Command::UnpauseTab));
    assert_eq!(press_command("nope"), None);
}

#[test]
fn fork_confirmation_texts() {
    let (title, body, yes) = fork_confirm_texts("Fix it", "Studio");
    assert_eq!(title, "Fork Fix it?");
    assert!(body.starts_with("Studio · This starts a separate conversation"));
    assert_eq!(yes, "Fork conversation");
}

// ----- the view ---------------------------------------------------------------

#[test]
fn a_plain_tab_shows_nothing() {
    let r = Recovery::default();
    let v = tab_chrome(&chrome(&r, None), &no_hint);
    assert_eq!(v["overlay"], "");
    assert_eq!(v["overlayButtons"], json!([]));
    assert_eq!(v["exitBanner"], "");
    assert_eq!(v["limited"], false);
}

#[test]
fn recovery_cards_carry_their_buttons_in_order() {
    let cases: Vec<(RecoveryState, &str, Vec<&str>)> = vec![
        (
            RecoveryState::Unreachable("ssh".into()),
            "Machine unreachable",
            vec![
                "recovery-retry",
                "recovery-view",
                "recovery-details",
                "recovery-close",
            ],
        ),
        (
            RecoveryState::TerminalUncertain("x".into()),
            "Session state uncertain",
            vec![
                "recovery-retry",
                "recovery-continue",
                "recovery-view",
                "recovery-details",
                "recovery-close",
            ],
        ),
        (
            RecoveryState::AgentUncertain("x".into(), launch()),
            "Another execution could still be running",
            vec![
                "recovery-retry",
                "recovery-view",
                "recovery-fork",
                "recovery-details",
                "recovery-close",
            ],
        ),
        (
            RecoveryState::AgentUncertain("x".into(), RecoveryLaunch::default()),
            "Another execution could still be running",
            vec![
                "recovery-retry",
                "recovery-view",
                "recovery-details",
                "recovery-close",
            ],
        ),
        (
            RecoveryState::Restorable(launch()),
            "Terminal ended",
            vec![
                "recovery-restore",
                "recovery-view",
                "recovery-fork",
                "recovery-close",
            ],
        ),
        (
            RecoveryState::Ended(launch()),
            "Session ended",
            vec!["recovery-replace", "recovery-view", "recovery-close"],
        ),
        (RecoveryState::Checking, "Checking session", vec![]),
        (RecoveryState::Restoring, "Restoring conversation", vec![]),
    ];
    for (state, title, expected) in cases {
        let r = Recovery {
            state: state.clone(),
            ..Default::default()
        };
        let v = tab_chrome(&chrome(&r, None), &no_hint);
        assert_eq!(v["overlay"], "recovery", "{state:?}");
        assert_eq!(v["overlayTitle"], title);
        assert_eq!(ids(&v), expected, "{state:?}");
        if !expected.is_empty() {
            assert_eq!(v["overlayButtons"][0]["primary"], true);
        }
    }
}

#[test]
fn fork_labels_note_and_details() {
    let mut r = Recovery {
        state: RecoveryState::AgentUncertain("tmux\u{7} said no".into(), launch()),
        ..Default::default()
    };
    let v = tab_chrome(&chrome(&r, None), &no_hint);
    assert_eq!(v["overlayButtons"][2]["label"], "Fork anyway…");
    assert_eq!(
        v["overlayNote"],
        "Forking keeps the uncertain execution unchanged."
    );
    assert_eq!(v["overlayReason"], "");
    r.toggle_details();
    let v = tab_chrome(&chrome(&r, None), &no_hint);
    assert_eq!(v["overlayReason"], "tmux said no");
    r.state = RecoveryState::Restorable(launch());
    let v = tab_chrome(&chrome(&r, None), &no_hint);
    assert_eq!(v["overlayButtons"][2]["label"], "Fork instead…");
    assert_eq!(v["overlayNote"], "");
    assert_eq!(v["overlayReason"], "");
}

#[test]
fn buttons_carry_their_command_hints() {
    let r = Recovery {
        state: RecoveryState::Ended(launch()),
        ..Default::default()
    };
    let hint = |c: Command| {
        if c == Command::CloseTab {
            "x".to_string()
        } else {
            String::new()
        }
    };
    let v = tab_chrome(&chrome(&r, None), &hint);
    assert_eq!(
        v["overlayButtons"][2],
        json!({ "id": "recovery-close", "label": "Close tab", "hint": "x", "primary": false, "disabled": false })
    );
}

#[test]
fn ssh_drops_cover_the_terminal() {
    let r = Recovery::default();
    let mut c = chrome(&r, None);
    c.connection = Some(("reconnecting", "ssh", false, String::new()));
    let v = tab_chrome(&c, &no_hint);
    assert_eq!(v["overlay"], "reconnecting");
    assert_eq!(v["overlayTitle"], "Reconnecting…");
    c.connection = Some(("disconnected", "ssh", true, String::new()));
    let v = tab_chrome(&c, &no_hint);
    assert_eq!(v["overlay"], "disconnected");
    assert_eq!(v["overlayTitle"], "Connection interrupted");
    // Mosh keeps its local prediction visible.
    c.connection = Some(("disconnected", "mosh", true, String::new()));
    assert_eq!(tab_chrome(&c, &no_hint)["overlay"], "");
}

#[test]
fn a_failed_remote_exit_covers_and_a_local_one_shows_the_banner() {
    let r = Recovery::default();
    let mut c = chrome(&r, None);
    c.exited = true;
    c.exit_code = Some(255);
    let v = tab_chrome(&c, &no_hint);
    assert_eq!(v["overlay"], "");
    assert_eq!(v["exitBanner"], "Connection ended (exit status 255).");
    c.exit_code = None;
    assert_eq!(tab_chrome(&c, &no_hint)["exitBanner"], "Connection ended.");
    c.exit_code = Some(0);
    assert_eq!(tab_chrome(&c, &no_hint)["exitBanner"], "Connection ended.");
    c.remote_machine = true;
    c.exit_code = Some(1);
    let v = tab_chrome(&c, &no_hint);
    assert_eq!(v["overlay"], "disconnected");
    // A remote tab shows the connection strip, not the exit banner.
    assert_eq!(v["exitBanner"], "");
    c.has_session = false;
    assert_eq!(tab_chrome(&c, &no_hint)["overlay"], "");
    // Recovery replaces both.
    let busy = Recovery {
        state: RecoveryState::Checking,
        ..Default::default()
    };
    let mut c = chrome(&busy, None);
    c.exited = true;
    let v = tab_chrome(&c, &no_hint);
    assert_eq!(v["overlay"], "recovery");
    assert_eq!(v["exitBanner"], "");
}

#[test]
fn a_paused_session_offers_unpause() {
    let paused = Session {
        id: "s".into(),
        state: "paused".into(),
        ..Default::default()
    };
    let m = machine_with(vec![paused.clone()]);
    let r = Recovery::default();
    let mut c = chrome(&r, Some((&m, &paused)));
    let v = tab_chrome(&c, &no_hint);
    assert_eq!(v["overlay"], "paused");
    assert_eq!(v["overlayTitle"], "Paused to free memory");
    assert_eq!(v["overlayDetail"], "Opens where it left off");
    assert_eq!(v["overlayNote"], "Any key in this tab resumes it too");
    assert_eq!(v["overlayButtons"][0]["id"], "unpause-button");
    assert_eq!(v["overlayButtons"][0]["label"], "Unpause");
    assert_eq!(v["overlayButtons"][0]["primary"], true);
    c.resuming = true;
    assert_eq!(
        tab_chrome(&c, &no_hint)["overlayButtons"][0]["label"],
        "Resuming…"
    );
    let custom = Session {
        status_text: "Paused 3h ago; opens where it left off".into(),
        ..paused.clone()
    };
    let c = chrome(&r, Some((&m, &custom)));
    assert_eq!(
        tab_chrome(&c, &no_hint)["overlayDetail"],
        "Paused 3h ago; opens where it left off"
    );
    let mut c = chrome(&r, Some((&m, &paused)));
    c.exited = true;
    assert_eq!(tab_chrome(&c, &no_hint)["overlay"], "");
}

#[test]
fn a_limited_session_shows_the_banner_while_running() {
    let limited = Session {
        id: "s".into(),
        state: "limited".into(),
        provider: "codex".into(),
        pid: 42,
        ..Default::default()
    };
    let m = machine_with(vec![limited.clone()]);
    let r = Recovery::default();
    let mut c = chrome(&r, Some((&m, &limited)));
    let v = tab_chrome(&c, &no_hint);
    assert_eq!(v["limited"], true);
    assert_eq!(v["limitedStatus"], session_summary(&limited));
    assert_eq!(v["limitedProvider"], provider_label("codex"));
    c.exited = true;
    let v = tab_chrome(&c, &no_hint);
    assert_eq!(v["limited"], false);
    assert_eq!(v["limitedStatus"], "");
}
