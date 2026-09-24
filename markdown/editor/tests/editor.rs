//! The editing rules, driven the way a browser drives them: `beforeinput`
//! events, and plain typing the platform performs itself and reports back.

use exact_markdown_editor::{view::HIDDEN, Deco, Editor, Input, Outcome, Selected};

fn u(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

struct Page {
    e: Editor,
    now: f64,
}

impl Page {
    fn open(text: &str) -> Self {
        let mut e = Editor::new();
        e.load(&u(text));
        Self { e, now: 0.0 }
    }

    fn source(&self) -> String {
        String::from_utf16(self.e.source()).unwrap()
    }

    fn tick(&mut self) -> f64 {
        self.now += 40.0;
        self.now
    }

    /// A click or arrow key: the platform puts the caret at `p`.
    fn caret(&mut self, p: u32) {
        if self.e.select(p, p, None) == Selected::Place {
            // The host places the snapped caret; its echo is ignored.
            let (a, b) = self.e.selection();
            assert_eq!(self.e.select(a, b, Some((a, b))), Selected::Ignore);
        }
    }

    fn input(&mut self, input: Input, data: &str) -> Outcome {
        let now = self.tick();
        self.e.before_input(input, &u(data), None, now)
    }

    /// Typing, a key at a time; a key the editor leaves to the platform is
    /// inserted at the caret and read back, as a contentEditable does.
    fn type_(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                self.input(Input::Paragraph, "");
                continue;
            }
            let data = ch.to_string();
            if self.input(Input::Text, &data) == Outcome::Native {
                self.platform_types(&data);
            }
        }
    }

    fn platform_types(&mut self, data: &str) {
        let (a, b) = self.e.selection();
        let mut text = self.e.source().to_vec();
        let data = u(data);
        text.splice(a as usize..b as usize, data.iter().copied());
        let at = a + data.len() as u32;
        let now = self.tick();
        self.e.reconcile(&text, Some((at, at)), now);
    }

    fn backspace(&mut self) {
        assert!(matches!(
            self.input(Input::Backward, ""),
            Outcome::Handled(_)
        ));
    }

    fn tool(&mut self, name: &str) {
        let now = self.tick();
        self.e.command(name, "", now);
    }
}

#[test]
fn pending_bold_then_plain() {
    let mut p = Page::open("");
    p.type_("Hello ");
    p.tool("bold");
    p.type_("world");
    p.tool("bold");
    p.type_(" again");
    assert_eq!(p.source(), "Hello **world** again");
    assert_eq!(p.e.visible_text(), "Hello world again");
}

#[test]
fn bold_input_event_toggles_pending_bold() {
    let mut p = Page::open("");
    p.type_("a ");
    p.input(Input::Bold, "");
    p.type_("b");
    p.input(Input::Bold, "");
    p.type_(" c");
    assert_eq!(p.source(), "a **b** c");
}

#[test]
fn a_space_at_the_end_of_bold_goes_outside() {
    let mut p = Page::open("");
    p.tool("bold");
    p.type_("bold");
    p.type_(" plain");
    assert_eq!(p.source(), "**bold** plain");
}

#[test]
fn typing_after_a_typed_closer_stays_outside() {
    let mut p = Page::open("");
    p.type_("**hi**");
    p.type_("x");
    assert_eq!(p.source(), "**hi**x");
}

#[test]
fn heading_and_list_shortcuts_continue_and_end_lists() {
    let mut p = Page::open("");
    p.type_("# Title\n- one\n");
    p.type_("two\n\n");
    p.type_("after");
    assert!(
        p.source().starts_with("# Title\n- one\n- two\n"),
        "{:?}",
        p.source()
    );
    assert!(p.source().ends_with("after"), "{:?}", p.source());
}

#[test]
fn backspace_deletes_letters_never_markers() {
    let mut p = Page::open("plain **bold** end");
    for _ in 0..4 {
        p.backspace();
    }
    assert_eq!(p.source(), "plain **bold**");
    p.backspace();
    assert_eq!(p.source(), "plain **bol**");
}

#[test]
fn emptying_a_bold_word_removes_its_markers() {
    let mut p = Page::open("a **b** c");
    for _ in 0..3 {
        p.backspace();
    }
    assert_eq!(p.source(), "a ");
}

#[test]
fn backspace_at_a_heading_start_drops_the_heading() {
    let mut p = Page::open("para\n# Title");
    p.caret(7);
    p.backspace();
    assert_eq!(p.source(), "para\nTitle");
}

#[test]
fn backspace_in_an_empty_item_drops_the_bullet() {
    let mut p = Page::open("- one\n- two");
    for _ in 0..3 {
        p.backspace();
    }
    assert_eq!(p.source(), "- one\n- ");
    p.backspace();
    assert_eq!(p.source(), "- one\n");
}

#[test]
fn backspace_after_a_link_deletes_its_text() {
    let mut p = Page::open("see [link](https://x.com)");
    p.backspace();
    assert_eq!(p.source(), "see [lin](https://x.com)");
}

#[test]
fn select_all_and_type_replaces_everything() {
    let mut p = Page::open("plain **bold** and *it*");
    let len = p.e.source().len() as u32;
    p.e.select(0, len, None);
    p.type_("Z");
    assert_eq!(p.source(), "Z");
}

#[test]
fn replacing_a_selected_bold_word_keeps_it_bold() {
    let mut p = Page::open("plain **bold** end");
    p.e.select(8, 12, None);
    p.type_("X");
    assert_eq!(p.source(), "plain **X** end");
}

#[test]
fn a_click_after_a_bold_word_types_into_it() {
    let mut p = Page::open("plain **bold** end");
    p.caret(12);
    p.type_("!");
    assert_eq!(p.source(), "plain **bold!** end");
}

#[test]
fn a_click_before_a_bold_word_types_before_it() {
    let mut p = Page::open("plain **bold** end");
    p.caret(8);
    assert_eq!(p.e.selection(), (6, 6), "the caret snaps before the opener");
    p.type_("x");
    assert_eq!(p.source(), "plain x**bold** end");
}

#[test]
fn a_caret_past_hidden_markers_types_between_letters() {
    // Four right arrows from the start of `ab cd ef` land after `c`.
    let mut p = Page::open("ab **cd** ef");
    p.caret(6);
    p.type_("X");
    assert_eq!(p.source(), "ab **cXd** ef");
}

#[test]
fn a_task_box_toggles() {
    let mut p = Page::open("- [ ] task");
    let now = p.tick();
    p.e.toggle_task(0, now);
    assert_eq!(p.source(), "- [x] task");
}

#[test]
fn one_undo_removes_a_formatted_insertion() {
    let mut p = Page::open("");
    p.type_("Hello");
    p.now += 1600.0;
    p.tool("bold");
    p.type_("X");
    p.input(Input::Undo, "");
    assert_eq!(p.source(), "Hello");
    p.input(Input::Undo, "");
    assert_eq!(p.source(), "");
}

#[test]
fn undo_and_redo_a_list() {
    let mut p = Page::open("one");
    p.tool("bullet");
    assert_eq!(p.source(), "- one");
    p.tool("undo");
    assert_eq!(p.source(), "one");
    p.tool("redo");
    assert_eq!(p.source(), "- one");
}

#[test]
fn composition_commits_into_bold() {
    let mut p = Page::open("**ab**");
    p.caret(3);
    p.platform_types("日本");
    assert_eq!(p.source(), "**a日本b**");
    let mut p = Page::open("**ab** x");
    p.caret(4);
    p.platform_types("仮名");
    assert_eq!(p.source(), "**ab仮名** x");
}

#[test]
fn pasted_markdown_stays_markdown() {
    let mut p = Page::open("x ");
    p.type_("pasted **bold** text");
    assert_eq!(p.source(), "x pasted **bold** text");
    let mut p = Page::open("x ");
    p.input(Input::Paste, "pasted **bold**\r\ntext");
    assert_eq!(p.source(), "x pasted **bold**\ntext");
}

#[test]
fn toolbar_heading_and_quote() {
    let mut p = Page::open("hello");
    p.tool("heading1");
    assert_eq!(p.source(), "# hello");
    p.tool("quote");
    assert_eq!(p.source(), "> # hello");
    let mut p = Page::open("hello");
    let now = p.tick();
    p.e.command("heading", "2", now);
    assert_eq!(
        p.source(),
        "## hello",
        "the Contract form: format(id, \"heading\", \"2\")"
    );
}

#[test]
fn a_link_at_a_caret_is_labelled_by_its_host() {
    let mut p = Page::open("see ");
    let now = p.tick();
    p.e.command("link", "https://example.com/a/b", now);
    assert_eq!(p.source(), "see [example.com](https://example.com/a/b)");
    let now = p.tick();
    p.e.command("link", "", now);
    assert_eq!(
        p.source(),
        "see [example.com](https://example.com/a/b)",
        "an empty URL inserts nothing"
    );
}

#[test]
fn backspace_takes_the_platforms_grapheme() {
    // 👍🏽 is four code units; the platform's target range covers all four.
    let mut p = Page::open("a 👍🏽");
    let now = p.tick();
    p.e.before_input(Input::Backward, &[], Some((2, 6)), now);
    assert_eq!(p.source(), "a ");
    // A target that crosses hidden syntax is not trusted.
    let mut p = Page::open("**b** c");
    p.caret(6);
    let now = p.tick();
    p.e.before_input(Input::Backward, &[], Some((2, 6)), now);
    assert_eq!(p.source(), "**b**c");
}

#[test]
fn an_app_write_keeps_the_caret_and_resets_history() {
    let mut p = Page::open("hello world");
    p.caret(8);
    let change = p.e.set_value(&u("hi hello world"));
    assert!(change.source && change.place);
    assert_eq!(p.e.selection(), (11, 11));
    p.input(Input::Undo, "");
    assert_eq!(p.source(), "hi hello world");
    assert!(
        !p.e.set_value(&u("hi hello world")).source,
        "an echo changes nothing"
    );
    let mut p = Page::open("a\nb");
    assert!(
        !p.e.set_value(&u("a\r\nb")).source,
        "the same text with other line endings"
    );
}

#[test]
fn line_endings_and_lone_surrogates_are_normalized_once() {
    let mut e = Editor::new();
    let mut text = u("a\r\nb\rc");
    text.push(0xD800);
    e.load(&text);
    assert_eq!(String::from_utf16(e.source()).unwrap(), "a\nb\nc\u{FFFD}");
}

#[test]
fn pending_formats_show_in_the_toolbar_facts() {
    let mut p = Page::open("plain");
    p.tool("bold");
    assert_eq!(p.e.facts().formats, "bold");
    p.tool("bold");
    assert_eq!(p.e.facts().formats, "");
    let mut p = Page::open("**b**");
    p.caret(3);
    assert_eq!(p.e.facts().formats, "bold");
    p.tool("bold");
    assert_eq!(
        p.e.facts().formats,
        "",
        "turning bold off at a caret inside it"
    );
    p.type_("x");
    assert_eq!(p.source(), "**b**x");
}

#[test]
fn lines_hide_syntax_and_carry_paragraph_styles() {
    let e = {
        let mut e = Editor::new();
        e.load(&u("# Title\n- **one**\n1. two\n```\ncode\n```"));
        e
    };
    let lines = e.lines();
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[0].heading, 1);
    assert_eq!(lines[0].segments, vec![(0, 2, HIDDEN), (2, 7, 0)]);
    assert_eq!(lines[1].deco, Some(Deco::Bullet));
    let shown: Vec<_> = lines[1].segments.iter().filter(|s| s.2 != HIDDEN).collect();
    assert_eq!(shown, vec![&(12, 15, 1)]);
    assert!(matches!(lines[2].deco, Some(Deco::Number(18, 20))));
    use exact_markdown_editor::view::flags;
    assert_ne!(lines[3].flags & flags::COLLAPSED, 0);
    assert_eq!(
        lines[4].flags & (flags::CODE | flags::CODE_FIRST | flags::CODE_LAST),
        flags::CODE | flags::CODE_FIRST | flags::CODE_LAST
    );
    assert_ne!(lines[5].flags & flags::COLLAPSED, 0);
    assert_eq!(e.visible_text(), "Title\none\ntwo\ncode\n");
}

#[test]
fn native_range_readback_matches_whole_source_including_history() {
    for (source, from, to, replacement, caret) in [
        ("a\n\nb", 2, 2, "x", 3),
        ("first\n**hello**\nlast", 6, 15, "**héllo😀**", 13),
        ("before\ntext\nafter", 7, 11, "text*", 12),
        ("a\nb\nc", 2, 3, "b\nnew", 7),
    ] {
        let mut range = Editor::new();
        let mut whole = Editor::new();
        let original = u(source);
        range.load(&original);
        whole.load(&original);
        let mut next = original.clone();
        next.splice(from as usize..to as usize, u(replacement));
        let selection = Some((caret, caret));
        range.reconcile_range(from, to, &u(replacement), selection, 10.0);
        whole.reconcile(&next, selection, 10.0);
        assert_eq!(range.source(), whole.source());
        assert_eq!(range.selection(), whole.selection());
        assert_eq!(range.lines(), whole.lines());
        for command in ["undo", "redo"] {
            range.command(command, "", 20.0);
            whole.command(command, "", 20.0);
            assert_eq!(range.source(), whole.source());
            assert_eq!(range.selection(), whole.selection());
        }
    }
}
