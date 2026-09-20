#[test]
fn reel_rejects_camera_overrides_before_preflight() {
    let mut failures=vec![];
    for command in ["reel","render"] {
        let mut child=std::process::Command::new(env!("CARGO_BIN_EXE_clod-view"))
            .args([command,"/no-such-cluster-lod-fixture.clod","--eye","0,-1,1","--target","0,0,1"])
            .stderr(std::process::Stdio::piped()).spawn().unwrap();
        println!("camera_override command={command} pid={}",child.id());
        let mut stderr=String::new();
        std::io::Read::read_to_string(&mut child.stderr.take().unwrap(),&mut stderr).unwrap();
        let exit=child.wait().unwrap();
        let rejected=stderr.contains("reel does not accept eye/target overrides");
        println!("camera_override command={command} exit={exit} rejected_before_load={rejected}");
        if rejected!=(command=="reel") { failures.push(command); }
    }
    println!("camera_override_cases=2 failures={failures:?}");
    assert!(failures.is_empty(),"{failures:?}");
}
