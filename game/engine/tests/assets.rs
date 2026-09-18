use exact_game::*;
struct Loading;
impl Game for Loading {
    const ID: &'static str = "asset-loading";
    const ASSETS: &'static [&'static str] = &["crate.model"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        assert!(w.model("crate.model").is_some());
        w.spawn_named("crate", (Transform::default(), Mesh::asset("crate.model")));
        w.publish("setup", true);
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.publish("ticks", w.tick() as u32);
    }
}
#[test]
fn declared_models_gate_setup_and_ticks_and_survive_restore() {
    let bytes = bin::to_vec(&asset::Model::default());
    let mut a = Sim::<Loading>::new(()).unwrap();
    let mut b = Sim::<Loading>::new(()).unwrap();
    assert_eq!(a.take_assets(), ["crate.model"]);
    assert!(a.take_assets().is_empty());
    assert_eq!(a.world().len(), 0);
    a.run(5000.);
    assert_eq!(a.world().tick(), 0);
    assert!(a.agent(r#"{"op":"state"}"#).contains("crate.model"));
    a.asset("crate.model", Some(&bytes)).unwrap();
    b.asset("crate.model", Some(&bytes)).unwrap();
    assert_eq!(a.world().len(), 1);
    a.run(1000.);
    b.run(1000.);
    assert_eq!(a.world().tick(), 60);
    assert_eq!(a.world().hash(), b.world().hash());
    let saved = a.save();
    a.restore(&saved).unwrap();
    assert!(a.world().model("crate.model").is_some());
    assert_eq!(a.world().hash(), b.world().hash());
}
#[test]
fn missing_declared_model_refuses_by_name_and_never_ticks() {
    let mut sim = Sim::<Loading>::new(()).unwrap();
    assert!(sim
        .asset("crate.model", None)
        .unwrap_err()
        .contains("crate.model"));
    sim.run(1000.);
    assert_eq!(sim.world().tick(), 0);
}

#[test]
fn embedded_assets_do_not_request_host_delivery_but_baked_models_still_do() {
    struct Mixed;
    impl Game for Mixed {
        const ID: &'static str = "mixed-assets";
        type Args = ();
        fn assets() -> &'static [Asset] {
            &[Asset {
                name: "embedded.glb",
                bytes: b"renderer-owned",
            }]
        }
        fn setup(w: &mut World, _: &()) {
            w.spawn((Transform::default(), Mesh::asset("embedded.glb")));
            w.spawn((Transform::default(), Mesh::asset("delivered.model")));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut sim = Sim::<Mixed>::new(()).unwrap();
    assert_eq!(sim.take_assets(), ["delivered.model"]);
    let saved = sim.save();
    let reply = sim.agent(r#"{"op":"state","now":1000}"#);
    assert!(
        reply.contains(r#""loading":["delivered.model"]"#),
        "{reply}"
    );
    assert!(reply.contains(r#""ownership":{"#), "{reply}");
    assert_eq!(
        sim.save(),
        saved,
        "inspection never advances a loading world"
    );
    sim.asset(
        "delivered.model",
        Some(&bin::to_vec(&asset::Model::default())),
    )
    .unwrap();
    assert!(sim.take_assets().is_empty());
    sim.restore(&saved).unwrap();
    assert!(sim.take_assets().is_empty());
}
