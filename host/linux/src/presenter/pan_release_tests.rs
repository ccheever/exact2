//! LLP 1057 §10.6 on Linux: a pan that began ends with one `panrelease`, its
//! velocity the contact tracker's; a tap releases nothing; a cancel at rest.
use super::super::*;
use exact_runner::DataError;

struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
const SOURCE: &str = r#"component App
  state x = 0
  state vx = 0
  state vy = 0
  state releases = 0
  state presses = 0
  action moved(dx: number, dy: number) writes x
    x = x + dx
  action released(sx: number, sy: number) writes vx, vy, releases
    vx = sx
    vy = sy
    releases = releases + 1
  action pressed writes presses
    presses = presses + 1
  view
    column width=400 height=500
      box testId="card" width=400 height=300 pan=moved panrelease=released press=pressed touch-action="none"
        text "drag me"
      text `${x} ${vx} ${vy} ${releases} ${presses}` testId="out"
"#;
fn boot() -> Presenter<NoData> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(SOURCE).unwrap().encode(),
        NoData,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn out(p: &Presenter<NoData>) -> Vec<f64> {
    let k = p.host.kernel();
    let id = k.node_by_key(k.find_by_test_id("out")[0]).unwrap().id;
    k.node(id)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .split(' ')
        .map(|s| s.parse().unwrap())
        .collect()
}

#[test]
fn a_flick_releases_once_at_the_trackers_velocity() {
    let mut p = boot();
    assert!(p.pointer_down(100., 100., 0.).unwrap());
    for i in 1..=5 {
        p.pointer_move(100. + 20. * i as f32, 100., 16. * i as f64)
            .unwrap();
    }
    p.pointer_up(200., 100., 80.).unwrap();
    let [x, vx, vy, releases, presses] = out(&p)[..] else {
        panic!()
    };
    assert_eq!((x, releases, presses), (100., 1., 0.));
    assert!((vx - 1250.).abs() <= 1., "{vx}");
    assert_eq!(vy, 0.);
}

#[test]
fn a_tap_presses_and_releases_nothing() {
    let mut p = boot();
    assert!(p.pointer_down(100., 100., 0.).unwrap());
    p.pointer_move(102., 101., 10.).unwrap();
    p.pointer_up(102., 101., 20.).unwrap();
    assert_eq!(out(&p), vec![0., 0., 0., 0., 1.]);
}

#[test]
fn a_cancelled_pan_releases_at_rest() {
    let mut p = boot();
    assert!(p.pointer_down(100., 100., 0.).unwrap());
    p.pointer_move(150., 100., 16.).unwrap();
    p.pointer_move(200., 100., 32.).unwrap();
    p.pointer_cancel(40.).unwrap();
    assert_eq!(out(&p), vec![100., 0., 0., 1., 0.]);
    // A held contact that stops before lifting releases at rest too.
    assert!(p.pointer_down(100., 100., 100.).unwrap());
    p.pointer_move(160., 100., 116.).unwrap();
    p.pointer_up(160., 100., 500.).unwrap();
    assert_eq!(out(&p), vec![160., 0., 0., 2., 0.]);
}
