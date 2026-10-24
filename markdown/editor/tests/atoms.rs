//! LLP 1045.000 D5: editing at an inline atom, driven as a browser drives
//! the editor (`beforeinput`, platform selections) and as an app does
//! (`setSelectionRange`, `setRangeText`, faces arriving).

use exact_markdown::Atoms;
use exact_markdown_editor::{view::HIDDEN, Editor, Input, Outcome, Selected, SelectionMode};

const KINDS: &str =
    r#"[{"name":"citation","link":"t3-citation:"},{"name":"mention","prefix":"@"}]"#;
const CITE: &str = "[Quote](t3-citation:1)";

fn u(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn n(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

struct Page {
    e: Editor,
    now: f64,
}

impl Page {
    fn open(text: &str, faces: &str) -> Self {
        let mut e = Editor::new();
        e.load(&u(text));
        e.set_atoms(Atoms::parse(KINDS, faces));
        Self { e, now: 0.0 }
    }

    fn source(&self) -> String {
        String::from_utf16(self.e.source()).unwrap()
    }

    fn now(&mut self) -> f64 {
        self.now += 40.0;
        self.now
    }

    fn caret(&mut self, p: u32) {
        if self.e.select(p, p, None) == Selected::Place {
            let (a, b) = self.e.selection();
            assert_eq!(self.e.select(a, b, Some((a, b))), Selected::Ignore);
        }
    }

    fn input(&mut self, input: Input, data: &str) -> Outcome {
        let now = self.now();
        self.e.before_input(input, &u(data), None, now)
    }

    fn handled(&mut self, input: Input, data: &str) {
        assert!(
            matches!(self.input(input, data), Outcome::Handled(_)),
            "{input:?} {data:?} at {:?} in {:?}",
            self.e.selection(),
            self.source()
        );
    }

    fn undo(&mut self) {
        let now = self.now();
        self.e.command("undo", "", now);
    }
}

#[test]
fn a_caret_inside_an_atom_goes_to_its_end_and_a_range_takes_it_whole() {
    let text = format!("a {CITE} b");
    let mut p = Page::open(&text, "[]");
    let (start, end) = (2, 2 + n(CITE));
    p.caret(start + 3);
    assert_eq!(p.e.selection(), (end, end));
    p.caret(start);
    assert_eq!(p.e.selection(), (start, start));
    // setSelectionRange: a caret by the caret rule, a range's ends outward.
    p.e.set_selection(start + 1, start + 1);
    assert_eq!(p.e.selection(), (end, end));
    p.e.set_selection(1, start + 4);
    assert_eq!(p.e.selection(), (1, end));
    p.e.set_selection(start + 4, end + 1);
    assert_eq!(p.e.selection(), (start, end + 1));
    // The platform's range: the same.
    assert_eq!(p.e.select(0, start + 2, None), Selected::Place);
    assert_eq!(p.e.selection(), (0, end));
}

#[test]
fn backspace_at_an_atoms_end_and_delete_at_its_start_remove_it_whole_as_one_undo_step() {
    let text = format!("a {CITE} b");
    let (start, end) = (2, 2 + n(CITE));
    let mut p = Page::open(&text, "[]");
    p.caret(end);
    p.handled(Input::Backward, "");
    assert_eq!(p.source(), "a  b");
    assert_eq!(p.e.selection(), (start, start));
    p.undo();
    assert_eq!(p.source(), text);
    assert_eq!(p.e.atoms().len(), 1);
    p.caret(start);
    p.handled(Input::Forward, "");
    assert_eq!(p.source(), "a  b");
}

#[test]
fn option_backspace_takes_the_atom_and_the_spaces_to_the_caret_but_not_a_line_break() {
    let mut p = Page::open(&format!("a {CITE}   b"), "[]");
    let after = 2 + n(CITE) + 3;
    p.caret(after);
    p.handled(Input::WordBackward, "");
    assert_eq!(p.source(), "a b");
    let mut p = Page::open(&format!("{CITE}\nb"), "[]");
    p.caret(n(CITE) + 1);
    // No atom across the line break: the platform's own word deletion.
    let now = p.now();
    let out =
        p.e.before_input(Input::WordBackward, &[], Some((n(CITE), n(CITE) + 1)), now);
    assert!(matches!(out, Outcome::Handled(_)));
    assert_eq!(p.source(), format!("{CITE}b"));
    let mut p = Page::open(&format!("x  {CITE} y"), "[]");
    p.caret(1);
    p.handled(Input::WordForward, "");
    assert_eq!(p.source(), "x y");
}

#[test]
fn typing_beside_an_atom_is_the_editors_and_lands_outside_it() {
    let text = format!("a {CITE}");
    let (start, end) = (2, 2 + n(CITE));
    let mut p = Page::open(&text, "[]");
    p.caret(end);
    p.handled(Input::Text, "!");
    assert_eq!(p.source(), format!("a {CITE}!"));
    assert_eq!(p.e.atoms().len(), 1);
    p.caret(start);
    p.handled(Input::Text, "x");
    assert_eq!(p.source(), format!("a x{CITE}!"));
    assert_eq!(p.e.selection(), (start + 1, start + 1));
    // Typing over a range that holds an atom replaces the range, atom included.
    p.e.set_selection(2, end + 2);
    p.handled(Input::Text, "z");
    assert_eq!(p.source(), "a z");
}

#[test]
fn a_bold_atom_has_two_caret_positions_and_its_deletion_leaves_no_stray_markers() {
    let text = format!("a **{CITE}** b");
    let mut p = Page::open(&text, "[]");
    let (start, end) = (4, 4 + n(CITE));
    // Before the atom is before the bold; after it, before the closer.
    p.caret(start);
    assert_eq!(p.e.selection(), (2, 2));
    p.caret(end + 2);
    assert_eq!(p.e.selection(), (end, end));
    p.handled(Input::Backward, "");
    assert_eq!(p.source(), "a  b");
}

#[test]
fn two_adjacent_atoms_share_one_caret_position() {
    let two = format!("{CITE}[B](t3-citation:2)");
    let mut p = Page::open(&format!("{two} x"), "[]");
    assert_eq!(p.e.atoms().len(), 2);
    p.caret(n(CITE));
    assert_eq!(p.e.selection(), (n(CITE), n(CITE)));
    p.handled(Input::Backward, "");
    assert_eq!(p.source(), "[B](t3-citation:2) x");
}

#[test]
fn the_selection_facts_name_one_selected_atom_and_refuse_link_over_it() {
    let text = format!("a {CITE} b");
    let mut p = Page::open(&text, "[]");
    p.e.set_selection(2, 2 + n(CITE));
    let facts = p.e.facts();
    assert_eq!(facts.atom, CITE);
    assert_eq!(facts.link, "t3-citation:1");
    assert!(facts.unavailable.split(' ').any(|c| c == "link"));
    p.e.set_selection(1, 2 + n(CITE));
    assert_eq!(p.e.facts().atom, "");
    assert!(!p.e.facts().unavailable.contains("link"));
}

#[test]
fn a_face_that_arrives_makes_a_gated_token_an_atom_and_moves_a_caret_inside_it() {
    let mut p = Page::open("hi @src/main.rs now", "[]");
    assert!(p.e.atoms().is_empty());
    p.caret(6);
    let change = p.e.set_atoms(Atoms::parse(
        KINDS,
        r#"[{"source":"@src/main.rs","label":"main.rs"}]"#,
    ));
    assert!(change.place && !change.source);
    assert_eq!(p.e.selection(), (15, 15));
    assert_eq!(p.e.visible_text(), "hi main.rs now");
    // A character typed directly after it lengthens the source: a new key, text.
    p.handled(Input::Text, "x");
    assert_eq!(p.source(), "hi @src/main.rsx now");
    assert!(p.e.atoms().is_empty());
}

#[test]
fn set_range_text_replaces_exactly_its_range_and_the_result_is_recognized_again() {
    let text = format!("a {CITE} b");
    let mut p = Page::open(&text, "[]");
    // A new label inside the atom keeps the form: still an atom.
    let now = p.now();
    p.e.replace(3, 8, &u("Other"), SelectionMode::End, now);
    assert_eq!(p.source(), "a [Other](t3-citation:1) b");
    assert_eq!(p.e.atoms().len(), 1);
    // Breaking its form leaves text.
    let now = p.now();
    p.e.replace(2, 3, &[], SelectionMode::End, now);
    assert!(p.e.atoms().is_empty());
}

#[test]
fn a_composition_over_a_range_holding_an_atom_deletes_the_range_first() {
    let text = format!("a {CITE} b");
    let mut p = Page::open(&text, "[]");
    p.e.set_selection(1, 2 + n(CITE) + 1);
    let now = p.now();
    let change = p.e.compose(now);
    assert!(change.source && !change.place);
    assert_eq!(p.source(), "ab");
    // The composed text is read back as typing; undo takes it, then the deletion.
    let now = p.now();
    p.e.reconcile(&u("aかb"), Some((2, 2)), now);
    p.undo();
    assert_eq!(p.source(), "ab");
    p.undo();
    assert_eq!(p.source(), text);
    // A caret composes as before.
    p.e.set_selection(0, 0);
    let now = p.now();
    assert!(!p.e.compose(now).source);
}

#[test]
fn the_drawn_line_hides_an_atoms_source_and_lists_the_atom() {
    let text = format!("a **{CITE}**");
    let p = Page::open(&text, "[]");
    let line = &p.e.lines()[0];
    let (start, end) = (4, 4 + n(CITE));
    assert!(line.segments.contains(&(start, end, HIDDEN)));
    assert_eq!(line.atoms, vec![(start, end, 0, exact_markdown::BOLD)]);
    // Copy takes the source; plain copy, the label.
    let mut p = Page::open(
        &text,
        r#"[{"source":"[Quote](t3-citation:1)","label":"Q"}]"#,
    );
    p.e.set_selection(start, end);
    assert_eq!(p.e.copy_range(), Some((2, end + 2)));
    assert_eq!(p.e.plain(&text), "a Q");
}
