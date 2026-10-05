use exact_game::*;
#[derive(Default, Data)]
struct Island {
    seed: u64,
    lanterns: Vec<Vec3>,
    heights: Vec<f32>,
    sign: String,
}
struct LevelGame;
impl Game for LevelGame {
    const ID: &'static str = "typed-level";
    const LEVELS: &'static [asset::Level] = &[asset::Level::of::<Island>("island.level.json")];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        let level = w.level::<Island>("island.level.json").unwrap();
        w.reseed(level.seed);
        for p in &level.lanterns {
            w.spawn((Transform::at(p.x, p.y, p.z),));
        }
        w.publish("sign", level.sign.clone());
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
const FIRST: &[u8] =
    br#"{"seed":7,"lanterns":[[1,2,3],[4,5,6],[7,8,9]],"heights":[0.25,0.5,-0.0],"sign":"Island"}"#;
#[test]
fn typed_level_arrives_before_setup_and_refuses_changed_restore_by_name() {
    let mut sim = Sim::<LevelGame>::new(()).unwrap();
    assert!(sim.is_loading());
    assert_eq!(sim.take_assets(), ["island.level.json"]);
    assert!(sim.take_assets().is_empty(), "the level is requested once");
    assert_eq!(sim.world().query::<&Transform>().iter().count(), 0);
    sim.run(1000.);
    assert_eq!(sim.world().tick(), 0);
    sim.asset("island.level.json", Some(FIRST)).unwrap();
    assert!(!sim.is_loading());
    assert_eq!(sim.world().query::<&Transform>().iter().count(), 3);
    assert_eq!(
        sim.world()
            .level::<Island>("island.level.json")
            .unwrap()
            .seed,
        7
    );
    assert!(sim.world().level::<Vec3>("island.level.json").is_err());
    assert_eq!(
        bin::to_vec(
            &sim.world()
                .level::<Island>("island.level.json")
                .unwrap()
                .heights
        ),
        bin::to_vec(&vec![0.25f32, 0.5, -0.0])
    );
    sim.take_assets();
    sim.invalidate_device_assets();
    assert!(
        sim.device_assets_ready(),
        "levels need no device preparation"
    );
    let save = sim.save().unwrap();
    let mut fresh = Sim::<LevelGame>::new(()).unwrap();
    fresh.asset("island.level.json", Some(FIRST)).unwrap();
    fresh.restore(&save).unwrap();
    assert_eq!(fresh.world().hash(), sim.world().hash());
    let changed = String::from_utf8(FIRST.to_vec())
        .unwrap()
        .replace("Island", "Other island");
    let mut other = Sim::<LevelGame>::new(()).unwrap();
    other
        .asset("island.level.json", Some(changed.as_bytes()))
        .unwrap();
    assert!(other
        .restore(&save)
        .unwrap_err()
        .to_string()
        .contains("island.level.json"));
}
/// A look's palette: presentation data only `present` reads.
#[derive(Default, Data)]
struct Palette {
    sky: [f32; 3],
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Presentation)]
struct Sky([f32; 3]);
struct ShownGame;
impl Game for ShownGame {
    const ID: &'static str = "shown-level";
    const LEVELS: &'static [asset::Level] = &[
        asset::Level::of::<Island>("island.level.json"),
        asset::Level::shown::<Palette>("palette.level.json"),
    ];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        assert!(w.level::<Palette>("palette.level.json").is_err());
        w.spawn_named("sky", Transform::default());
    }
    fn present(p: &mut Present, _: &()) {
        let sky = p.level::<Palette>("palette.level.json").unwrap().sky;
        let e = p.named("sky").unwrap();
        p.insert(e, Sky(sky));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn shown_data_is_awaited_read_by_present_and_outside_saves() {
    let load = |palette: &[u8]| {
        let mut sim = Sim::<ShownGame>::new(()).unwrap();
        let mut names = sim.take_assets();
        names.sort();
        assert_eq!(names, ["island.level.json", "palette.level.json"]);
        sim.asset("island.level.json", Some(FIRST)).unwrap();
        assert!(sim.is_loading(), "setup waits for presentation data too");
        sim.asset("palette.level.json", Some(palette)).unwrap();
        assert!(!sim.is_loading());
        sim
    };
    let mut sim = load(br#"{"sky":[0.1,0.2,0.3]}"#);
    sim.run(100.);
    let sky = |sim: &Sim<ShownGame>| *sim.world().get::<Sky>("sky").unwrap();
    assert_eq!(sky(&sim), Sky([0.1, 0.2, 0.3]));
    let save = sim.save().unwrap();
    let mut other = load(br#"{"sky":[0.9,0.8,0.7]}"#);
    other.restore(&save).unwrap();
    assert_eq!(other.world().hash(), sim.world().hash());
    assert_eq!(sky(&other), Sky([0.9, 0.8, 0.7]), "a new palette draws the same world");
}

#[test]
fn malformed_runtime_level_names_the_authored_field() {
    let mut sim = Sim::<LevelGame>::new(()).unwrap();
    let err = sim
        .asset("island.level.json", Some(br#"{"lanterns":[[1,"bad",3]]}"#))
        .unwrap_err();
    assert!(err.contains("lanterns.0"), "{err}");
    assert!(sim.is_loading());
    let err = sim
        .asset("island.level.json", Some(br#"{"heights":[1,"bad"]}"#))
        .unwrap_err();
    assert!(err.contains("heights.1"), "{err}");
}

#[test]
fn level_declaration_validates_names_and_counts_against_the_surface_limit() {
    struct BadName;
    impl Game for BadName {
        const ID: &'static str = "bad-level-name";
        const LEVELS: &'static [asset::Level] =
            &[asset::Level::of::<Island>("../island.level.json")];
        type Args = ();
        fn setup(_: &mut World, _: &()) {
            panic!("invalid level must refuse before setup")
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    assert!(Sim::<BadName>::new(())
        .err()
        .unwrap()
        .contains("../island.level.json"));
    struct WrongKind;
    impl Game for WrongKind {
        const ID: &'static str = "wrong-level-kind";
        const LEVELS: &'static [asset::Level] = &[asset::Level::of::<Island>("island.model")];
        type Args = ();
        fn setup(_: &mut World, _: &()) {
            panic!("invalid level must refuse before setup")
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    assert!(Sim::<WrongKind>::new(())
        .err()
        .unwrap()
        .contains("island.model"));
    #[rustfmt::skip]
    const MODELS: [&str; 256] = [
        "0.model", "1.model", "2.model", "3.model", "4.model", "5.model", "6.model", "7.model",
        "8.model", "9.model", "10.model", "11.model", "12.model", "13.model", "14.model", "15.model",
        "16.model", "17.model", "18.model", "19.model", "20.model", "21.model", "22.model", "23.model",
        "24.model", "25.model", "26.model", "27.model", "28.model", "29.model", "30.model", "31.model",
        "32.model", "33.model", "34.model", "35.model", "36.model", "37.model", "38.model", "39.model",
        "40.model", "41.model", "42.model", "43.model", "44.model", "45.model", "46.model", "47.model",
        "48.model", "49.model", "50.model", "51.model", "52.model", "53.model", "54.model", "55.model",
        "56.model", "57.model", "58.model", "59.model", "60.model", "61.model", "62.model", "63.model",
        "64.model", "65.model", "66.model", "67.model", "68.model", "69.model", "70.model", "71.model",
        "72.model", "73.model", "74.model", "75.model", "76.model", "77.model", "78.model", "79.model",
        "80.model", "81.model", "82.model", "83.model", "84.model", "85.model", "86.model", "87.model",
        "88.model", "89.model", "90.model", "91.model", "92.model", "93.model", "94.model", "95.model",
        "96.model", "97.model", "98.model", "99.model", "100.model", "101.model", "102.model", "103.model",
        "104.model", "105.model", "106.model", "107.model", "108.model", "109.model", "110.model", "111.model",
        "112.model", "113.model", "114.model", "115.model", "116.model", "117.model", "118.model", "119.model",
        "120.model", "121.model", "122.model", "123.model", "124.model", "125.model", "126.model", "127.model",
        "128.model", "129.model", "130.model", "131.model", "132.model", "133.model", "134.model", "135.model",
        "136.model", "137.model", "138.model", "139.model", "140.model", "141.model", "142.model", "143.model",
        "144.model", "145.model", "146.model", "147.model", "148.model", "149.model", "150.model", "151.model",
        "152.model", "153.model", "154.model", "155.model", "156.model", "157.model", "158.model", "159.model",
        "160.model", "161.model", "162.model", "163.model", "164.model", "165.model", "166.model", "167.model",
        "168.model", "169.model", "170.model", "171.model", "172.model", "173.model", "174.model", "175.model",
        "176.model", "177.model", "178.model", "179.model", "180.model", "181.model", "182.model", "183.model",
        "184.model", "185.model", "186.model", "187.model", "188.model", "189.model", "190.model", "191.model",
        "192.model", "193.model", "194.model", "195.model", "196.model", "197.model", "198.model", "199.model",
        "200.model", "201.model", "202.model", "203.model", "204.model", "205.model", "206.model", "207.model",
        "208.model", "209.model", "210.model", "211.model", "212.model", "213.model", "214.model", "215.model",
        "216.model", "217.model", "218.model", "219.model", "220.model", "221.model", "222.model", "223.model",
        "224.model", "225.model", "226.model", "227.model", "228.model", "229.model", "230.model", "231.model",
        "232.model", "233.model", "234.model", "235.model", "236.model", "237.model", "238.model", "239.model",
        "240.model", "241.model", "242.model", "243.model", "244.model", "245.model", "246.model", "247.model",
        "248.model", "249.model", "250.model", "251.model", "252.model", "253.model", "254.model", "255.model",
    ];
    struct Bounded<const N: usize>;
    impl<const N: usize> Game for Bounded<N> {
        const ID: &'static str = "bounded-level";
        const ASSETS: &'static [&'static str] = MODELS.split_at(N).0;
        const LEVELS: &'static [asset::Level] = LevelGame::LEVELS;
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut full = Sim::<Bounded<255>>::new(()).unwrap();
    let names = full.take_assets();
    assert_eq!(names.len(), 256);
    assert!(names.iter().any(|name| name == "island.level.json"));
    let mut larger = Sim::<Bounded<256>>::new(()).unwrap();
    assert_eq!(larger.take_assets().len(), 257);
}
