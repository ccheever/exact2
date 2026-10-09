//! The composer model's tests, and the fleet the launch tests share.

use std::collections::HashMap;

use super::*;
use crate::types::{LaunchProfile, Session, Snapshot, SpriteBinding};

fn machine(id: &str, home: &str, sessions: Vec<(&str, &str)>) -> Machine {
    let sessions = sessions
        .into_iter()
        .map(|(cwd, started)| Session {
            cwd: cwd.into(),
            started: started.into(),
            ..Default::default()
        })
        .collect();
    Machine {
        id: id.into(),
        name: id.to_uppercase(),
        last: Some(Snapshot {
            home: home.into(),
            sessions,
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Three machines (laptop with sessions, studio with a profile, blank)
/// and one connected Claude account.
pub(crate) fn state() -> State {
    let mut presets = HashMap::new();
    presets.insert(
        "Research".to_string(),
        LaunchProfile {
            machine_id: "studio".into(),
            cwd: "~/research".into(),
            ..Default::default()
        },
    );
    let laptop = vec![
        ("/Users/me/old", "2026-09-01T10:00:00Z"),
        ("/Users/me/Developer/fleet", "2026-09-09T10:00:00Z"),
        ("/Users/me", "2026-09-05T10:00:00Z"),
    ];
    State {
        machines: vec![
            machine("laptop", "/Users/me", laptop),
            machine(
                "studio",
                "/home/me",
                vec![("/srv/app", "2026-09-02T10:00:00Z")],
            ),
            machine("blank", "", vec![]),
        ],
        accounts: vec![Account {
            name: "me".into(),
            email: "me@example.com".into(),
            provider: "claude".into(),
            shared: true,
            status: "connected".into(),
            ..Default::default()
        }],
        presets,
    }
}

/// A composer on the laptop with nothing remembered.
pub(crate) fn fresh(st: &State) -> Launch {
    Launch::new(&LaunchMemory::default(), st, "laptop", |p| {
        default_account(st, p)
    })
}

fn triple(launch: &Launch) -> (String, String, String) {
    (
        launch.model.clone(),
        launch.effort.clone(),
        launch.permissions.clone(),
    )
}

#[test]
fn home_folders_display_with_a_tilde() {
    assert_eq!(tilde("/Users/me/Developer", "/Users/me"), "~/Developer");
    assert_eq!(tilde("/Users/me", "/Users/me/"), "~");
    assert_eq!(tilde("/Users/meow/x", "/Users/me"), "/Users/meow/x");
    assert_eq!(tilde("/srv/app", ""), "/srv/app");
}

#[test]
fn projects_put_the_current_machine_first_and_dedupe_folders() {
    let recent = vec![
        RecentProject {
            machine: "laptop".into(),
            cwd: "~/Developer/fleet/".into(),
        },
        RecentProject {
            machine: "studio".into(),
            cwd: "/srv/app".into(),
        },
        RecentProject {
            machine: "gone".into(),
            cwd: "/nowhere".into(),
        },
    ];
    let projects = collect_projects(&state(), &recent, "studio");
    let rows: Vec<String> = projects
        .iter()
        .map(|p| format!("{}:{}:{}", p.machine, p.cwd, p.detail()))
        .collect();
    assert_eq!(
        rows,
        vec![
            "studio:/srv/app:recent",
            "studio:~/research:profile Research",
            "laptop:~/Developer/fleet:recent",
            "laptop:~:session",
            "laptop:~/old:session",
            "blank:~:home",
        ]
    );
    assert_eq!(projects[0].name(), "app");
    assert_eq!(projects[3].name(), "home");
}

#[test]
fn fleet_s_own_folders_are_not_offered_as_projects() {
    let mut st = state();
    if let Some(last) = st.machines[0].last.as_mut() {
        last.sessions.push(Session {
            id: "viewer".into(),
            cwd: "~/.local/share/fleet/claude-remote-client/viewer".into(),
            started: "2026-09-25T00:00:00Z".into(),
            ..Default::default()
        });
    }
    let projects = collect_projects(&st, &[], "laptop");
    assert!(!projects
        .iter()
        .any(|p| p.cwd.contains("/.local/share/fleet/")));
}

#[test]
fn eas_projects_use_local_sources_instead_of_ephemeral_worker_paths() {
    let mut local = machine("local", "/Users/me", vec![("/Users/me/repo", "2026")]);
    local.local = true;
    let worker = "/tmp/fleet-sessions/session/runtime/workspace";
    let mut eas = machine("eas-project", "/tmp", vec![(worker, "2026")]);
    eas.eas = Some(Default::default());
    eas.last.as_mut().unwrap().sessions[0].eas = true;
    let state = State {
        machines: vec![local, eas],
        ..Default::default()
    };
    let projects = collect_projects(&state, &[], "eas-project");
    let cloud: Vec<_> = projects
        .iter()
        .filter(|p| p.machine == "eas-project")
        .collect();
    assert_eq!(cloud.len(), 1);
    assert_eq!(cloud[0].cwd, "~/repo");
}

#[test]
fn defaults_survive_only_when_their_parts_still_exist() {
    let st = state();
    let recent = vec![RecentProject {
        machine: "laptop".into(),
        cwd: "~/Developer/fleet".into(),
    }];
    let account = |provider: &str| format!("default-{provider}");
    let saved = |launch: Option<LaunchDefaults>| LaunchMemory {
        launch,
        recent_projects: recent.clone(),
        ..Default::default()
    };
    // Nothing remembered yet: a fresh dialog, first known folder on the machine.
    let launch = Launch::new(&saved(None), &st, "laptop", account);
    assert_eq!(launch.machine, "laptop");
    assert_eq!(launch.provider, "codex");
    assert_eq!(launch.account, "default-codex");
    assert_eq!(launch.cwd, "~/Developer/fleet");
    // It opens ready to type, not parked on a control.
    assert_eq!(launch.focus, Focus::Prompt);
    assert!(launch.menu.is_none());

    let remembered = LaunchDefaults {
        machine: "studio".into(),
        provider: "claude".into(),
        account: "claude:me@example.com".into(),
        cwd: "/srv/app".into(),
        model: "opus".into(),
        effort: "high".into(),
        permissions: "plan".into(),
    };
    let launch = Launch::new(&saved(Some(remembered.clone())), &st, "laptop", account);
    assert_eq!(launch.machine, "studio");
    assert_eq!(launch.provider, "claude");
    assert_eq!(launch.account, "claude:me@example.com");
    assert_eq!(launch.cwd, "/srv/app");
    assert_eq!(
        triple(&launch),
        ("opus".into(), "high".into(), "plan".into())
    );
    assert_eq!(launch.defaults(), remembered);

    // The machine and account disappeared: fall back without the model.
    let stale = LaunchDefaults {
        machine: "retired".into(),
        account: "claude:someone@else".into(),
        ..remembered.clone()
    };
    let launch = Launch::new(&saved(Some(stale)), &st, "laptop", account);
    assert_eq!(launch.machine, "laptop");
    assert_eq!(launch.provider, "claude");
    assert_eq!(launch.account, "default-claude");
    assert_eq!(launch.cwd, "~/Developer/fleet");
    assert_eq!(
        triple(&launch),
        (String::new(), String::new(), String::new())
    );

    let unknown_provider = LaunchDefaults {
        provider: "mystery".into(),
        ..remembered.clone()
    };
    let launch = Launch::new(&saved(Some(unknown_provider)), &st, "laptop", account);
    assert_eq!(launch.provider, "codex");
    assert_eq!(launch.account, "default-codex");

    // The new harnesses are remembered like the others.
    let grok = LaunchDefaults {
        provider: "grok".into(),
        account: String::new(),
        ..remembered
    };
    let launch = Launch::new(&saved(Some(grok)), &st, "laptop", account);
    assert_eq!(
        (launch.provider.as_str(), launch.account.as_str()),
        ("grok", "")
    );
}

#[test]
fn new_launches_offer_the_fly_source_instead_of_an_existing_sessions_sprite() {
    let mut st = state();
    st.machines.push(Machine {
        id: "fly-acme".into(),
        name: "New Sprite · acme".into(),
        sprite_source: "acme".into(),
        ..Default::default()
    });
    st.machines.push(Machine {
        id: "sprite-old".into(),
        name: "Old session".into(),
        sprite: Some(SpriteBinding::default()),
        ..Default::default()
    });
    let projects = collect_projects(&st, &[], "fly-acme");
    assert!(projects.iter().any(|p| p.machine == "fly-acme"));
    assert!(!projects.iter().any(|p| p.machine == "sprite-old"));
    let memory = LaunchMemory {
        launch: Some(LaunchDefaults {
            machine: "sprite-old".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let launch = Launch::new(&memory, &st, "fly-acme", |_| String::new());
    assert_eq!(launch.machine, "fly-acme");
}

#[test]
fn switching_machines_keeps_the_repo_or_uses_the_destination_folder() {
    let mut st = state();
    st.presets.clear();
    let mut launch = fresh(&st);
    launch.prompt = "Keep working".into();
    launch.catalog_key = "old machine catalog".into();
    launch
        .catalogs
        .insert("codex".into(), ModelState::default());

    launch.select_machine("studio");
    assert_eq!(launch.machine, "studio");
    assert_eq!(launch.cwd, "/srv/app");
    assert_eq!(launch.prompt, "Keep working");
    assert!(launch.catalogs.is_empty());
    assert!(launch.catalog_key.is_empty());

    // An enrolled host with no sessions or profiles is still launchable.
    launch.select_machine("blank");
    assert_eq!(launch.cwd, "~");
    let mut saved = LaunchMemory::default();
    saved.remember_launch(launch.defaults(), "Default");
    let reopened = Launch::new(&saved, &st, "laptop", |_| String::new());
    assert_eq!(reopened.machine, "blank");
    assert_eq!(reopened.cwd, "~");

    // The same repo on the other machine wins over its newest folder.
    let mut launch = fresh(&st);
    launch.projects.push(Project {
        machine: "studio".into(),
        machine_name: "STUDIO".into(),
        cwd: "/src/fleet".into(),
        source: ProjectSource::Session,
    });
    launch.select_machine("studio");
    assert_eq!(launch.cwd, "/src/fleet");
}

#[test]
fn selecting_the_current_machine_preserves_a_typed_folder() {
    let mut launch = fresh(&state());
    launch.cwd = "/custom/project".into();
    launch.select_machine("laptop");
    assert_eq!(launch.cwd, "/custom/project");
    // A machine enrolled after opening the dialog has no cached projects.
    launch.select_machine("new-host");
    assert_eq!(launch.machine, "new-host");
    assert_eq!(launch.cwd, "~");
}

#[test]
fn quick_launch_uses_home_on_current_and_other_machines() {
    let mut launch = fresh(&state());
    launch.cwd = "~/research".into();
    launch.select_quick_machine("laptop");
    assert_eq!(launch.cwd, "~");
    launch.select_quick_machine("studio");
    assert_eq!(launch.machine, "studio");
    assert_eq!(launch.cwd, "~");
    launch.select_quick_machine("laptop");
    assert_eq!(launch.cwd, "~");
}

#[test]
fn tab_wraps_around_the_line_and_the_prompt_is_one_of_its_stops() {
    assert_eq!(Focus::Prompt.moved(1), Focus::Project);
    assert_eq!(Focus::Project.moved(1), Focus::Model);
    assert_eq!(Focus::Model.moved(1), Focus::Account);
    assert_eq!(Focus::Account.moved(1), Focus::Prompt);
    assert_eq!(Focus::Prompt.moved(-1), Focus::Account);
    assert_eq!(Focus::Prompt.moved(4), Focus::Prompt);
}

#[test]
fn effort_choices_follow_the_harness() {
    assert_eq!(effort_choices("codex")[0], "");
    assert!(effort_choices("codex").contains(&"xhigh"));
    assert!(effort_choices("claude").contains(&"max"));
    assert_eq!(effort_choices("antigravity"), effort_choices("claude"));
    assert_eq!(effort_choices("opencode"), &[""]);
    assert_eq!(effort_choices("grok"), &[""]);
}

#[test]
fn providers_read_and_machines_shorten() {
    assert_eq!(provider_label("antigravity"), "Antigravity");
    assert_eq!(provider_label("grok"), "Grok");
    assert_eq!(provider_label("other"), "other");
    let mut m = Machine {
        name: "studio.local".into(),
        ..Default::default()
    };
    assert_eq!(machine_label(&m), "studio");
    m.name = ".hidden".into();
    assert_eq!(machine_label(&m), ".hidden");
    m.local = true;
    assert_eq!(machine_label(&m), "this Mac");
    assert_eq!(provider_dot("claude"), "warn");
    assert_eq!(provider_dot("codex"), "good");
    assert_eq!(provider_dot("grok"), "accent");
}

#[test]
fn model_rows_cover_every_harness_and_choosing_one_keeps_things_consistent() {
    let st = state();
    let account = |p: &str| format!("default-{p}");
    let mut launch = Launch::new(&LaunchMemory::default(), &st, "laptop", account);
    launch.effort = "xhigh".into();
    let opus = ModelOption {
        id: "opus".into(),
        name: "Opus".into(),
        description: "big".into(),
        default: true,
    };
    launch.catalogs.insert(
        "claude".into(),
        ModelState {
            options: vec![opus],
            ..Default::default()
        },
    );
    let rows = launch.model_rows();
    let ids: Vec<String> = rows
        .iter()
        .map(|r| format!("{}:{}", r.provider, r.id))
        .collect();
    assert_eq!(
        ids,
        vec![
            "codex:",
            "claude:",
            "claude:opus",
            "opencode:",
            "antigravity:",
            "grok:"
        ]
    );
    assert_eq!(rows[0].name, "Default");
    assert_eq!(launch.model_name(), "Default");
    assert_eq!(effort_label(&launch.effort), "Extra High");

    // Switching harness resets the account and drops an effort Claude lacks.
    let opus = rows[2].clone();
    launch.choose_model(&opus, account);
    assert_eq!(launch.provider, "claude");
    assert_eq!(launch.model, "opus");
    assert_eq!(launch.account, "default-claude");
    assert_eq!(launch.effort, "");
    assert_eq!(launch.model_name(), "Opus");

    // A custom model shows in its harness's section so it can be re-picked.
    launch.model = "sonnet-x".into();
    let rows = launch.model_rows();
    assert!(rows
        .iter()
        .any(|r| r.provider == "claude" && r.id == "sonnet-x"));
    assert_eq!(launch.model_name(), "sonnet-x");

    // A harness that lists its own "default" replaces the synthetic row.
    let recommended = ModelOption {
        id: "default".into(),
        name: "Default (recommended)".into(),
        ..Default::default()
    };
    launch
        .catalogs
        .get_mut("claude")
        .unwrap()
        .options
        .insert(0, recommended);
    let rows = launch.model_rows();
    let claude: Vec<&str> = rows
        .iter()
        .filter(|r| r.provider == "claude")
        .map(|r| r.id.as_str())
        .collect();
    assert_eq!(claude, vec!["default", "opus", "sonnet-x"]);

    // Recent models lead the list, and picking one restores its last effort.
    launch.recent_models = vec![RecentModel {
        provider: "codex".into(),
        id: "gpt-6".into(),
        name: "GPT-6".into(),
    }];
    launch
        .efforts
        .insert(effort_key("codex", "gpt-6"), "xhigh".into());
    let rows = launch.model_rows();
    assert!(rows[0].recent);
    assert_eq!(rows[0].name, "GPT-6");
    let recent = rows[0].clone();
    launch.choose_model(&recent, account);
    assert_eq!(launch.provider, "codex");
    assert_eq!(launch.effort, "xhigh");
    assert!(launch.has_effort());
    launch.provider = "opencode".into();
    assert!(!launch.has_effort());

    // Discovered Antigravity models retain their CLI slug and native login.
    launch.catalogs.insert(
        "antigravity".into(),
        ModelState {
            options: vec![ModelOption {
                id: "gemini-3.8-flash-high".into(),
                name: "Gemini 3.8 Flash (High)".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
    );
    let rows = launch.model_rows();
    assert!(rows
        .iter()
        .any(|r| r.provider == "antigravity" && r.id.is_empty()));
    let gemini = rows
        .iter()
        .find(|r| r.id == "gemini-3.8-flash-high")
        .unwrap();
    launch.choose_model(gemini, |_| String::new());
    assert_eq!(launch.provider, "antigravity");
    assert_eq!(launch.model, "gemini-3.8-flash-high");
    assert_eq!(launch.model_name(), "Gemini 3.8 Flash (High)");
    assert!(launch.account.is_empty());
}

#[test]
fn effort_levels_follow_the_harness_and_keep_a_custom_value() {
    let mut launch = fresh(&state());
    assert_eq!(
        launch.effort_levels(),
        ["", "minimal", "low", "medium", "high", "xhigh"]
    );
    launch.provider = "claude".into();
    launch.effort = "ultra".into();
    // A level the harness no longer offers is still listed, so a launch
    // never silently changes what it was set to.
    assert_eq!(
        launch.effort_levels().last().map(String::as_str),
        Some("ultra")
    );
    assert_eq!(effort_label("ultra"), "Custom");
    launch.provider = "opencode".into();
    launch.effort.clear();
    assert_eq!(launch.effort_levels(), [""]);
}

#[test]
fn permission_choices_lead_with_the_machine_default_and_follow_the_provider() {
    let mut launch = fresh(&state());
    launch.provider = "codex".into();
    let codex = ["", "read-only", "auto", "full-access", "yolo"];
    assert_eq!(launch.permission_choices(""), codex);
    // A machine default makes "provider default" a distinct choice.
    let with_default = ["", "default", "read-only", "auto", "full-access", "yolo"];
    assert_eq!(launch.permission_choices("yolo"), with_default);
    // A remembered mode the provider no longer lists is still visible.
    launch.permissions = "custom".into();
    assert!(launch
        .permission_choices("")
        .contains(&"custom".to_string()));
    launch.permissions.clear();
    launch.provider = "antigravity".into();
    assert_eq!(
        launch.permission_choices(""),
        ["", "accept-edits", "plan", "bypassPermissions"]
    );

    // Switching harness through the model list drops the override: modes
    // are named per provider, so the new machine default applies.
    launch.provider = "codex".into();
    launch.permissions = "auto".into();
    let rows = launch.model_rows();
    let row = rows
        .iter()
        .find(|r| r.provider == "claude")
        .expect("claude default row");
    launch.choose_model(row, |_| String::new());
    assert_eq!(launch.provider, "claude");
    assert_eq!(launch.permissions, "");
}

#[test]
fn submit_builds_the_run_argv_and_remembers_the_launch() {
    let st = state();
    let mut launch = fresh(&st);
    launch.provider = "claude".into();
    launch.account = "claude:me@example.com".into();
    launch.cwd = "/srv/app ".into();
    launch.model = "opus".into();
    launch.effort = "high".into();
    launch.prompt = "Fix the build".into();
    let mut memory = LaunchMemory::default();
    let s = launch.submit(&st, &mut memory, "r1").unwrap();
    let expected = [
        "run",
        "laptop",
        "--request-id",
        "r1",
        "--provider",
        "claude",
        "--account",
        "claude:me@example.com",
        "--cwd",
        "/srv/app",
        "--model",
        "opus",
        "--effort",
        "high",
        "--prompt",
        "Fix the build",
        "--attach",
    ];
    assert_eq!(s.argv, expected);
    assert_eq!(s.title, "Claude · LAPTOP");
    assert_eq!(s.tab_key, "laptop:r1:false");
    assert_eq!(memory.launch.as_ref().unwrap().model, "opus");
    let recent = RecentProject {
        machine: "laptop".into(),
        cwd: "/srv/app".into(),
    };
    assert_eq!(memory.recent_projects[0], recent);
    assert_eq!(memory.recent_models[0].id, "opus");
    assert_eq!(memory.model_efforts["claude/opus"], "high");
    // A permission choice travels before --attach; OpenCode drops the effort.
    launch.permissions = "plan".into();
    launch.provider = "opencode".into();
    let s = launch.submit(&st, &mut memory, "r2").unwrap();
    assert_eq!(
        &s.argv[s.argv.len() - 3..],
        ["--permissions", "plan", "--attach"]
    );
    assert_eq!(launch.effort, "");
    launch.cwd = "  ".into();
    let err = launch.submit(&st, &mut memory, "r3").unwrap_err();
    assert_eq!(err, "Choose a project folder");
    launch.machine = "gone".into();
    let err = launch.submit(&st, &mut memory, "r4").unwrap_err();
    assert_eq!(err, "Choose a project on an enrolled machine");
    // The auto machine runs `fleet auto-machine` without a folder.
    launch.auto_machine = true;
    let s = launch.submit(&st, &mut memory, "r5").unwrap();
    assert_eq!(s.argv[0], "auto-machine");
    assert_eq!(s.tab_key, "auto:r5");
}

#[test]
fn an_eas_machine_launches_only_codex_with_a_codex_account() {
    let mut st = state();
    st.machines[0].eas = Some(Default::default());
    let mut launch = fresh(&st);
    launch.provider = "claude".into();
    launch.account = "claude:me@example.com".into();
    let mut memory = LaunchMemory::default();
    let err = launch.submit(&st, &mut memory, "r1").unwrap_err();
    assert_eq!(err, "EAS requires Codex and a connected Codex account");
    launch.provider = "codex".into();
    assert!(launch.submit(&st, &mut memory, "r2").is_err());
    st.accounts.push(Account {
        name: "cx".into(),
        email: "cx@example.com".into(),
        provider: "codex".into(),
        shared: true,
        ..Default::default()
    });
    launch.account = "codex:cx@example.com".into();
    assert!(launch.submit(&st, &mut memory, "r3").is_ok());
    assert!(memory.launch.is_some());
}
