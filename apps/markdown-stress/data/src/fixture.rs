//! Reproducible Markdown inputs, generated from bounded, explicit controls.
use std::fmt::Write;

/// Same upper bound as the shipped reader's file ingress (LLP 1033).
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
/// Explicit source byte budgets; actual bytes are reported, never rounded up.
pub const SIZES: [usize; 4] = [16 * 1024, 256 * 1024, 1024 * 1024, MAX_BYTES];

/// Each profile stresses a different parser/layout shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// Mixed prose, emphasis, code, tables, quotes and lists.
    Mixed,
    /// One paragraph, deliberately never split into smaller blocks.
    Paragraph,
    /// One giant fenced code block.
    Code,
    /// One giant four-column pipe table.
    Table,
    /// Many small headings and paragraphs.
    Blocks,
}

impl Profile {
    /// Every supported profile, in control order.
    pub const ALL: [Self; 5] = [
        Self::Mixed,
        Self::Paragraph,
        Self::Code,
        Self::Table,
        Self::Blocks,
    ];

    /// The stable Contract/CLI identifier.
    pub fn name(self) -> &'static str {
        match self {
            Self::Mixed => "mixed",
            Self::Paragraph => "paragraph",
            Self::Code => "code",
            Self::Table => "table",
            Self::Blocks => "blocks",
        }
    }

    /// Refuse an unknown identifier before allocating any document.
    pub fn parse(name: &str) -> Result<Self, &'static str> {
        Self::ALL
            .into_iter()
            .find(|p| p.name() == name)
            .ok_or("unknown fixture profile")
    }
}

/// Generate complete Markdown constructs within `budget`, with stable numbering.
/// No truncation of UTF-8 or Markdown syntax, no clock, randomness or external I/O.
pub fn generate(profile: Profile, budget: usize) -> Result<String, &'static str> {
    if !SIZES.contains(&budget) {
        return Err("source budget must be 16384, 262144, 1048576 or 4194304 bytes");
    }
    let mut out = String::with_capacity(budget);
    writeln!(out, "# Markdown stress: {}\n", profile.name()).unwrap();
    if profile == Profile::Code {
        out.push_str("```rust\n");
    }
    if profile == Profile::Table {
        out.push_str("| Sequence | State | Payload | Note |\n| --- | --- | --- | --- |\n");
    }
    let suffix = if profile == Profile::Code {
        "```\n"
    } else {
        "\n"
    };
    let prose = "A long paragraph keeps its complete text while the window changes width. Reading position, input and scroll responsiveness matter. Café 🦀 東京. ";
    for index in 0.. {
        let chunk = match profile {
            Profile::Paragraph => prose.to_string(),
            Profile::Code => format!("let sample_{index:06} = (\"synthetic line\", {index}, \"wrap this source when the window narrows\");\n"),
            Profile::Table => format!("| {index:06} | **ready** | `value_{index:06}` | Four real cells with varied wrapping, café 🦀 |\n"),
            Profile::Blocks => format!("## Section {index:06}\n\nSmall block {index:06}: **bold**, `code`, and ordinary prose.\n\n"),
            Profile::Mixed => format!("## Section {index:06}\n\n{}**Emphasis**, *italics*, and `inline code` remain real styled runs.\n\n```rust\nlet section = {index};\nlet wrap = \"{}\";\n```\n\n| Key | Value |\n| --- | --- |\n| section | {index:06} |\n| prose | **styled cell** |\n\n> A quotation with a different inset.\n\n- First item\n- Second item with `code`\n\n", prose.repeat(6), prose.repeat(2)),
        };
        if out.len() + chunk.len() + suffix.len() > budget {
            break;
        }
        out.push_str(&chunk);
    }
    out.push_str(suffix);
    Ok(out)
}
