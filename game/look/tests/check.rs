//! What the checker refuses, with where and why, against a world's registered
//! types: names, fields, enum arms, rows, order and types.
use exact_game::{Component, Data, Registered, World};
use exact_game_look::{Externs, Look};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
enum Mood {
    #[default]
    Calm,
    Angry,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
struct Critter {
    mood: Mood,
    speed: f32,
}

fn types() -> Vec<Registered> {
    let mut w = World::new(60, 1);
    w.register::<Critter>();
    w.registered()
}

fn refused(src: &str) -> String {
    let externs = Externs::new().constant("CELL", 5.0);
    match Look::compile(src, &types(), &externs) {
        Ok(_) => panic!("accepted:\n{src}"),
        Err(errors) => errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn accepted(src: &str) -> Look {
    Look::compile(src, &types(), &Externs::new()).unwrap_or_else(|e| panic!("{e:?}"))
}

#[test]
fn a_well_formed_look_is_accepted_and_reports_its_reads() {
    let look = accepted(
        "let t = seconds()\n\
         each e, c in Critter\n  \
           if c.mood is Angry\n    \
             Offset(e, rotation=pitch(c.speed * sin(t * 4)))\n  \
           else\n    \
             Opacity(e, 0.5)\n",
    );
    let report = look.report();
    assert!(report.contains("reads [Critter]"), "{report}");
    assert!(
        report.contains("GPU-lowerable `pitch(p0 * sin(t * 4))`"),
        "{report}"
    );
    assert_eq!(look.rules().len(), 1);
}

#[test]
fn unknown_names_fields_and_arms_are_refused_with_their_place() {
    let e = refused("each e, c in Critters\n  Opacity(e, 1)\n");
    assert!(
        e.starts_with("1:1:")
            && e.contains("no component `Critters`")
            && e.contains("did you mean `Critter`"),
        "{e}"
    );
    let e = refused("each e, c in Critter\n  Opacity(e, c.sped)\n");
    assert!(
        e.contains("2:15:") && e.contains("no field `sped`") && e.contains("mood, speed"),
        "{e}"
    );
    let e = refused("each e, c in Critter\n  if c.mood is Furious\n    Opacity(e, 1)\n");
    assert!(
        e.contains("`Furious` is not an arm of Critter.mood (its default is `Calm`)"),
        "{e}"
    );
}

#[test]
fn a_look_writes_presentation_rows_and_reads_only_the_simulation() {
    let e = refused("each e, c in Critter\n  Transform(e, 1)\n");
    assert!(
        e.contains("`Transform` is not a row a look writes (Offset, Opacity)"),
        "{e}"
    );
    let e = refused("each e, c in Critter\n  Opacity(e, e.Opacity)\n");
    assert!(
        e.contains("`Opacity` is a presentation component; a look writes it, never reads it"),
        "{e}"
    );
}

#[test]
fn types_order_and_constants_are_checked() {
    let e = refused("each e, c in Critter\n  Opacity(e, c.mood)\n");
    assert!(e.contains("expected a number, found an enum"), "{e}");
    let e = refused("fn a(x: number) = b(x)\nfn b(x: number) = x\n");
    assert!(e.contains("no function `b`"), "{e}");
    let e = refused("let n = CELL * 2\nlet n = 3\n");
    assert!(e.contains("`n` is already defined"), "{e}");
    let e = refused("each e, c in Critter\n  Offset(e, rotation=c.speed)\n");
    assert!(e.contains("expected a quat, found a number"), "{e}");
}
