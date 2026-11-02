// Included by markdown.rs: display math with Settings › Appearance › Render math on (mathRenderingEnabled;
// T3 Code 74541bab17 packages/shared/src/markdownMath.ts, MIT, see LICENSE-T3). The reference parses `$$…$$` and
// `\[…\]` as display formulas and a ```math fence as one; the view draws each in the math page (markdown.contract
// MathBlock), so each formula becomes a ```math fence before the portable parser reads the text, which would
// otherwise take `\[` for an escaped bracket. A formula whose delimiters open a line and close a line, at the top
// level outside code fences, is display math here; one inside a line of prose stays text, as inline math does.
// A document typesets its first MAX_FORMULAS formulas, each a web view of its own, as Files draws its first 24
// diagrams (timeline-mermaid.ts filesMermaid); later ones stay TeX source.

const MAX_FORMULAS: usize = 24;

/// The text with each display formula as a ```math fence of its TeX.
fn display_math_fences(text: &str) -> String {
    if !text.contains("$$") && !text.contains("\\[") && !text.contains("math") {
        return text.to_string();
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut fence: Option<(char, usize)> = None;
    let (mut index, mut formulas) = (0, 0);
    while index < lines.len() {
        let line = lines[index];
        let indent = line.len() - line.trim_start_matches(' ').len();
        let trimmed = &line[indent.min(line.len())..];
        if indent <= 3 {
            let run = |c: char| trimmed.chars().take_while(|&x| x == c).count();
            let (tick, tilde) = (run('`'), run('~'));
            let marker = if tick >= 3 { Some(('`', tick)) } else if tilde >= 3 { Some(('~', tilde)) } else { None };
            match (fence, marker) {
                (None, Some(open)) => {
                    fence = Some(open);
                    // An authored ```math fence is a formula too; past the cap it is a `tex` code block.
                    if trimmed[open.1..].trim() == "math" {
                        if formulas >= MAX_FORMULAS {
                            out.push(format!("{}{}tex", &line[..indent], &trimmed[..open.1]));
                            index += 1;
                            continue;
                        }
                        formulas += 1;
                    }
                }
                (Some((c, n)), Some((m, k))) if c == m && k >= n && trimmed[k..].trim().is_empty() => fence = None,
                _ => {}
            }
            if fence.is_none() && marker.is_none() && formulas < MAX_FORMULAS {
                if let Some((tex, end)) = display_formula(&lines, index, trimmed) {
                    let longest = tex.split(|c| c != '`').map(str::len).max().unwrap_or(0);
                    let ticks = "`".repeat(longest.max(2) + 1);
                    out.push(format!("{ticks}math"));
                    out.push(tex);
                    out.push(ticks);
                    formulas += 1;
                    index = end + 1;
                    continue;
                }
            }
        }
        out.push(line.to_string());
        index += 1;
    }
    out.join("\n")
}

/// A display formula opening at `lines[start]` (its text after the indent): its TeX and its last line. The first
/// closer after the opener must end its line; a blank line ends the paragraph first, except inside `$$` lines.
fn display_formula(lines: &[&str], start: usize, opening: &str) -> Option<(String, usize)> {
    let (open, close) = if opening.starts_with("$$") && !opening.starts_with("$$$") {
        ("$$", "$$")
    } else if opening.starts_with("\\[") {
        ("\\[", "\\]")
    } else {
        return None;
    };
    // `$$` alone on its line opens math flow, which may hold blank lines.
    let flow = open == "$$" && opening.trim_end() == "$$";
    let mut tex = String::new();
    let mut rest = &opening[open.len()..];
    let mut index = start;
    loop {
        if let Some(at) = rest.find(close) {
            if !rest[at + close.len()..].trim().is_empty() {
                return None;
            }
            tex.push_str(&rest[..at]);
            let tex = tex.trim().to_string();
            return (!tex.is_empty()).then_some((tex, index));
        }
        tex.push_str(rest);
        tex.push('\n');
        index += 1;
        rest = lines.get(index)?;
        if rest.trim().is_empty() && !flow {
            return None;
        }
    }
}

#[cfg(test)]
mod display_math_tests {
    use super::display_math_fences;

    #[test]
    fn display_formulas_become_math_fences() {
        assert_eq!(display_math_fences("$$\nE = mc^2\n$$"), "```math\nE = mc^2\n```");
        assert_eq!(display_math_fences("$$a+b$$"), "```math\na+b\n```");
        assert_eq!(display_math_fences("Where\n\\[\n\\begin{aligned} a &= b \\end{aligned}\n\\]\nholds."), "Where\n```math\n\\begin{aligned} a &= b \\end{aligned}\n```\nholds.");
        assert_eq!(display_math_fences("$$\nx\n\ny\n$$"), "```math\nx\n\ny\n```");
        // A fence of the TeX's own backticks is longer than they are.
        assert_eq!(display_math_fences("$$\\text{```}$$"), "````math\n\\text{```}\n````");
        // The first 24 formulas of a document; the rest stay source.
        let many = vec!["$$x$$"; 26].join("\n\n");
        let fenced = display_math_fences(&many);
        assert_eq!(fenced.matches("```math").count(), 24);
        assert!(fenced.ends_with("$$x$$\n\n$$x$$"));
        let fences = vec!["```math\nx\n```"; 25].join("\n\n");
        assert!(display_math_fences(&fences).ends_with("```tex\nx\n```"));
        assert_eq!(display_math_fences(&fences).matches("```math").count(), 24);
    }

    #[test]
    fn prose_code_and_unclosed_formulas_stay_text() {
        for text in [
            "Inline $x^2$ and display $$a+b$$ in a sentence.",
            "\\[1\\] and \\[2, 3\\] are citations.",
            "```\n$$\nx\n$$\n```",
            "    $$x$$",
            "\\[\nx^2\n\nmore",
            "$$\nx",
            "$$ $$",
            "$$$x$$$",
            "- $$x$$",
        ] {
            assert_eq!(display_math_fences(text), text, "{text}");
        }
    }
}
