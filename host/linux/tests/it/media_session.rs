//! LLP 1098 D9, §5 on Linux: the media session is a record here, read from
//! node props and the runner's handlers; the owner is the latest mount,
//! recorded as commits land (not when the agent asks), and a slot reused by
//! a remount is a new claimant; `tap … mediasession` dispatches the action,
//! never a press, and refuses a non-owner, an unhandled action and the
//! player's own play and pause.

use exact_linux::{agent, presenter::PainterChoice, Presenter};
use exact_runner::{DataError, DataSource, Value};
use serde_json::json;

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const SOURCE: &str = r#"component Player
  state trailer = false
  state episode = false
  state seek = 0
  state pressed = 0
  state heard = ""
  action both
    trailer = true
    episode = true
  action toggleTrailer
    trailer = not trailer
  action showEpisode
    episode = true
  action skipBy(sign: number, d: MediaSessionActionDetails)
    seek = seek + sign * d.seekOffset
  action seekAt(d: MediaSessionActionDetails)
    seek = d.seekTime
  action next(d: MediaSessionActionDetails)
    heard = d.action
  action press
    pressed = pressed + 1
  view
    column
      button "both" press=both testId="both"
      button "trailer" press=toggleTrailer testId="toggle"
      button "episode" press=showEpisode testId="episode"
      when trailer
        video "assets/t.mp4" testId="trailer" width=160 height=90 metadata=MediaMetadata(title="Trailer", artist="", album="", artwork="") nexttrack=next press=press
      when episode
        audio "assets/ep.mp3" testId="audio" controls=true metadata=MediaMetadata(title="Episode 1", artist="Show", album="", artwork="assets/art.png") seekforwardOffset=30 seekforward=skipBy(1) seekbackward=skipBy(-1) seekto=seekAt previoustrack=next press=press
"#;

fn boot() -> Presenter<NoData> {
    let plan = contract::compile(SOURCE).unwrap();
    let dir = std::env::temp_dir().join(format!("exact-media-session-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (p, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (400., 300.),
        1.,
        dir,
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn ask(p: &mut Presenter<NoData>, request: serde_json::Value) -> serde_json::Value {
    serde_json::from_str(&agent::handle(p, &request.to_string())).unwrap()
}

fn id(p: &Presenter<NoData>, test_id: &str) -> u32 {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// Press one of the fixture's buttons: one commit.
fn press(p: &mut Presenter<NoData>, test_id: &str) {
    let target = id(p, test_id);
    let reply = ask(p, json!({"op": "tap", "id": target}));
    assert!(reply.get("error").is_none(), "{test_id}: {reply}");
}

fn session(p: &mut Presenter<NoData>) -> serde_json::Value {
    ask(p, json!({"op": "state"}))["mediaSession"].clone()
}

fn slot(p: &mut Presenter<NoData>, name: &str) -> serde_json::Value {
    ask(p, json!({"op": "state"}))["slots"][name].clone()
}

#[test]
fn the_owner_is_the_latest_mount_and_the_actions_are_dispatched() {
    let mut p = boot();
    assert_eq!(
        session(&mut p),
        json!({"owner": null, "testId": null, "claimants": [], "metadata": null, "actions": [],
            "seekOffsets": null, "playbackState": "none", "position": null, "published": "none",
            "readback": null})
    );
    // The episode mounts in one commit, the trailer (earlier in the tree) in
    // a later one, with no `state` between: the trailer owns by mount order.
    press(&mut p, "episode");
    press(&mut p, "toggle");
    let (trailer, audio) = (id(&p, "trailer"), id(&p, "audio"));
    let s = session(&mut p);
    assert_eq!(s["owner"], json!(trailer), "{s}");
    assert_eq!(s["testId"], json!("trailer"));
    assert_eq!(s["claimants"], json!([trailer, audio]));
    assert_eq!(
        s["actions"],
        json!(["nexttrack"]),
        "no play or pause: no player"
    );
    assert_eq!(s["playbackState"], json!("paused"));
    // A non-owner's action is refused, and nothing is dispatched.
    let r = ask(
        &mut p,
        json!({"op": "tap", "id": audio, "mediaSession": "seekforward"}),
    );
    assert_eq!(
        r["error"],
        json!("mediasession: \"audio\" does not own the media session; \"trailer\" does")
    );
    // Unmounted, the episode owns.
    press(&mut p, "toggle");
    let s = session(&mut p);
    assert_eq!(
        (s["owner"].clone(), s["claimants"].clone()),
        (json!(audio), json!([audio]))
    );
    assert_eq!(
        s["metadata"],
        json!({"title": "Episode 1", "artist": "Show", "album": "", "artwork": "assets/art.png"})
    );
    assert_eq!(
        s["actions"],
        json!(["previoustrack", "seekbackward", "seekforward", "seekto"])
    );
    assert_eq!(
        s["seekOffsets"],
        json!({"seekbackward": 10.0, "seekforward": 30.0})
    );
    // The element's offset when no seconds are given, the given ones else.
    let r = ask(
        &mut p,
        json!({"op": "tap", "id": audio, "mediaSession": "seekforward"}),
    );
    assert_eq!(r["delivery"], json!("substituted"), "{r}");
    assert_eq!(slot(&mut p, "seek"), json!(30));
    ask(
        &mut p,
        json!({"op": "tap", "id": audio, "mediaSession": "seekbackward", "seconds": 5}),
    );
    assert_eq!(slot(&mut p, "seek"), json!(25));
    ask(
        &mut p,
        json!({"op": "tap", "id": audio, "mediaSession": "seekto", "seconds": 600}),
    );
    assert_eq!(slot(&mut p, "seek"), json!(600));
    ask(
        &mut p,
        json!({"op": "tap", "id": audio, "mediaSession": "previoustrack"}),
    );
    assert_eq!(slot(&mut p, "heard"), json!("previoustrack"));
    // Never a press.
    assert_eq!(slot(&mut p, "pressed"), json!(0));
    let r = ask(
        &mut p,
        json!({"op": "tap", "id": audio, "mediaSession": "nexttrack"}),
    );
    assert_eq!(
        r["error"],
        json!("mediasession: \"audio\" has no nexttrack")
    );
    let r = ask(
        &mut p,
        json!({"op": "tap", "id": audio, "mediaSession": "play"}),
    );
    assert!(
        r["error"].as_str().unwrap().contains("play is unavailable"),
        "{r}"
    );
    // Mounted again: a new key, so a new mount, the latest.
    press(&mut p, "toggle");
    assert_eq!(session(&mut p)["owner"], json!(id(&p, "trailer")));
}

#[test]
fn of_two_mounted_in_one_commit_the_later_in_the_tree_owns() {
    let mut p = boot();
    press(&mut p, "both");
    assert_eq!(session(&mut p)["owner"], json!(id(&p, "audio")));
}
