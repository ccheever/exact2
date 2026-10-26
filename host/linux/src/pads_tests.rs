//! The registry's claims (LLP 1133.022 D4): an effect completes after its
//! span; a second on the same device preempts the first, whichever client
//! made it; a disconnect ends every claim on the device; a client's end
//! ends only its own; `current` follows a press, not a stick.
use super::*;
use std::sync::mpsc;

/// The registry is the process's: these tests take turns with it.
static TURN: Mutex<()> = Mutex::new(());

fn fresh() -> std::sync::MutexGuard<'static, ()> {
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    *registry().lock().unwrap() = Registry::default();
    turn
}

struct Motors(Arc<Mutex<Vec<String>>>);
impl Output for Motors {
    fn rumble(&mut self, strong: f32, weak: f32, ms: u32) -> bool {
        self.0
            .lock()
            .unwrap()
            .push(format!("rumble {strong} {weak} {ms}"));
        true
    }
    fn stop(&mut self) {
        self.0.lock().unwrap().push("stop".into());
    }
}

fn pad(index: u8, vendor: u16) -> Arc<Mutex<Vec<String>>> {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let identity = Identity {
        id: format!("pad {index}"),
        vendor,
        ..Default::default()
    };
    connect(index, identity, Some(Box::new(Motors(heard.clone()))));
    heard
}

fn ended() -> (Done, mpsc::Receiver<&'static str>) {
    let (tx, rx) = mpsc::channel();
    (Box::new(move |how| tx.send(how).unwrap()), rx)
}

#[test]
fn an_effect_completes_after_its_span_and_a_second_preempts_it() {
    let _turn = fresh();
    tick(1000.);
    let motors = pad(0, 0x054c);
    let (a, b) = (client(), client());
    let (done, first) = ended();
    rumble(a, 0, 0.6, 0.3, 0., 180, done).unwrap();
    assert_eq!(*motors.lock().unwrap(), ["rumble 0.6 0.3 180"]);
    tick(1100.);
    assert!(first.try_recv().is_err(), "still playing");
    // Another session's effect on the same pad preempts the first.
    let (done, second) = ended();
    rumble(b, 0, 1., 1., 50., 100, done).unwrap();
    assert_eq!(first.try_recv(), Ok("preempted"));
    assert_eq!(motors.lock().unwrap().last().unwrap(), "stop");
    tick(1149.);
    assert_eq!(motors.lock().unwrap().len(), 2, "its start delay: not yet");
    tick(1150.);
    assert_eq!(motors.lock().unwrap().last().unwrap(), "rumble 1 1 100");
    tick(1250.);
    assert_eq!(second.recv(), Ok("complete"));
}

#[test]
fn a_disconnect_ends_the_devices_claims_and_a_return_is_a_new_device() {
    let _turn = fresh();
    pad(1, 0x045e);
    let a = client();
    let (done, heard) = ended();
    rumble(a, 1, 0.5, 0.5, 0., 1000, done).unwrap();
    disconnect(1);
    assert_eq!(heard.try_recv(), Ok("preempted"));
    assert_eq!(
        rumble(a, 1, 0.5, 0.5, 0., 10, Box::new(|_| {})),
        Err("NotFoundError")
    );
    pad(1, 0x045e);
    let (done, again) = ended();
    rumble(a, 1, 0.5, 0.5, 0., 10, done).unwrap();
    connect(1, Identity::default(), None);
    assert_eq!(
        again.try_recv(),
        Ok("preempted"),
        "another pad took the slot"
    );
    assert_eq!(
        rumble(a, 1, 0.5, 0.5, 0., 10, Box::new(|_| {})),
        Err("NotSupportedError"),
        "a pad with no motors"
    );
}

#[test]
fn a_clients_end_ends_only_its_own_claims() {
    let _turn = fresh();
    pad(0, 0);
    pad(1, 0);
    let (a, b) = (client(), client());
    let (done, mine) = ended();
    rumble(a, 0, 1., 1., 0., 1000, done).unwrap();
    let (done, theirs) = ended();
    rumble(b, 1, 1., 1., 0., 1000, done).unwrap();
    end(a);
    assert_eq!(mine.try_recv(), Ok("preempted"));
    assert!(theirs.try_recv().is_err());
    reset(1).unwrap();
    assert_eq!(theirs.try_recv(), Ok("preempted"));
}

#[test]
fn the_pad_in_use_follows_a_press_and_its_change_is_told() {
    let _turn = fresh();
    pad(0, 0x054c);
    pad(2, 0x057e);
    let (tx, told) = mpsc::channel();
    let watcher = client();
    watch(watcher, move || tx.send(()).unwrap());
    let mut r = Reading::default();
    r.axes[0] = 0.3;
    read(2, r);
    assert_eq!(list().1, None, "a stick is not use");
    r.buttons[2] = 1.;
    read(2, r);
    assert_eq!(list().1, Some(2));
    assert!(told.try_recv().is_ok());
    read(2, r);
    assert!(told.try_recv().is_err(), "held, not pressed again");
    let (pads, _) = list();
    assert_eq!(
        pads.iter()
            .map(|p| (p.index, p.identity.family()))
            .collect::<Vec<_>>(),
        [(0, "playstation"), (2, "nintendo")]
    );
    disconnect(2);
    assert_eq!(list().1, None);
    end(watcher);
}

#[test]
fn a_test_builds_pads_connect_from_the_environment() {
    let _turn = fresh();
    let r = Mutex::new(Registry::default());
    inject_into(
        &r,
        r#"[{"index":0,"id":"DualSense Wireless Controller","vendor":"054c","product":"0ce6","pressed":[0]}]"#,
    );
    let r = r.into_inner().unwrap();
    let slot = r.slots[0].as_ref().unwrap();
    assert_eq!(slot.identity.family(), "playstation");
    assert_eq!(slot.identity.product, 0x0ce6);
    assert_eq!(r.current, Some(0));
    assert!(slot.output.is_some());
}
