//! @ref LLP 1139 D1, D3, D6, D8 (stage 12 C3) — what a library component
//! writes: its spacing, corners and colours are knobs, never literals
//! (`lower-library-literal`); a control's role is the native control's,
//! with what its platform needs (`lower-library-control`); nothing in a
//! library package reads the host (`lower-library-platform`); a style one
//! component alone applies reads its params (D3); and a library's names are
//! its API (`tamber-package-unknown`, `tamber-param-short-library`).

use contract::CompileError;
use std::path::{Path, PathBuf};

struct Dir(PathBuf);
impl Dir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("exact-librules-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const DESIGN: &str = "tamber lib\n\ntheme\n  lib.pad = 4px\n  lib.ink = #111111\n";

/// The errors of an app whose `app` view uses the library package
/// `@acme/lib`, its entry `index`, its app design `design`.
fn build(name: &str, index: &str, app: &str, design: &str) -> Vec<CompileError> {
    build_as(name, "@acme/lib", "@acme/lib", DESIGN, index, app, design)
}

/// [`build`], the package installed as `installed` under its own name
/// `package`, its design `lib_design`.
fn build_as(
    name: &str,
    installed: &str,
    package: &str,
    lib_design: &str,
    index: &str,
    app: &str,
    design: &str,
) -> Vec<CompileError> {
    let dir = Dir::new(name);
    let at = format!("app/node_modules/{installed}");
    let ns = package.rsplit('/').next().unwrap();
    dir.write(
        &format!("{at}/package.json"),
        &format!(r#"{{"name":"{package}","version":"1.0.0","private":true,"exports":{{".":"./index.contract"}}}}"#),
    );
    dir.write(&format!("{at}/index.contract"), index);
    dir.write(&format!("{at}/{ns}.tamber"), lib_design);
    dir.write("app/app.tamber", design);
    let path = dir.write("app/app.contract", app);
    let src = std::fs::read_to_string(&path).unwrap();
    match contract::compile_path_source_all(Path::new(&path), &src, false) {
        Ok(_) => Vec::new(),
        Err(all) => all,
    }
}

const APP: &str = "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n";
const PLAIN: &str = "tamber app\n";

fn ids<'e>(all: &'e [CompileError], id: &str) -> Vec<&'e CompileError> {
    all.iter().filter(|e| e.id == id).collect()
}

#[test]
fn a_librarys_knobs_are_never_literals() {
    let id = "lower-library-literal";
    let card =
        |rows: &str| format!("component Card\n  view\n    column {rows}\n      text \"a\"\n");
    // Tokens, a role, a base name and zero: knobs.
    let all = build(
        "knobs",
        &card("padding=\"lib.pad\" margin-top=\"space.md\" margin-left=\"auto\" color=\"lib.ink\" background-color=\"transparent\" border-color=\"currentcolor\" border-radius=\"0px\""),
        APP,
        PLAIN,
    );
    assert!(ids(&all, id).is_empty(), "{all:?}");
    for (name, rows) in [
        ("padding", "padding=\"12px\""),
        ("gap", "gap=\"8px\""),
        ("radius", "border-radius=\"6px\""),
        ("colour", "color=\"#ff0000\""),
        ("background", "background-color=\"#ffffff\""),
    ] {
        let all = build(&format!("lit-{name}"), &card(rows), APP, PLAIN);
        assert!(!ids(&all, id).is_empty(), "{name}: {all:?}");
    }
    // A shorthand is judged per longhand, as lowering splits it: `border`'s
    // width and style are no knobs. A named length, and a colour mix over
    // knobs, are knobs.
    let all = build(
        "split",
        &card("padding=\"0 lib.pad\" gap=\"0 lib.pad\" border-radius=\"lib.pad 0\" border=\"1px solid lib.ink\" margin-top=\"-exact-hairline\" background-color=\"color-mix(in srgb, lib.ink 40%, transparent)\""),
        APP,
        PLAIN,
    );
    assert!(all.is_empty(), "{all:?}");
    // A grouped calc over a base name, and a nested numeric one, as a
    // param's default.
    for (name, default) in [
        ("calc-group", "calc(space.md * (1 + 2))"),
        ("calc-nested", "calc(space.md * calc(1 + 2))"),
    ] {
        let all = build(
            name,
            &format!("component Card\n  params\n    pad: length = \"{default}\"\n  view\n    column padding=\"param.pad\"\n      text \"a\"\n"),
            APP,
            PLAIN,
        );
        assert!(all.is_empty(), "{name}: {all:?}");
    }
    for (name, rows, said) in [
        ("percent", "padding=\"calc(50%)\"", "calc(50%)"),
        ("percent-beside", "padding=\"calc(100% - lib.pad)\"", "100%"),
        ("side", "padding=\"0 12px\"", "12px"),
        ("border", "border=\"1px solid #ff0000\"", "#ff0000"),
        (
            "mix",
            "background-color=\"color-mix(in srgb, lib.ink 40%, white)\"",
            "color-mix",
        ),
        ("negative", "margin-top=-12", "-12"),
    ] {
        let all = build(&format!("lit-{name}"), &card(rows), APP, PLAIN);
        assert!(
            ids(&all, id).iter().any(|e| e.message.contains(said)),
            "{name}: {all:?}"
        );
    }
    // A class's row, and a param's default.
    let all = build(
        "lit-class",
        &(card("class=Loud") + "\nstyle Loud\n  background-color = \"#ff0000\"\n"),
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id)
            .iter()
            .any(|e| e.message.contains("style Loud")),
        "{all:?}"
    );
    // Every kind's default: a duration's too, a token's name passing.
    let all = build(
        "lit-duration",
        "component Card\n  params\n    fade: duration = \"200ms\"\n  view\n    column transition-duration=\"param.fade\"\n      text \"a\"\n",
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id)
            .iter()
            .any(|e| e.message.contains("param `fade`")),
        "{all:?}"
    );
    let all = build(
        "lit-param",
        "component Card\n  params\n    pad: length = \"10px\"\n  view\n    column padding=\"param.pad\"\n      text \"a\"\n",
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id)
            .iter()
            .any(|e| e.message.contains("param `pad`")),
        "{all:?}"
    );
}

#[test]
fn a_controls_role_is_the_native_controls() {
    let id = "lower-library-control";
    let card = |body: &str| {
        format!("component Card\n  state on = false\n  state v = \"\"\n  view\n    column\n{body}")
    };
    for (name, body, said) in [
        (
            "role",
            "      view role=\"switch\"\n",
            "drawn as `role=\"switch\"`",
        ),
        (
            "checked",
            "      input type=\"checkbox\" aria-label=\"On\"\n",
            "`checked`",
        ),
        (
            "label",
            "      input type=\"checkbox\" checked=on\n",
            "`aria-label` or `aria-labelledby`",
        ),
        (
            "range",
            "      input type=\"range\" value=v aria-label=\"R\"\n",
            "`min`",
        ),
        ("textarea", "      textarea aria-label=\"T\"\n", "`value`"),
        ("progress", "      progress\n", "`aria-label`"),
    ] {
        let all = build(&format!("ctl-{name}"), &card(body), APP, PLAIN);
        assert!(
            ids(&all, id).iter().any(|e| e.message.contains(said)),
            "{name}: {all:?}"
        );
    }
    // `aria-labelledby` names a checkbox (Contract has no `label`);
    // `type` is any case; a file input needs nothing of the table. The
    // whole build passes.
    let all = build(
        "ctl-ok",
        &card("      text \"On\" id=\"on-label\"\n      input type=\"CHECKBOX\" checked=on aria-labelledby=\"on-label\"\n      input type=\"file\" aria-label=\"Upload\"\n"),
        APP,
        PLAIN,
    );
    assert!(all.is_empty(), "{all:?}");
    // Each type a choice may be is held to its row.
    let all = build(
        "ctl-type-choice",
        &card("      input type=(on ? \"checkbox\" : \"text\") checked=on aria-label=\"x\"\n"),
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id).iter().any(|e| e.message.contains("`value`")),
        "{all:?}"
    );
    let all = build(
        "ctl-type-both",
        &card("      input type=(on ? \"checkbox\" : \"radio\") checked=on aria-label=\"x\"\n"),
        APP,
        PLAIN,
    );
    assert!(ids(&all, id).is_empty(), "{all:?}");
    // A role a template writes, or a choice may take, is checked as one
    // written.
    let all = build(
        "ctl-template",
        &card("      view role=`checkbox`\n"),
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id).iter().any(|e| e.message.contains("drawn as")),
        "{all:?}"
    );
    let all = build(
        "ctl-choice",
        &card("      view role=(on ? \"checkbox\" : \"radio\")\n"),
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id).iter().any(|e| e.message.contains("drawn as")),
        "{all:?}"
    );
    // A determinate progress shows `value` of `max`.
    let all = build(
        "ctl-max",
        &card("      progress value=0.5 aria-label=\"P\"\n"),
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id).iter().any(|e| e.message.contains("`max`")),
        "{all:?}"
    );
    // A button with a control's role is judged where lowering decides how
    // it draws: native, it passes; a bare box, it is refused.
    let all = build(
        "ctl-button",
        &card("      button \"A\" role=\"radio\"\n"),
        APP,
        PLAIN,
    );
    assert!(all.is_empty(), "{all:?}");
    let all = build(
        "ctl-bare",
        &card("      button \"A\" role=\"radio\" background-color=\"lib.ink\"\n"),
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id).iter().any(|e| e.message.contains("bare box")),
        "{all:?}"
    );
}

#[test]
fn a_library_package_never_reads_the_host() {
    let id = "lower-library-platform";
    let all = build(
        "host",
        "shape Delivery\n  stream: string\n\ncomponent Card\n  resource delivery = exactDelivery() as shape Delivery\n  view\n    text delivery.stream\n",
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id)
            .iter()
            .any(|e| e.message.contains("exactDelivery")),
        "{all:?}"
    );
    let all = build(
        "display",
        "shape Viewport\n  displayMode: string\n  pointer: string\n\ncomponent Card\n  resource vp = exactViewport() as shape Viewport\n  view\n    text vp.displayMode\n",
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id)
            .iter()
            .any(|e| e.message.contains("displayMode")),
        "{all:?}"
    );
    // Read through an alias, or in an action.
    let viewport = "shape Viewport\n  displayMode: string\n  pointer: string\n\ncomponent Card\n  resource vp = exactViewport() as shape Viewport\n";
    for (name, body) in [
        ("alias", "  derive v = vp\n  derive m = v.displayMode\n  view\n    text m\n"),
        ("action", "  state m = \"\"\n  action read\n    m = vp.displayMode\n  view\n    button \"r\" press=read\n"),
    ] {
        let all = build(&format!("display-{name}"), &format!("{viewport}{body}"), APP, PLAIN);
        assert!(
            ids(&all, id)
                .iter()
                .any(|e| e.message.contains("displayMode")),
            "{name}: {all:?}"
        );
    }
    // A `fn` the viewport is passed to, and a `provide` a child injects.
    for (name, src) in [
        ("fn", format!("{viewport}  view\n    text mode(vp)\n\nfn mode(v: Viewport): string = v.displayMode\n")),
        ("inject", format!("{viewport}  provide\n    view = vp\n  view\n    Mode()\n\ncomponent Mode\n  inject\n    view: Viewport\n  view\n    text view.displayMode\n")),
    ] {
        let all = build(&format!("display-{name}"), &src, APP, PLAIN);
        assert!(
            ids(&all, id)
                .iter()
                .any(|e| e.message.contains("displayMode")),
            "{name}: {all:?}"
        );
    }
    // Through a `fn` that passes it on or returns it, and an action's
    // parameter.
    for (name, src) in [
        ("relay", format!("{viewport}  view\n    text relay(vp)\n\nfn mode(v: Viewport): string = v.displayMode\n\nfn relay(v: Viewport): string = mode(v)\n")),
        ("same", format!("{viewport}  view\n    text same(vp).displayMode\n\nfn same(v: Viewport): Viewport = v\n")),
        ("param", format!("{viewport}  state m = \"\"\n  action show(v: Viewport)\n    m = v.displayMode\n  view\n    button \"r\" press=show(vp)\n")),
    ] {
        let all = build(&format!("display-{name}"), &src, APP, PLAIN);
        assert!(
            ids(&all, id)
                .iter()
                .any(|e| e.message.contains("displayMode")),
            "{name}: {all:?}"
        );
    }
    // A `match`'s binding of it.
    let all = build(
        "display-match",
        &format!("{viewport}  view\n    text (match some(vp) {{ case some(x) => x.displayMode, case none => \"\" }})\n"),
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id)
            .iter()
            .any(|e| e.message.contains("displayMode")),
        "{all:?}"
    );
    // A `let` is its own scope's: `v` the viewport in one action is not
    // `v` in another. A nearer provider of another value hides the root's.
    let scoped = format!("shape Item\n  displayMode: string\n\n{viewport}  props\n    item: Item\n  state m = \"\"\n  action a\n    let v = vp\n    m = v.pointer\n  action b\n    let v = item\n    m = v.displayMode\n  view\n    button \"a\" press=a\n    button \"b\" press=b\n");
    let app = "use Card, Item from \"@acme/lib\"\n\ncomponent App\n  view\n    Card(item=Item(displayMode=\"x\"))\n";
    let all = build("display-scoped", &scoped, app, PLAIN);
    assert!(ids(&all, id).is_empty(), "{all:?}");
    let nearer = format!("{viewport}  provide\n    view = vp\n  view\n    Middle(alt=Viewport(displayMode=\"a\", pointer=\"b\"))\n\ncomponent Middle\n  props\n    alt: Viewport\n  provide\n    view = alt\n  view\n    Mode()\n\ncomponent Mode\n  inject\n    view: Viewport\n  view\n    text view.displayMode\n");
    let all = build("display-nearer", &nearer, APP, PLAIN);
    assert!(ids(&all, id).is_empty(), "{all:?}");
    // Without the nearer one, the root's reaches it.
    let far = nearer.replace("  provide\n    view = alt\n", "");
    let all = build("display-far", &far, APP, PLAIN);
    assert!(
        ids(&all, id)
            .iter()
            .any(|e| e.message.contains("displayMode")),
        "{all:?}"
    );
    // A slot's fill keeps its writer's providers: a slot providing another
    // value doesn't hide the writer's viewport.
    let fill = format!("{viewport}  provide\n    view = vp\n  view\n    Frame(alt=Viewport(displayMode=\"a\", pointer=\"b\"))\n      Mode()\n\ncomponent Frame\n  slot\n  props\n    alt: Viewport\n  provide\n    view = alt\n  view\n    children\n\ncomponent Mode\n  inject\n    view: Viewport\n  view\n    text view.displayMode\n");
    let all = build("display-fill", &fill, APP, PLAIN);
    assert!(
        ids(&all, id)
            .iter()
            .any(|e| e.message.contains("displayMode")),
        "{all:?}"
    );
    // A relay of any depth.
    // Each `fn` passes it to the one before: a summary a pass deeper each
    // time, past any fixed count of passes.
    let mut fns = String::from("fn r0(v: Viewport): string = v.displayMode\n");
    for i in 1..20 {
        fns += &format!("\nfn r{i}(v: Viewport): string = r{}(v)\n", i - 1);
    }
    let all = build(
        "display-deep",
        &format!("{viewport}  view\n    text r19(vp)\n\n{fns}"),
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, id)
            .iter()
            .any(|e| e.message.contains("displayMode")),
        "{all:?}"
    );
    // The scoped `let`s, with `b` reading the viewport's: refused.
    let all = build(
        "display-scoped-bad",
        &scoped.replace("let v = item", "let v = vp"),
        app,
        PLAIN,
    );
    assert!(!ids(&all, id).is_empty(), "{all:?}");
    // A prop's `displayMode` is the app's data.
    let all = build(
        "display-prop",
        "shape Item\n  displayMode: string\n\ncomponent Card\n  props\n    item: Item\n  view\n    text item.displayMode\n",
        "use Card, Item from \"@acme/lib\"\n\ncomponent App\n  view\n    Card(item=Item(displayMode=\"compact\"))\n",
        PLAIN,
    );
    assert!(all.is_empty(), "{all:?}");
    // A capability is no platform.
    let all = build(
        "pointer",
        "shape Viewport\n  pointer: string\n\ncomponent Card\n  resource vp = exactViewport() as shape Viewport\n  view\n    text vp.pointer\n",
        APP,
        PLAIN,
    );
    assert!(ids(&all, id).is_empty(), "{all:?}");
}

#[test]
fn a_style_one_component_alone_applies_reads_its_params() {
    let card = "component Card\n  props\n    tone: enum<\"plain\", \"quiet\">\n  params\n    fill: color = \"lib.ink\"  control=\"color\"\n    quiet: color = \"-exact-secondary-background\"  control=\"color\"\n  view\n    column class=(tone == \"quiet\" ? Quiet : Plain)\n      text \"a\"\n\nstyle Plain\n  background-color = \"param.fill\"\n\nstyle Quiet\n  background-color = \"param.quiet\"\n";
    let app = "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card(tone=\"plain\")\n";
    let all = build("owned", card, app, PLAIN);
    assert!(all.is_empty(), "{all:?}");
    // A second component applying it: no one's params.
    let shared =
        card.to_string() + "\ncomponent Other\n  view\n    column class=Plain\n      text \"b\"\n";
    let all = build("shared", &shared, app, PLAIN);
    assert!(!ids(&all, "tamber-param-scope").is_empty(), "{all:?}");
    // A param its component doesn't declare.
    let unknown = card.replace("param.quiet", "param.dim");
    let all = build("unknown", &unknown, app, PLAIN);
    assert!(!ids(&all, "tamber-param-unknown").is_empty(), "{all:?}");
}

#[test]
fn a_librarys_names_are_its_api() {
    let card = "component Card\n  params\n    pad: length = \"lib.pad\"\n  view\n    column padding=\"param.pad\"\n      text \"a\"\n";
    // The app setting a name the package defines: its override.
    let all = build(
        "override",
        card,
        APP,
        "tamber app\n\ntheme\n  lib.pad = 8px\n",
    );
    assert!(all.is_empty(), "{all:?}");
    // One it doesn't define.
    let all = build(
        "unknown-token",
        card,
        APP,
        "tamber app\n\ntheme\n  lib.gap = 8px\n",
    );
    assert!(!ids(&all, "tamber-package-unknown").is_empty(), "{all:?}");
    // A param line under the package with no such param.
    let all = build(
        "unknown-param",
        card,
        APP,
        "tamber app\n\nparams\n  @acme/lib/index.contract#Card.gap = 8px\n",
    );
    assert!(!ids(&all, "tamber-package-unknown").is_empty(), "{all:?}");
    // A library param by its short name.
    let all = build(
        "short",
        card,
        APP,
        "tamber app\n\nparams\n  Card.pad = 8px\n",
    );
    assert!(
        ids(&all, "tamber-param-short-library")
            .iter()
            .any(|e| e.message.contains("@acme/lib/index.contract#Card.pad")),
        "{all:?}"
    );
    // A short name naming no param of a library component: an error, never
    // the warning a rename would fall back to.
    let all = build(
        "short-gone",
        card,
        APP,
        "tamber app\n\nparams\n  Card.gap = 8px\n",
    );
    assert!(
        !ids(&all, "tamber-param-short-library").is_empty(),
        "{all:?}"
    );
    assert!(ids(&all, "tamber-param-unknown").is_empty(), "{all:?}");
    // A family name in quotes and an SVG fragment name no token; a
    // template no value fills is read as a string is.
    let ok = card.replace(
        "padding=\"param.pad\"",
        "padding=\"param.pad\" font-family=\"'lib.custom', system-ui\"",
    );
    let all = build("row-quoted", &ok, APP, PLAIN);
    assert!(ids(&all, "tamber-package-unknown").is_empty(), "{all:?}");
    let all = build(
        "row-template",
        &card.replace(
            "padding=\"param.pad\"",
            "padding=\"param.pad\" margin-top=`lib.gone`",
        ),
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, "tamber-package-unknown")
            .iter()
            .any(|e| e.message.contains("lib.gone")),
        "{all:?}"
    );
    // A choice's condition is no row value, and a name a hole completes
    // (`lib.${tone}`) is the hole's.
    let all = build(
        "row-cond",
        &card.replace("  view\n", "  state tone = \"ink\"\n  view\n").replace("padding=\"param.pad\"", "padding=\"param.pad\" margin-top=(tone == \"lib.primary\" ? \"space.md\" : \"space.sm\") color=`lib.${tone}`"),
        APP,
        PLAIN,
    );
    assert!(ids(&all, "tamber-package-unknown").is_empty(), "{all:?}");
    // A choice inside a concatenation is spelled out.
    let all = build(
        "row-concat",
        &card
            .replace("  view\n", "  state on = false\n  view\n")
            .replace(
                "padding=\"param.pad\"",
                "padding=\"param.pad\" color=(\"\" + (on ? \"lib.gone\" : \"lib.ink\"))",
            ),
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, "tamber-package-unknown")
            .iter()
            .any(|e| e.message.contains("lib.gone")),
        "{all:?}"
    );
    // Fragments a `+` glues are no names of their own.
    let all = build(
        "row-glued",
        &card
            .replace("  view\n", "  state on = false\n  view\n")
            .replace(
            "padding=\"param.pad\"",
            "padding=\"param.pad\" color=((on ? \"lib.\" : \"\") + (on ? \"ink\" : \"lib.ink\"))",
        ),
        APP,
        PLAIN,
    );
    assert!(ids(&all, "tamber-package-unknown").is_empty(), "{all:?}");
    // An empty side passes on its neighbour's glue; a quote opened in one
    // side closes in another.
    let all = build(
        "row-empty-quote",
        &card.replace("  view\n", "  state on = false\n  view\n").replace("padding=\"param.pad\"", "padding=\"param.pad\" color=((\"lib.in\" + \"\") + \"k\") font-family=(\"'\" + (on ? \"lib.custom\" : \"lib.other\") + \"', system-ui\") -exact-tint-color=((on ? \"'\" : \"\") + (on ? \"Family'\" : \"'lib.custom'\"))"),
        APP,
        PLAIN,
    );
    assert!(ids(&all, "tamber-package-unknown").is_empty(), "{all:?}");
    // A template's text around its holes is read too.
    let all = build(
        "row-hole",
        &card.replace("  view\n", "  state share = 40\n  view\n").replace("padding=\"param.pad\"", "padding=\"param.pad\" background-color=`color-mix(in srgb, lib.gone ${share}%, transparent)`"),
        APP,
        PLAIN,
    );
    assert!(
        ids(&all, "tamber-package-unknown")
            .iter()
            .any(|e| e.message.contains("lib.gone")),
        "{all:?}"
    );
    // A Contract row naming a name the package doesn't define.
    let row = card.replace(
        "padding=\"param.pad\"",
        "padding=\"param.pad\" margin-top=\"lib.gap\"",
    );
    let all = build("unknown-row", &row, APP, PLAIN);
    assert!(
        ids(&all, "tamber-package-unknown")
            .iter()
            .any(|e| e.message.contains("lib.gap")),
        "{all:?}"
    );
    // Its icons are names it defines too (LLP 1121 stage 13): the app's
    // icon overriding one builds, one under its namespace it lacks is
    // refused.
    let icons = format!("{DESIGN}\nicons\n  lib.star = icon(sf \"star\")\n");
    let drawn = card.replace("      text \"a\"\n", "      image \"symbol:lib.star\"\n");
    for (name, app_icons, refused) in [
        ("icon-override", "lib.star", false),
        ("icon-unknown", "lib.gone", true),
    ] {
        let design = format!("tamber app\n\nicons\n  {app_icons} = icon(sf \"heart\")\n");
        let all = build_as(name, "@acme/lib", "@acme/lib", &icons, &drawn, APP, &design);
        let unknown = ids(&all, "tamber-package-unknown");
        assert_eq!(!unknown.is_empty(), refused, "{name}: {all:?}");
        assert!(
            unknown.iter().all(|e| e.message.contains("lib.gone")),
            "{all:?}"
        );
    }
}

#[test]
fn a_package_is_its_own_name_and_base_names_are_no_packages() {
    // Installed under another name: the checks read the package's own.
    let alias = "use Card from \"books\"\n\ncomponent App\n  view\n    Card()\n";
    let card = "component Card\n  view\n    column padding=\"12px\"\n      text \"a\"\n";
    let all = build_as(
        "alias",
        "books",
        "@acme/lib",
        DESIGN,
        card,
        alias,
        "tamber app\n\ntheme\n  lib.gap = 8px\n",
    );
    assert!(!ids(&all, "lower-library-literal").is_empty(), "{all:?}");
    assert!(!ids(&all, "tamber-package-unknown").is_empty(), "{all:?}");
    // A package named `space`: `space.md` stays the app's base override.
    let space = "use Card from \"@acme/space\"\n\ncomponent App\n  view\n    Card()\n";
    let card = "component Card\n  view\n    column padding=\"space.pad\"\n      text \"a\"\n";
    let all = build_as(
        "space",
        "@acme/space",
        "@acme/space",
        "tamber space\n\ntheme\n  space.pad = 4px\n",
        card,
        space,
        "tamber app\n\ntheme\n  space.md = 20px\n",
    );
    assert!(ids(&all, "tamber-package-unknown").is_empty(), "{all:?}");
}

/// A library view's buttons stand in for platform controls (LLP 1139 D9's
/// condition (b), theme2 2026-10-22 04:40): each is named, and one whose
/// look reads a state (`selected`, `pressed`, …) says it aloud, through an
/// `aria-*` state or a label that reads it; a presentational bool (a size,
/// an ink) needs nothing.
#[test]
fn a_library_button_is_named_and_says_the_state_its_look_shows() {
    let id = "lower-library-control";
    let card = |body: &str| {
        format!("component Card\n  props\n    selected: bool\n    small: bool\n    go: action\n  derive lit = selected\n  derive spoken = (selected ? \"Selected\" : \"Select\")\n  derive fade = (selected ? 1 : 0.5)\n  derive fade2 = (lit ? 1 : 0.5)\n  view\n    column\n{body}")
    };
    let app = "use Card from \"@acme/lib\"\n\ncomponent App\n  action go\n  view\n    Card(selected=true, small=false, go=go)\n";
    for (name, body, said) in [
        (
            "unnamed",
            "      button press=go\n        box width=4 height=4\n",
            "without a name",
        ),
        (
            "unnamed-role",
            "      row role=\"button\" press=go\n        box width=4 height=4\n",
            "without a name",
        ),
        (
            "unsaid",
            "      button press=go aria-label=\"Home\" opacity=(selected ? 1 : 0.5)\n        box width=4 height=4\n",
            "doesn't say aloud: its look reads `selected`",
        ),
        (
            "empty-label",
            "      button press=go aria-label=\"\"\n        box width=4 height=4\n",
            "without a name",
        ),
        (
            "hidden-text",
            "      button press=go\n        text \"Go\" aria-hidden=true\n",
            "without a name",
        ),
        (
            "one-arm",
            "      button press=go\n        if small\n          text \"Go\"\n",
            "without a name",
        ),
        (
            "derived-look",
            "      button press=go aria-label=\"Home\" opacity=(lit ? 1 : 0.5)\n        box width=4 height=4\n",
            "its look reads `selected`",
        ),
        (
            "derived-fade",
            "      button press=go aria-label=\"Home\" opacity=fade\n        box width=4 height=4\n",
            "its look reads `selected`",
        ),
        (
            "derive-under-binding",
            "      for selected in [\"a\"] key = selected\n        button press=go aria-label=\"Row\" opacity=(lit ? 1 : 0.5)\n          box width=4 height=4\n",
            "its look reads `selected`",
        ),
        (
            "text-one-arm",
            "      button press=go opacity=(selected ? 1 : 0.5)\n        if small\n          text (selected ? \"On\" : \"Off\")\n        else\n          text \"Home\"\n",
            "doesn't say aloud",
        ),
        (
            "label-wins",
            "      button press=go aria-label=\"Home\" opacity=(selected ? 1 : 0.5)\n        text (selected ? \"On\" : \"Off\")\n",
            "doesn't say aloud",
        ),
        (
            "hidden-text-says",
            "      button press=go opacity=(selected ? 1 : 0.5)\n        text \"Home\"\n        text (selected ? \"On\" : \"Off\") aria-hidden=true\n",
            "doesn't say aloud",
        ),
        (
            "labelledby-wins",
            "      text \"Home\" id=\"home-name\"\n      button press=go aria-labelledby=\"home-name\" aria-label=(selected ? \"On\" : \"Off\") opacity=(selected ? 1 : 0.5)\n        box width=4 height=4\n",
            "doesn't say aloud",
        ),
        (
            "derive-met-twice",
            "      button press=go aria-label=\"Home\" opacity=([lit].length * fade2)\n        box width=4 height=4\n",
            "its look reads `selected`",
        ),
        (
            "label-own-binding",
            "      for selected in [\"row\"] key = selected\n        text selected id=\"bound-name\"\n        button press=go aria-labelledby=\"bound-name\" opacity=fade\n          box width=4 height=4\n",
            "doesn't say aloud",
        ),
        (
            "unsaid-inside",
            "      button press=go aria-label=\"Home\"\n        box width=4 height=4 opacity=(selected ? 1 : 0.5)\n",
            "doesn't say aloud",
        ),
    ] {
        let all = build(&format!("btn-{name}"), &card(body), app, PLAIN);
        assert!(
            ids(&all, id).iter().any(|e| e.message.contains(said)),
            "{name}: {all:?}"
        );
    }
    // Said by aria-selected, by a label that reads it, a text names a
    // button, and a presentational bool needs nothing: the build passes.
    let all = build(
        "btn-ok",
        &card("      button press=go aria-label=\"Home\" aria-selected=selected opacity=(selected ? 1 : 0.5)\n        box width=4 height=4\n      button press=go aria-label=(selected ? \"Undo\" : \"Do\") opacity=(selected ? 1 : 0.5)\n        box width=4 height=4\n      button press=go\n        text \"Go\"\n      button press=go aria-label=\"Back\" opacity=(small ? 0.5 : 1)\n        box width=4 height=4\n      button press=go aria-label=spoken opacity=(selected ? 1 : 0.5)\n        box width=4 height=4\n      button press=go aria-label=\"\" opacity=(selected ? 1 : 0.5)\n        text (selected ? \"On\" : \"Off\")\n      text (selected ? \"Mine\" : \"Theirs\") id=\"state-name\"\n      button press=go aria-labelledby=\"state-name\" opacity=(selected ? 1 : 0.5)\n        box width=4 height=4\n      button press=go aria-labelledby=\"\" aria-label=spoken opacity=(selected ? 1 : 0.5)\n        box width=4 height=4\n      for selected in [\"row\"] key = selected\n        button press=go aria-labelledby=\"state-name\" opacity=fade\n          box width=4 height=4\n      if small\n        text (selected ? \"A\" : \"B\") id=\"twice\"\n      else\n        text \"C\" id=\"twice\"\n      button press=go aria-labelledby=\"twice\" opacity=(selected ? 1 : 0.5)\n        box width=4 height=4\n      for selected in [\"a\"] key = selected\n        button press=go aria-label=\"Row\" opacity=(selected == \"a\" ? 1 : 0.5)\n          box width=4 height=4\n"),
        app,
        PLAIN,
    );
    assert!(all.is_empty(), "{all:?}");
    // Named by its caller's content (the slot) or a used component's text,
    // a state said by its own text, and an id that reads a state (no look):
    // the build passes (reload1's review).
    let all = build(
        "btn-ok-more",
        "component Label\n  props\n    text: string\n  view\n    text text\n\ncomponent Card\n  props\n    selected: bool\n    go: action\n  slot\n  view\n    column\n      button press=go\n        children\n      button press=go\n        Label(text=\"Go\")\n      button press=go opacity=(selected ? 1 : 0.5)\n        text (selected ? \"Unfollow\" : \"Follow\")\n      button press=go aria-label=\"Home\" id=(selected ? \"on\" : \"off\")\n        box width=4 height=4\n",
        "use Card from \"@acme/lib\"\n\ncomponent App\n  action go\n  view\n    Card(selected=true, go=go)\n      text \"x\"\n",
        PLAIN,
    );
    assert!(all.is_empty(), "{all:?}");
}
