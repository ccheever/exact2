//! LLP 1133.007's grant families: `process.spawn`, the `home:` and `bundle:`
//! namespaces, `env.read login:` and `secret.hold`.
use super::*;

fn spawns(set: &GrantSet, program: &str) -> bool {
    set.permits(&Operation::ProcessSpawn {
        program: program.into(),
    })
}

#[test]
fn a_spawn_line_admits_its_program_by_whole_names() {
    let set = GrantSet::parse(
        "process.spawn path:ssh\nprocess.spawn /usr/bin/tar\nprocess.spawn home:/.t3/runtime/versions\nprocess.spawn bundle:/Helpers/t3/bin/t3",
    )
    .unwrap();
    assert!(spawns(&set, "path:ssh"));
    assert!(!spawns(&set, "path:sshd"));
    assert!(!spawns(&set, "/usr/bin/ssh"), "a name is not its path");
    assert!(spawns(&set, "/usr/bin/tar"));
    assert!(!spawns(&set, "/usr/bin/tar2"));
    assert!(spawns(&set, "home:/.t3/runtime/versions/1.2.0/t3"));
    assert!(!spawns(&set, "home:/.t3/runtime/versions2/t3"));
    assert!(!spawns(&set, "home:/.t3/runtime/versions/../../bin/sh"));
    assert!(spawns(&set, "bundle:/Helpers/t3/bin/t3"));
    assert!(!spawns(&set, "app:/Helpers/t3/bin/t3"), "namespaces differ");
    assert_eq!(set.spawn_targets().len(), 4);
}

#[test]
fn a_spawn_line_names_one_program_or_one_path() {
    for bad in [
        "process.spawn ssh",
        "process.spawn path:",
        "process.spawn path:bin/ssh",
        "process.spawn path:..",
        "process.spawn doc:/chosen",
        "process.spawn home:/",
        "process.spawn relative/bin",
        "process.spawn path:ssh extra",
    ] {
        assert!(GrantSet::parse(bad).is_err(), "{bad} parsed");
    }
}

#[test]
fn home_and_bundle_paths_are_namespaces_and_home_alone_is_refused() {
    let set = GrantSet::parse(
        "fs.read home:/.ssh\nfs.write home:/.t3\nfs.read bundle:/Resources/t3-runtime",
    )
    .unwrap();
    let read = |path: &str| set.permits(&Operation::FsRead { path: path.into() });
    assert!(read("home:/.ssh/config"));
    assert!(!read("home:/.ssh2"));
    assert!(!read("/.ssh/config"), "home: is not the root");
    assert!(read("bundle:/Resources/t3-runtime/a.tar"));
    assert!(set.permits(&Operation::FsWrite {
        path: "home:/.t3/userdata/x.json".into()
    }));
    for whole in ["fs.read home:/", "fs.write home:/", "sqlite.open home:/"] {
        let refused = GrantSet::parse(whole).unwrap_err();
        assert!(refused.contains("home-grant-whole"), "{whole}: {refused}");
    }
    // A host resolves these namespaces; realizing leaves them as they are.
    let realized = set.map_fs(|p| format!("/private{p}"));
    assert!(realized.permits(&Operation::FsRead {
        path: "home:/.ssh/config".into()
    }));
}

#[test]
fn a_login_variable_is_its_own_name() {
    let set = GrantSet::parse("env.read login:PATH\nenv.read HOME").unwrap();
    assert!(set.permits(&Operation::EnvRead {
        name: "login:PATH".into()
    }));
    assert!(!set.permits(&Operation::EnvRead {
        name: "PATH".into()
    }));
    assert!(!set.permits(&Operation::EnvRead {
        name: "login:HOME".into()
    }));
}

#[test]
fn a_secret_is_kept_or_held_never_both() {
    let set = GrantSet::parse("secret.hold ssh.password\nsecret.keep session").unwrap();
    assert_eq!(set.held_secrets(), vec!["ssh.password"]);
    assert!(set.permits(&Operation::SecretHold {
        name: "ssh.password".into()
    }));
    assert!(!set.permits(&Operation::SecretKeep {
        name: "ssh.password".into()
    }));
    assert!(!set.permits(&Operation::SecretHold {
        name: "session".into()
    }));
    let twice = GrantSet::parse("secret.hold a\nsecret.keep a").unwrap_err();
    assert!(twice.contains("secret-name-twice"), "{twice}");
    assert!(GrantSet::parse("secret.hold Bad").is_err());
}
