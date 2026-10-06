//! `cargo run -p exact-terminal --example replay -- <bytes-file> [COLS ROWS]`:
//! feed a recorded terminal byte stream to a VT emulator (vt100) and print
//! what a person would see — every row that scrolled into the scrollback,
//! then the screen — so the inline writer can be checked without a
//! terminal (LLP 1101 §5).

fn scrollback_len(parser: &mut vt100::Parser) -> usize {
    parser.set_scrollback(usize::MAX);
    let n = parser.screen().scrollback();
    parser.set_scrollback(0);
    n
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bytes = std::fs::read(&args[0]).expect("a recording");
    let cols: u16 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(80);
    let rows: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(24);
    let mut parser = vt100::Parser::new(rows, cols, 100_000);
    let mut scrolled: Vec<String> = Vec::new();
    let mut seen = 0;
    // Feed in small pieces; after each, read the rows that just scrolled off
    // the top (vt100 shows at most a screen of scrollback at a time).
    for chunk in bytes.chunks(64) {
        parser.process(chunk);
        let len = scrollback_len(&mut parser);
        let new = len - seen;
        if new > 0 {
            assert!(new <= rows as usize, "a chunk scrolled more than a screen");
            parser.set_scrollback(new);
            let contents = parser.screen().contents();
            scrolled.extend(contents.lines().take(new).map(str::to_string));
            parser.set_scrollback(0);
            seen = len;
        }
    }
    println!("── scrollback: {} rows ──", scrolled.len());
    for line in &scrolled {
        println!("{line}");
    }
    println!(
        "── screen ({cols}×{rows}), cursor {:?} ──",
        parser.screen().cursor_position()
    );
    for (i, line) in parser.screen().contents().split('\n').enumerate() {
        println!("{i:2}│{line}");
    }
}
