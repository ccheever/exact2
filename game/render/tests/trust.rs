//! The update trust baked into this crate (LLP 1127.000 D4): a build baked with
//! `EXACT_UPDATE_TRUST=production` refuses every look request by name, and any
//! other build answers. This test links the rlib a game's GPU crate links and
//! reads the trust of the run that built it, so a production run over a
//! target a development build left (the incremental case) fails here unless
//! the rlib was rebuilt: `cargo test -p exact-game-render --test trust`, then
//! `EXACT_UPDATE_TRUST=production cargo test -p exact-game-render --test trust`.
#![cfg(not(target_arch = "wasm32"))]
use exact_game::*;
use exact_game_render::WorldSurface;
use exact_gpu::Surface;

struct Scene;
impl Game for Scene {
    const ID: &'static str = "trust";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("camera", (Transform::at(0., 2., 6.), Camera::default()));
        w.spawn_named("cube", (Transform::default(), Mesh::cube(1.)));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}

#[test]
fn the_baked_trust_decides_whether_look_requests_answer() {
    let production = std::env::var("EXACT_UPDATE_TRUST").as_deref() == Ok("production");
    assert_eq!(
        exact_game_render::development_build(),
        !production,
        "the rlib was built with another trust than this run's"
    );
    let mut s = WorldSurface::<Scene>::default();
    s.bind(&[], None).unwrap();
    s.agent(r#"{"op":"clock","now":0,"width":320,"height":180}"#)
        .unwrap();
    for request in [
        r#"{"op":"state","look":true}"#,
        r#"{"op":"state","draws":true}"#,
        r#"{"op":"state","entity":"cube","visible":true}"#,
        r#"{"op":"screenshot","form":"look","top":20}"#,
    ] {
        let reply = s.agent(request).unwrap();
        let refused =
            reply.contains("this production build (EXACT_UPDATE_TRUST=production) refuses");
        assert_eq!(refused, production, "{request}: {reply}");
    }
}
