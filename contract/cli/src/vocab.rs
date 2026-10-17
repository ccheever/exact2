//! `contract vocab [--json] [<name> …]` — what the compiler admits: every tag,
//! every attribute by kind (a style's rows with codec, values and default; a
//! prop's type; a handler), the renamed spellings, and the rules no lookup
//! lists, the host commands an action states, the standard functions:
//! each one's signature, rules and example (LLP 1116.000 D5), and the
//! idioms a builder looks up by what they want (`plural`, LLP 1116.004 D4). @ref LLP 1086
//! D3. The list is `contract_lower::vocab`'s, which keeps only what the live
//! lookups admit; the commands are `contract_syntax::HOST_COMMANDS`; the
//! functions are `contract::functions`', the roster's signatures beside the
//! grammar's tables. Several names are one call, each entry in turn (LLP
//! 1116.001 D4.3), so a build's lookups are one step. A concept (`scroll`,
//! `plural`, `choice`, `modal`) answers with its whole idiom, after the
//! entries of a tag or attribute of the same name (`concepts.md`, LLP
//! 1116.004 D5.1).

mod card;
mod concepts;
mod recipes;

use concepts::Concept;
use contract::functions::{self, Function};
use contract_lower::tags::{self, AttrTarget, Tag};
use contract_lower::vocab;
use contract_syntax::HOST_COMMANDS;
use exact_kernel::{PropId, StyleId};
use serde_json::{json, Value};
use std::process::ExitCode;

const USAGE: &str = "usage: contract vocab [--json] [--all] [<name> …]\n  no name: the card, what an app writes, in one read; --all: every tag, attribute and function\n  names: each one's entry in turn, a recipe (docs/recipes) whole, a pattern's summary (exit 1 if one is unknown); --json: everything, as JSON";

pub fn run(args: &[String]) -> ExitCode {
    let mut json = false;
    let mut all = false;
    let mut names = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--json" => json = true,
            "--all" => all = true,
            // A vendor-prefixed name (`-exact-fit`, `-webkit-line-clamp`) is a name, not a flag.
            _ if !arg.starts_with('-') || (arg.len() > 2 && !arg.starts_with("--")) => {
                names.push(arg.as_str())
            }
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    match names.as_slice() {
        [] if json => {
            println!("{:#}", everything());
            ExitCode::SUCCESS
        }
        [] if all => {
            print!("{}", listing());
            ExitCode::SUCCESS
        }
        [] => {
            print!("{}", card::card());
            ExitCode::SUCCESS
        }
        [name] if json => {
            let (doc, known) = one_json(name);
            println!("{doc:#}");
            exit(known)
        }
        // Several names: each entry in order, a refusal in its place, and exit 1
        // when any name is unknown (LLP 1116.001 D4.3). JSON is an array of the
        // entries one name prints.
        _ if json => {
            let (docs, known): (Vec<Value>, Vec<bool>) = names.iter().map(|n| one_json(n)).unzip();
            println!("{:#}", Value::Array(docs));
            exit(known.iter().all(|k| *k))
        }
        _ => {
            let mut known = true;
            // A concept or recipe two names share prints once, at the first.
            let mut shown = Shown::default();
            for (i, name) in names.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                known &= one_text(name, &mut shown);
            }
            exit(known)
        }
    }
}

/// Exit 0 when every name was known, else 1.
fn exit(known: bool) -> ExitCode {
    if known {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// One name's entries, for its text and its JSON: its tag and attribute
/// (`input` is both), the style attributes it is a value of (`tabular-nums`),
/// an open set's rule, a host command, its function (`filter` is an attribute
/// and a function); `None` when nothing admits it.
struct Entry {
    tag: Option<Value>,
    attr: Option<Value>,
    value_of: Vec<&'static str>,
    open: Option<&'static str>,
    command: Option<String>,
    function: Option<Function>,
    part: Option<Part>,
    concept: Option<Concept>,
    recipe: Option<(&'static str, &'static str)>,
    pattern: Option<(&'static str, String)>,
}

/// The concepts and recipes printed so far in one call.
#[derive(Default)]
struct Shown {
    concepts: Vec<&'static str>,
    recipes: Vec<&'static str>,
}

/// A component of a built-in library (`exact:content`'s `RoundButton`), read
/// from its source: its props, the comment above it (LLP 1116.003 D4), and the
/// call that comment ends with (`// Call: EmptyState(…)`, LLP 1116.010 D3): one
/// that works as written, which a test compiles for every part, so it cannot
/// drift from the props.
struct Part {
    module: &'static str,
    props: Vec<String>,
    about: String,
    call: Option<String>,
}

const PARTS: &[(&str, &str)] = &[("exact:content", include_str!("../../lib/content.contract"))];

fn part(name: &str) -> Option<Part> {
    PARTS.iter().find_map(|(module, source)| {
        let lines: Vec<&str> = source.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.strip_prefix("component ") == Some(name))?;
        let first = lines[..at]
            .iter()
            .rposition(|l| !l.starts_with("//"))
            .map_or(0, |i| i + 1);
        let comment: Vec<&str> = lines[first..at]
            .iter()
            .map(|l| l.trim_start_matches('/').trim())
            .collect();
        let call = comment
            .iter()
            .find_map(|l| l.strip_prefix("Call:"))
            .map(|c| c.trim().to_owned());
        let about: Vec<&str> = comment
            .into_iter()
            .filter(|l| !l.starts_with("Call:"))
            .collect();
        let mut props = Vec::new();
        let mut rest = lines[at + 1..].iter().skip_while(|l| l.trim() != "props");
        if rest.next().is_some() {
            props = rest
                .take_while(|l| l.starts_with("    "))
                .map(|l| l.trim().to_owned())
                .collect();
        }
        Some(Part {
            module,
            props,
            about: about.join(" "),
            call,
        })
    })
}

fn entry(name: &str) -> Option<Entry> {
    let tag = tags::tag(name).map(|t| tag_json(name, &t));
    let attr = tags::attr(name).map(|a| attr_json(name, &a));
    let value_of: Vec<&'static str> = vocab::attrs()
        .into_iter()
        .filter(|(n, a)| rows_of(n, a).iter().any(|r| r.enum_names().contains(&name)))
        .map(|(n, _)| n)
        .collect();
    let concept = concepts::concept(name);
    let recipe = recipes::recipe(name);
    let pattern = recipes::pattern(name);
    // A concept's own name (`placeholder-color`) is answered by the concept,
    // and a recipe's by the recipe, not by the open set's note that it might
    // be a native module.
    let open = vocab::open_set(name).filter(|_| {
        tag.is_none()
            && attr.is_none()
            && value_of.is_empty()
            && concept.is_none()
            && recipe.is_none()
            && pattern.is_none()
    });
    let command = HOST_COMMANDS.contains(&name).then(|| command_text(name));
    let function = functions::function(name);
    let part = part(name);
    let known = tag.is_some()
        || attr.is_some()
        || !value_of.is_empty()
        || open.is_some()
        || command.is_some()
        || function.is_some()
        || part.is_some()
        || concept.is_some()
        || recipe.is_some()
        || pattern.is_some();
    known.then_some(Entry {
        tag,
        attr,
        value_of,
        open,
        command,
        function,
        part,
        concept,
        recipe,
        pattern,
    })
}

/// The compiler's refusal of a name nothing admits.
fn refusal(name: &str) -> String {
    idiom(name).unwrap_or_else(|| refused(name))
}

/// One name as JSON, and whether it was known.
fn one_json(name: &str) -> (Value, bool) {
    let Some(Entry {
        tag,
        attr,
        value_of,
        open,
        command,
        function,
        part,
        concept,
        recipe,
        pattern,
    }) = entry(name)
    else {
        return (json!({ "name": name, "refused": refusal(name) }), false);
    };
    let mut doc = json!({ "name": name });
    if let Some(tag) = tag {
        doc["tag"] = tag;
    }
    if name == "symbol" {
        doc["roles"] = exact_kernel::generated::SYMBOL_ROLES.to_vec().into();
    }
    if let Some(attr) = attr {
        doc["attribute"] = attr;
    }
    if !value_of.is_empty() {
        doc["valueOf"] = value_of.into();
    }
    if let Some(open) = open {
        doc["note"] = open.into();
    }
    if let Some(command) = command {
        doc["command"] = command.into();
    }
    if let Some(f) = &function {
        doc["function"] = function_json(f);
    }
    if let Some(p) = part {
        doc["part"] =
            json!({ "module": p.module, "props": p.props, "about": p.about, "call": p.call });
    }
    if let Some(c) = concept {
        // A name the web has and Contract refuses (`every`) says so beside it.
        if function.is_none() {
            if let Some(note) = idiom(name) {
                doc["refusal"] = note.into();
            }
        }
        doc["concept"] = concept_json(&c);
    }
    if let Some((recipe, text)) = recipe {
        doc["recipe"] =
            json!({ "name": recipe, "path": format!("docs/recipes/{recipe}.md"), "text": text });
    }
    if let Some((pattern, about)) = pattern {
        doc["pattern"] = json!({ "name": pattern, "about": about, "write": format!("bun exact.mjs pattern {pattern}") });
    }
    (doc, true)
}

/// One name as text, printed (a refusal on stderr), and whether it was
/// known; a concept or recipe already in `shown` is named, not printed again.
fn one_text(name: &str, shown: &mut Shown) -> bool {
    // LLP 1116.017 D1: the parser reads `h1`-`h6` as headings, so no table has them.
    if let Some(level) = name
        .strip_prefix('h')
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|n| (1..=6).contains(n))
    {
        println!("{name}: a heading of level {level}: `{name} \"Title\"` is `text \"Title\" role=\"heading\" aria-level={level}`; a level the app computes is written that way");
        return true;
    }
    let Some(Entry {
        tag,
        attr,
        value_of,
        open,
        command,
        function,
        part,
        concept,
        recipe,
        pattern,
    }) = entry(name)
    else {
        eprintln!("{}", refusal(name));
        return false;
    };
    // Nothing but the concept: a name the web has and Contract refuses
    // (`every`) says so first; anything else is printed above the concept.
    let only = tag.is_none()
        && attr.is_none()
        && value_of.is_empty()
        && open.is_none()
        && command.is_none()
        && function.is_none()
        && part.is_none();
    let above = !only || concept.is_some();
    if let Some(p) = part {
        println!(
            "{name}: a part of `{}` (`use {name} from \"{}\"`)",
            p.module, p.module
        );
        println!("  props {}", p.props.join(", "));
        println!("  {}", p.about);
        if let Some(call) = &p.call {
            println!("  call {call}");
        }
    }
    if let Some(t) = tags::tag(name) {
        println!("{name}: tag, {}", tag_text(&t));
        if let Some(note) = vocab::tag_note(name) {
            println!("  {note}");
        }
    }
    // A symbol refused at compile time points here for the roles (LLP 1116.021 D3).
    if name == "symbol" {
        println!("{}", symbol_roles());
    }
    if let Some(a) = tags::attr(name) {
        println!("{name}: {} attribute", kind(&a));
        for line in attr_detail(name, &a) {
            println!("  {line}");
        }
        if let Some(only) = only_on(name) {
            println!("  only on {only}");
        }
        if name == "src" {
            println!("  an `image`'s source is its argument: `image url` (HTML's `img src`)");
        }
        if matches!(name, "min" | "max" | "step") {
            println!("  on an `input` a number of either sign is written as its text: `min=-40`, `step=0.5`, `max=1e6` (LLP 1116.010 D3); a decimal pad (`inputmode=\"decimal\"`) with a negative `min` has the ± key");
        }
        if name == "markup" {
            println!("  \"markdown\" or \"none\" (the default): a `text` reads its string as Markdown, a `textarea` edits it (LLP 1045)");
        }
        if matches!(name, "intersect" | "intersect-margin") {
            println!("  IntersectionObserver: nearest scroller or viewport, threshold 0; mount and enter/leave. Optional final IntersectionObserverEntry: isIntersecting (bool), intersectionRatio (number). intersect-margin is a literal 1–4 px/% rootMargin, default 0px. Browser executors and shared AppKit/UIKit/Linux layout/scroll delivery; native geometry qualification remains open, see LLP 1001 for geometry deviations (LLP 1005).");
        }
        if matches!(name, "reachstart" | "reachend") {
            println!("  {}", vocab::collection_edge_refusal(name));
        }
        if name == "resize" {
            println!("  given an action, the element resize event: ResizeObserver's, after layout, with the content box's width and height and its `DOMRectReadOnly`");
        }
        if matches!(name, "focusin" | "focusout") {
            println!("  bubbles from the focused child without making this node a Tab stop; an action may take `FocusEvent(target: string, relatedTarget: option<string>)` on every host (`focusout` also fires on a move between descendants and on the focused node's removal)");
        }
        if name == "type" {
            println!("  {SEARCH_TYPE}");
        }
        if name == "title" {
            println!("  on `head`, the document's title; elsewhere HTML's advisory text, the platform's tooltip (prop title, str)");
        }
        if name == "navigationBar" {
            println!("  on a route (LLP 1115.000 D5.2). \"none\": the screen has no navigation bar on iOS, a full-bleed screen that draws its own chrome and way back (a photo viewer, a header laid out by hand); its `header` paints as written and the edge swipe still goes back. \"auto\", the default, is the platform's bar: the route's `header` when it is one (`contract vocab header`), else the system Back button alone on a pushed route, and no bar on a stack's first route. A `navigationBack` control drawn in a pushed route's page without it is a second Back (warning `lower-back-twice`). On the navigation root or off a route it is refused. docs/recipes/navigation-stack.md");
        }
        if name == "navigationDetent" {
            println!("  {DETENT}");
        }
        if name == "navigationGrabber" {
            println!("  on a sheet route (`navigationPresentation=\"modal\"`). \"visible\" shows the sheet's grabber, the short bar at its top edge, \"hidden\" hides it; \"auto\", the default, is the platform's: iOS shows one on a sheet with several detents (`navigationDetent=\"medium large\"`), which the grabber resizes, and none on a sheet of one height. UIKit's `prefersGrabberVisible`. Draw no grabber of your own: say this. Hosts whose sheets have none ignore it");
        }
        if name == "wakelock" {
            println!("  on any element (LLP 1125 D11). \"screen\" keeps the display from dimming or locking for idleness while the element is mounted and displayed (no ancestor `display: none`, under no unselected tab and no screen a stack keeps under its top; the one a sheet covers is shown), so a screen kept in a hidden tab panel holds nothing; \"none\", the default, holds nothing. Bind it to state: `wakelock=(playing ? \"screen\" : \"none\")`. A literal other than the two is refused; a computed one is journaled and holds nothing. iOS's idle timer, a macOS display-sleep assertion, the web's Screen Wake Lock (asked again when the page is visible again), Android's `keepScreenOn`; Linux and Windows journal it as unavailable. The agent's `state.wakelock` is `{{held, by}}`");
        }
        if name == "navigationSplit" {
            println!("  on a stack's root route (LLP 1116.004 D1.2). A wide window (900 px and up on the web) shows a stack whose root route is a list whose rows push beside its top, as a split view does, with nothing to write: a row pushes when it draws the disclosure, an `image \"symbol:forward-chevron\"` (the chevron an iOS list draws) in a `link`, or in a `button` or pressable whose action writes the router (`push`, `open`, `go`); a chevron on a row that only selects is decoration, and its route is left unsplit, `navigationSplit=\"none\"` said for you. The list is a 320-wide column, its chevrons hidden and the row whose press pushed the detail marked; the top route is the detail beside it (a detail a row pushed has no Back), a `navigationPresentation=\"modal\"` route stays a sheet, and a phone pushes. The router is unchanged: a push from a row while a detail shows stacks it, so `replace` there to swap the detail. \"none\" keeps the stack pushed; a number is the column's width and splits a list the rule does not find. While nothing is selected the detail pane shows the platform's placeholder (\"No Selection\", centred, in the secondary label's colour); \"placeholder\" on a box in the root route (an `EmptyState`) is your own instead, shown only there and then and laid out nowhere else. A root `tablist` left as the tabs recipe places it is a sidebar on the same window. docs/recipes/navigation-stack.md");
        }
        if name == "-exact-fit" {
            println!("  on a `text`: \"shrink\" draws a line too wide for its box smaller until it fits, down to half its size, as UIKit's adjustsFontSizeToFitWidth (a timer's readout at the largest text size); with `white-space=\"nowrap\"` the whole line fits, else its longest word (LLP 1116.001 D1.4)");
        }
    }
    if !value_of.is_empty() {
        let of: Vec<String> = value_of.iter().map(|n| format!("`{n}`")).collect();
        println!(
            "{name}: a value of {} (`contract vocab {}` lists the others)",
            of.join(", "),
            value_of[0]
        );
    }
    if let Some(open) = open {
        // A hyphenated name is usually a CSS property Contract lacks (r26 t1 read
        // `outline-width` as a module): say that first.
        if open == vocab::MODULE_NOTE {
            println!("{name}: not a built-in tag or attribute, so a built-in tag refuses an attribute of that name; {open}");
        } else {
            println!("{name}: {open}");
        }
        if let Some((near, _)) = recipes::RECIPES.iter().find(|(r, _)| one_edit(name, r)) {
            println!("  did you mean the recipe `{near}`?");
        }
    }
    if let Some(command) = command {
        println!("{name}: {command}");
    }
    if let Some(f) = &function {
        print!("{}", function_text(f));
    }
    if let Some(c) = concept {
        if only {
            if let Some(note) = idiom(name) {
                println!("{note}");
            }
        }
        if shown.concepts.contains(&c.name) {
            println!("{name}: the concept `{}`, above", c.name);
        } else {
            if !only {
                println!();
            }
            print!("{}", c.text());
            shown.concepts.push(c.name);
        }
    }
    if let Some((recipe, text)) = recipe {
        if shown.recipes.contains(&recipe) {
            println!("{name}: the recipe `{recipe}`, above");
        } else {
            if above {
                println!();
            }
            println!("{name}: the recipe docs/recipes/{recipe}.md");
            print!("{text}");
            shown.recipes.push(recipe);
        }
    }
    if let Some((pattern, about)) = pattern {
        println!("{name}: a pattern, a tested app (`bun exact.mjs pattern {pattern}` writes it here): {about}");
    }
    true
}

/// What `image "symbol:…"` takes: the roles each host draws from its own set.
fn symbol_roles() -> String {
    format!(
        "symbol: an image's source `symbol:<role>` is an icon each host draws from its own set; the roles: {}; `symbol:sf/<name>` is any SF Symbol, drawn on Apple and blank elsewhere (docs/recipes/symbols.md)",
        exact_kernel::generated::SYMBOL_ROLES.join(", ")
    )
}

fn concept_json(c: &Concept) -> Value {
    json!({
        "name": c.name,
        "title": c.title,
        "names": c.names,
        "answer": c.answer,
        "contract": c.contract(),
    })
}

/// Why the compiler refuses a name the web has (`sort`, `now`, `padStart`),
/// with what to write instead; `None` for any other.
fn idiom(name: &str) -> Option<String> {
    if name == "now" {
        return Some(contract_syntax::idioms::NOW_REFUSED.into());
    }
    contract_syntax::idioms::refusal(name)
}

/// A name nothing admits: the lookups' hints, and a function one edit away.
fn refused(name: &str) -> String {
    let refusal = vocab::refusal(name);
    let head = format!("`{name}` is not a tag or an attribute");
    let hints = refusal.strip_prefix(&head).unwrap_or(&refusal);
    let mut out = format!(
        "`{name}` is not a tag, an attribute, a standard function, a recipe or a pattern{hints}"
    );
    if let Some(near) = functions::functions()
        .into_iter()
        .find(|f| one_edit(name, &f.name))
    {
        out += &format!("; did you mean the function `{}`?", near.name);
    } else if let Some((near, _)) = recipes::RECIPES.iter().find(|(r, _)| one_edit(name, r)) {
        out += &format!("; did you mean the recipe `{near}`?");
    }
    out
}

/// Whether two names differ by one insertion, deletion, substitution or
/// swap of neighbours, or by case alone.
fn one_edit(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a == b {
        return false;
    }
    if a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.eq_ignore_ascii_case(y)) {
        return true;
    }
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let (ra, rb) = (&a[prefix..], &b[prefix..]);
    match ra.len() as isize - rb.len() as isize {
        0 => {
            ra[1..] == rb[1..]
                || (ra.len() >= 2 && ra[0] == rb[1] && ra[1] == rb[0] && ra[2..] == rb[2..])
        }
        1 => ra[1..] == *rb,
        -1 => *ra == rb[1..],
        _ => false,
    }
}

fn function_json(f: &Function) -> Value {
    let mut doc = json!({
        "name": f.name,
        "kind": if f.signature.is_some() { "standard function" } else { "intrinsic" },
        "signature": f.signature,
        "rules": f.rows.iter().map(|r| json!({"call": r.call, "rule": r.rule, "example": r.example})).collect::<Vec<_>>(),
    });
    if let Some(note) = idiom(&f.name) {
        doc["note"] = note.into();
    }
    if routed(f) {
        doc["routes"] = true.into();
    }
    doc
}

/// Whether the function exists only in an app with a `routes` declaration.
fn routed(f: &Function) -> bool {
    exact_plan::Stdlib::from_name(&f.name).is_some_and(contract_types::routes::needs_routes)
}

/// A function's entry: its signature, each row's call and rule, and its
/// example, then what the web's function of that name becomes.
fn function_text(f: &Function) -> String {
    let mut out = match &f.signature {
        Some(signature) => format!("{}: standard function\n  signature: {signature}\n", f.name),
        None => format!("{}: compiler intrinsic\n", f.name),
    };
    for row in &f.rows {
        out += &format!("  {}: {}\n", row.call, row.rule);
        if !row.example.is_empty() {
            out += &format!("    example: {}\n", row.example);
        }
    }
    if routed(f) {
        out += "  only in an app with a `routes` declaration\n";
    }
    if let Some(note) = idiom(&f.name) {
        out += &format!("  the web's `{}`: {note}\n", f.name);
    }
    out += "  docs/contract-grammar.md#standard-functions-and-intrinsics\n";
    out
}

/// A host command: an action statement, never a value.
fn command_text(name: &str) -> String {
    let what = match name {
        "showNotification" => "; `showNotification(title=…, body=…, tag=…, showTrigger=…, navigate=…)` posts a local notification; its plain click opens `navigate=` through the navigation root (warm event or cold launch location). Needs `device.notifications` (docs/reference.md#notifications)",
        "setRangeText" => "; `setRangeText(id, text[, start, end[, selectionMode]])`: web/macOS/iOS/Linux plain text field replacement; UTF-16 offsets, selectionMode preserve (default), select, start or end; one input (InputEvent.inputType insertReplacementText), no undo step. Markdown, Windows, Android and terminal are not supported yet (LLP 1069.001)",
        "setMode" => "; `setMode(\"dark-style\", \"dim\")` sets one of the window's app modes, an axis of the app's design (`app.tamber`) and one of its values; a subtree pins its own with `-exact-mode` (LLP 1121 D5)",
        "setRootFontSize" => "; `setRootFontSize(px)` sets CSS's `:root { font-size }`, the size every `rem` follows (px above 0), over the host's own; `setRootFontSize(\"125%\")` sets a percentage of the host's size that keeps following it (Dynamic Type, the browser's setting); `setRootFontSize(\"medium\")` hands it back (LLP 1069.000 D3, LLP 1121 D6)",
        _ => "",
    };
    format!("host command, an action statement (docs/contract-grammar.md#host-commands){what}")
}

fn family(t: &Tag) -> &'static str {
    if t.node_type.name().starts_with("Svg") {
        "svg"
    } else {
        "html"
    }
}

fn kind(a: &AttrTarget) -> &'static str {
    match a {
        AttrTarget::Styles(_) => "style",
        AttrTarget::Prop(_) => "prop",
        AttrTarget::InvertedBoolProp(_) => "inverted-prop",
        AttrTarget::Handler(_) => "handler",
        AttrTarget::Flex => "flex",
        AttrTarget::Shorthand => "shorthand",
        AttrTarget::Surface => "surface",
        AttrTarget::MediaMetadata => "record",
    }
}

fn prop_type(p: PropId) -> String {
    format!("{:?}", tags::prop_ty(p)).to_lowercase()
}

/// What a long press (a right click) opens: the menu `contextPopover` names.
const CONTEXTMENU: &str = "a long press on iOS, a right click elsewhere: `contextmenu=aim(id)` notes which item it was and `contextPopover=\"<menu id>\"` names the `popover=\"auto\" role=\"menu\"` column it opens (iOS: a UIMenu over the item). See `contract vocab menu`; a test opens it with `tap \"<testId>\" contextmenu`";

/// HTML's capitalization hint, said for the fields that want other than sentences.
const AUTOCAPITALIZE: &str = "`words` for a person's name, `none` for a username or a code; unsaid is sentences (an email, URL or password field is never capitalized)";

/// What `metadata=` takes (LLP 1098 D1): the Media Session's record.
const METADATA: &str = "MediaMetadata(title=, artist=, album=, artwork=), each a string";

/// `border-radius` on a sheet route (ModalIOS.swift `cornerRadius(of:)`).
const SHEET_RADIUS: &str = "on iOS a sheet route's (`navigationPresentation=\"modal\"`) top-left radius is also its sheet's corner radius (`preferredCornerRadius`), which seats a large sheet flat on the bottom edge. Unsaid, the sheet is UIKit's: on iOS 26 and later rounded like the display and inset from the bottom edge, as Apple's own sheets are";
const DETENT: &str = "on a sheet route (`navigationPresentation=\"modal\"`), its resting heights, the first to start: `\"45%\"` is that share of the sheet's maximum (UIKit's maximumDetentValue; React Native's `sheetAllowedDetents: [0.45]`), so a share of the screen needs no measure. A point height and fit-content stop at that maximum. UIKit takes detents shortest to tallest, and a point height and a share cross on some screen, so the words before large are one kind (points, or shares with medium as half), each taller than the one before. A bound value iOS cannot use is journaled (`logs`) with the route, and the sheet keeps its other detents or opens large. iOS alone sizes a sheet; other hosts ignore it (LLP 1075.003 §9.11, §9.13)";
/// LLP 1116.020 D3: round 18's builders drew search as a field and a Search
/// button in the content; the bar's search field is one attribute away.
const SEARCH_TYPE: &str = "on an `input`, HTML's (`number`, `date`, `checkbox`, …); `type=\"search\"` in a route's `header` is the bar's search field, filtering as you type (iOS: UIKit's search controller; `contract vocab search`)";
const APPEARANCE: &str = "Explicit none on a header or root tablist keeps the authored bar; on a header button it keeps only that button authored in place while the rest of the header projects. A title button keeps its contents; an authored Back replaces UIKit Back. Search or a header tablist can opt out on that element; a tab button alone keeps its parent segmented. Tabs retain their panels and navigation stacks. Auto restores platform projection.";

/// Where an attribute is admitted, if not on every tag.
fn only_on(name: &str) -> Option<&'static str> {
    // `title` is every element's, HTML's tooltip; `head`'s is the document's.
    if tags::HEAD_FIELDS.contains(&name) && name != "title" {
        return Some("`head`");
    }
    vocab::CONTEXTUAL
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, on)| *on)
}

/// The rows `flex: <n>` sets.
const FLEX_ROWS: [StyleId; 3] = [StyleId::FlexGrow, StyleId::FlexShrink, StyleId::FlexBasis];

fn rows_of(name: &str, a: &AttrTarget) -> &'static [StyleId] {
    match a {
        AttrTarget::Styles(rows) => rows,
        AttrTarget::Flex => &FLEX_ROWS,
        AttrTarget::Shorthand => vocab::shorthand_rows(name),
        _ => &[],
    }
}

fn row_json(row: StyleId) -> Value {
    json!({
        "row": row.name(),
        "codec": vocab::codec(row),
        "values": row.enum_names(),
        "default": vocab::default(row),
    })
}

fn tag_json(name: &str, t: &Tag) -> Value {
    let mut doc = json!({
        "name": name,
        "node": t.node_type.name(),
        "family": family(t),
        "fixedStyles": t.fixed_styles.iter().map(|(row, v)| json!({"row": row.name(), "value": v})).collect::<Vec<_>>(),
        "fixedProps": t.fixed_props.iter().map(|(p, v)| json!({"prop": p.name(), "value": v})).collect::<Vec<_>>(),
        "positional": t.positional.map(PropId::name),
    });
    if let Some(note) = vocab::tag_note(name) {
        doc["note"] = note.into();
    }
    doc
}

const TEXT_PX_WIDTH: &str = "An absolute width (a number, px, pt or absolute calc()) containing scalable text stays fixed at accessibility sizes (lower-text-px-width). Use rem for the width (width=\"3rem\"), or let content determine it. The lint is deliberately conservative: it stays silent for unresolved classes, uncertain visibility or sizing, flex sizing that can supersede width, and min/max-width bounds that may change it. Text in absolute or fixed positioning, popovers and dialogs is checked inside that subtree, without counting against enclosing boxes. Text inherits the platform root font. Native Apple controls use the Apple control font, which can scale with Dynamic Type; web controls inherit, and Linux control defaults are fixed. An absolute font size stops scaling within that boundary; inherit and unset use the parent font even on controls. The warning checks layout boxes containing text runs or text fields, including HTML inside SVG foreignObject. It reads lowering’s grouped-list typography and known styles; computed widths and fonts remain unknown.";

fn attr_json(name: &str, a: &AttrTarget) -> Value {
    let mut doc = json!({ "name": name, "kind": kind(a) });
    match a {
        AttrTarget::Styles(_) | AttrTarget::Flex | AttrTarget::Shorthand => {
            doc["rows"] = rows_of(name, a).iter().map(|r| row_json(*r)).collect();
            if rows_of(name, a).iter().any(|r| vocab::takes_roles(*r)) {
                let (exact, css) = vocab::color_roles();
                doc["roles"] = json!({ "exact": exact, "css": css });
            }
        }
        AttrTarget::Prop(p) | AttrTarget::InvertedBoolProp(p) => {
            doc["prop"] = p.name().into();
            doc["type"] = prop_type(*p).into();
            if let Some(accepts) = vocab::accepts(*p) {
                doc["accepts"] = accepts.into();
            }
        }
        AttrTarget::Handler(event) => {
            doc["event"] = (*event).into();
            if name == "contextmenu" {
                doc["note"] = CONTEXTMENU.into();
            }
        }
        AttrTarget::Surface => {}
        AttrTarget::MediaMetadata => {
            doc["type"] = METADATA.into();
            doc["props"] = json!(["mediaTitle", "mediaArtist", "mediaAlbum", "mediaArtwork"]);
        }
    }
    if let Some(only) = only_on(name) {
        doc["only"] = only.into();
    }
    if name == "type" {
        doc["note"] = SEARCH_TYPE.into();
    }
    if name == "autocapitalize" {
        doc["note"] = AUTOCAPITALIZE.into();
    }
    if name == "accept" {
        doc["note"] = "On input type=file, omit, leave empty, or use */* for any file; specific types are media or app file_handlers types".into();
    }
    if matches!(name, "width" | "font-size") {
        doc["note"] = TEXT_PX_WIDTH.into();
    }
    if name == "buttonStyle" {
        doc["note"] = vocab::BUTTON_DEFAULT_NOTE.into();
    }
    if name == "appearance" {
        doc["note"] = APPEARANCE.into();
    }
    if name == "navigationDetent" {
        doc["note"] = DETENT.into();
    }
    if name == "border-radius" {
        doc["note"] = SHEET_RADIUS.into();
    }
    if name == "transform" {
        doc["note"] = vocab::BOX_TRANSFORM_REFUSAL.into();
    }
    if matches!(name, "reachstart" | "reachend") {
        doc["note"] = vocab::collection_edge_refusal(name).into();
    }
    doc
}

fn everything() -> Value {
    json!({
        "tags": vocab::tags().iter().map(|(n, t)| tag_json(n, t)).collect::<Vec<_>>(),
        "attributes": vocab::attrs().iter().map(|(n, a)| attr_json(n, a)).collect::<Vec<_>>(),
        "renamed": vocab::renamed().iter().map(|(n, r)| json!({"name": n, "use": r})).collect::<Vec<_>>(),
        "html": vocab::html_tags().iter().map(|(n, h)| json!({"name": n, "hint": h})).collect::<Vec<_>>(),
        "head": tags::HEAD_FIELDS,
        "contextual": vocab::CONTEXTUAL.iter().map(|(n, on)| json!({"name": n, "only": on})).collect::<Vec<_>>(),
        "commands": HOST_COMMANDS,
        "functions": functions::functions().iter().map(function_json).collect::<Vec<_>>(),
        "concepts": concepts::concepts().iter().map(concept_json).collect::<Vec<_>>(),
        "notes": { "modules": vocab::MODULE_NOTE, "data": vocab::DATA_NOTE },
    })
}

fn tag_text(t: &Tag) -> String {
    let mut text = format!("{} ({})", t.node_type.name(), family(t));
    let fixed: Vec<String> = t
        .fixed_styles
        .iter()
        .map(|(row, v)| format!("{}: {v}", row.name()))
        .chain(
            t.fixed_props
                .iter()
                .map(|(p, v)| format!("{}={v}", p.name())),
        )
        .collect();
    if !fixed.is_empty() {
        text += &format!("; sets {}", fixed.join(", "));
    }
    if let Some(p) = t.positional {
        text += &format!("; positional {}", p.name());
    }
    text
}

/// One row's codec, values and default, as one phrase.
fn row_text(row: StyleId) -> String {
    let mut text = vocab::codec(row);
    let values = row.enum_names();
    if !values.is_empty() {
        text += &format!(" {}", values.join("|"));
    }
    if let Some(default) = vocab::default(row) {
        text += &format!(", default {default}");
    }
    text
}

/// An attribute's detail lines: a style's rows (shared description once),
/// a prop's type, a handler's event.
fn attr_detail(name: &str, a: &AttrTarget) -> Vec<String> {
    match a {
        AttrTarget::Styles(_) | AttrTarget::Flex | AttrTarget::Shorthand => {
            let rows = rows_of(name, a);
            let names: Vec<&str> = rows.iter().map(|r| r.name()).collect();
            let texts: Vec<String> = rows.iter().map(|r| row_text(*r)).collect();
            let mut lines = vec![format!("rows {}", names.join(", "))];
            if texts.iter().all(|t| *t == texts[0]) {
                lines.push(texts[0].clone());
            } else {
                lines.extend(names.iter().zip(&texts).map(|(n, t)| format!("{n}: {t}")));
            }
            if matches!(a, AttrTarget::Flex) {
                lines.push("`flex: <n>` is CSS's `<n> 1 0%`".into());
            }
            // The roles a colour takes, by name: a builder guessed one (`contract vocab background-color`, tip-splitter).
            if rows.iter().any(|r| vocab::takes_roles(*r)) {
                let (exact, css) = vocab::color_roles();
                lines.push(format!(
                    "roles `-exact-<role>`, a role that follows the platform and the scheme: {}",
                    exact.join(", ")
                ));
                lines.push(format!("CSS system colours, by name: {}", css.join(", ")));
            }
            if rows == [StyleId::Transition] {
                lines.push(format!(
                    "properties {}",
                    vocab::transition_properties().join(", ")
                ));
            }
            if name == "backdrop-filter" {
                lines.push("macOS samples only the parent's subtree; make the glass a sibling of what it covers (LLP 1053.000 §3)".into());
            }
            if matches!(name, "width" | "font-size") {
                lines.push(TEXT_PX_WIDTH.into());
            }
            if name == "font-family" {
                lines.push("CSS family list: literal lists and literal choices work on every host; a string from state or data names installed or generic families on web, macOS, iOS and Linux. Invalid lists leave the row unset. Bundled faces use a `font` declaration".into());
            }
            if name == "appearance" {
                lines.push(APPEARANCE.into());
            }
            if name == "border-radius" {
                lines.push(SHEET_RADIUS.into());
            }
            if name == "transform" {
                lines.push(vocab::BOX_TRANSFORM_REFUSAL.into());
            }
            lines
        }
        AttrTarget::Prop(p) => {
            let mut line = format!("prop {}, {}", p.name(), prop_type(*p));
            if let Some(accepts) = vocab::accepts(*p) {
                line += &format!(", accepts {accepts}");
            }
            // A prop with a fixed set of names lists them (authoring bench: `vocab buttonStyle`).
            if p.name() == "buttonStyle" {
                line += &format!(
                    ", one of {}",
                    exact_kernel::generated::BUTTON_STYLES.join("|")
                );
                // LLP 1115.000 D14.5: a presentation default is the platform's, said per host.
                return vec![line, vocab::BUTTON_DEFAULT_NOTE.into()];
            }
            if name == "autocapitalize" {
                return vec![line, AUTOCAPITALIZE.into()];
            }
            if name == "accept" {
                return vec![line, "on input type=file: omit, leave empty, or use */* for any file; specific types are media or app file_handlers types".into()];
            }
            vec![line]
        }
        AttrTarget::InvertedBoolProp(p) => {
            vec![format!("prop {}, bool, set to the inverse", p.name())]
        }
        AttrTarget::Handler(event) if name == "contextmenu" => {
            vec![format!("event {event}"), CONTEXTMENU.into()]
        }
        AttrTarget::Handler(event) => vec![format!("event {event}")],
        AttrTarget::Surface => vec!["a canvas's surface binding: surface=name(args)".into()],
        AttrTarget::MediaMetadata => vec![
            format!("record {METADATA}"),
            "props mediaTitle, mediaArtist, mediaAlbum, mediaArtwork, its fields".into(),
            "claims the media session (LLP 1098 D1)".into(),
        ],
    }
}

/// Names wrapped into indented lines.
fn wrapped(out: &mut String, names: impl IntoIterator<Item = String>) {
    let mut line = String::from(" ");
    for name in names {
        if line.len() + name.len() > 92 {
            out.push_str(line.trim_end_matches(','));
            out.push('\n');
            line = String::from(" ");
        }
        line += &format!(" {name},");
    }
    out.push_str(line.trim_end_matches(','));
    out.push('\n');
}

fn listing() -> String {
    let mut out = String::new();
    let tags = vocab::tags();
    out += &format!(
        "tags ({}): name  node (family); fixed rows and props\n",
        tags.len()
    );
    for (name, t) in &tags {
        out += &format!("  {name:<20} {}\n", tag_text(t));
    }
    let attrs = vocab::attrs();
    let of = |k: &'static str| attrs.iter().filter(move |(_, a)| kind(a) == k);
    out += &format!(
        "\nstyle attributes ({}): name  codec [values], default (rows, when not one of the same name)\n",
        of("style").count() + of("flex").count() + of("shorthand").count()
    );
    for (name, a) in of("style").chain(of("flex")).chain(of("shorthand")) {
        let rows = rows_of(name, a);
        let mut text = if rows.iter().all(|r| row_text(*r) == row_text(rows[0])) {
            row_text(rows[0])
        } else {
            rows.iter()
                .map(|r| format!("{}: {}", r.name(), row_text(*r)))
                .collect::<Vec<_>>()
                .join("; ")
        };
        if rows.len() > 1 || rows[0].name() != name.replace('-', "_") {
            text += &format!(
                " ({})",
                rows.iter().map(|r| r.name()).collect::<Vec<_>>().join(" ")
            );
        }
        if let Some(only) = only_on(name) {
            text += &format!("; only on {only}");
        }
        out += &format!("  {name:<28} {text}\n");
    }
    let props: Vec<_> = of("prop")
        .chain(of("inverted-prop"))
        .chain(of("surface"))
        .chain(of("record"))
        .collect();
    out += &format!("\nprop attributes ({}): name  type\n", props.len());
    for (name, a) in props {
        let mut text = match a {
            AttrTarget::Prop(p) => prop_type(*p),
            AttrTarget::InvertedBoolProp(p) => format!("bool (inverse of {})", p.name()),
            AttrTarget::MediaMetadata => METADATA.into(),
            _ => "surface=name(args), on `canvas`".into(),
        };
        if let Some(only) = only_on(name) {
            text += &format!("; only on {only}");
        }
        out += &format!("  {name:<28} {text}\n");
    }
    out += &format!("\nhandlers ({}):\n", of("handler").count());
    wrapped(&mut out, of("handler").map(|(n, _)| n.to_string()));
    out += "\nrenamed (refused, with the name to write):\n";
    wrapped(
        &mut out,
        vocab::renamed().iter().map(|(n, r)| format!("{n} -> {r}")),
    );
    out += "\nHTML elements Contract spells differently:\n";
    for (name, hint) in vocab::html_tags() {
        out += &format!("  {name:<8} {hint}\n");
    }
    out += &format!(
        "\nonly on `head`: {}\nonly on some tags:\n",
        tags::HEAD_FIELDS.join(", ")
    );
    for (name, on) in vocab::CONTEXTUAL {
        out += &format!("  {name:<16} {on}\n");
    }
    out += &format!(
        "\nhost commands ({}), action statements:\n",
        HOST_COMMANDS.len()
    );
    wrapped(&mut out, HOST_COMMANDS.iter().map(|c| c.to_string()));
    let (mut standard, intrinsic): (Vec<Function>, Vec<Function>) = functions::functions()
        .into_iter()
        .partition(|f| f.signature.is_some());
    standard.sort_by_key(|f| f.name.to_lowercase());
    out += &format!(
        "\nstandard functions ({}): the roster's signature; `contract vocab <name>` gives its rules and an example\n",
        standard.len()
    );
    for f in &standard {
        out += &format!("  {}\n", f.signature.as_deref().unwrap_or_default());
    }
    out += "\ncompiler intrinsics:\n";
    wrapped(&mut out, intrinsic.into_iter().map(|f| f.name));
    out += &format!(
        "\nopen sets:\n  {}\n  {}\n",
        vocab::MODULE_NOTE,
        vocab::DATA_NOTE
    );
    let concepts = concepts::concepts();
    out += &format!(
        "\nconcepts ({}): `contract vocab <name>` answers with the whole idiom, its code and its recipe\n",
        concepts.len()
    );
    for c in &concepts {
        out += &format!("  {:<10} {}\n", c.name, c.title);
    }
    out
}
