//! Deterministic bounded synthetic data. No accounts, clocks, random source or I/O.

pub const MAX_DRAFT_CHARS: usize = 512;
pub const MAX_REVISION: usize = 120;
pub const PAGE_SIZE: usize = 100;

#[derive(Clone, Copy, Debug)]
pub struct Controls {
    pub count: usize,
    pub revision: usize,
    pub batch: usize,
}

impl Controls {
    pub fn new(count: usize, revision: usize, batch: usize) -> Result<Self, &'static str> {
        if ![100, 1_000, 10_000, 100_000].contains(&count) {
            return Err("history must be 100, 1000, 10000 or 100000");
        }
        if revision > MAX_REVISION || ![1, 8, 32].contains(&batch) {
            return Err("revision must be 0..120; changed rows must be 1, 8 or 32");
        }
        Ok(Self {
            count,
            revision,
            batch,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub sender: String,
    pub body: String,
    pub outgoing: bool,
    pub meta: String,
}

const BODIES: [&str; 6] = [
    "Sounds good.",
    "The sample is ready. Try typing while the last few messages are updating.",
    "A longer synthetic message wraps naturally when the window gets narrow. Every row has a stable key and no attachment downloads. This is text layout work.",
    "First paragraph: this history is generated entirely on this machine.\n\nSecond paragraph: a long conversation should not make the composer unusable. Scroll and type together, then pause the producer to compare how the app feels.",
    "Review notes\n• Stable message identity\n• Different text lengths\n• Incoming and outgoing bubbles\n• No real conversations\n\nChanging the history size regenerates deterministic data. Manual pages are explicit; they do not track the viewport or recycle rows. Eager mode asks the current runtime to construct the whole selected history.",
    "A deliberately long message exercises wrapping and mixed row heights. Narrow the window and the same content takes more vertical space.\n\nThe producer revises a fixed set of messages near the end of the history. Each revision travels through the actual Rust DataSource, Contract resource, keyed rows and host. No timing result is invented.\n\nThis is a baseline for graceful overload work: a synchronous query can block input, and Stop cannot interrupt work already running. A bounded page limits this example's materialized data; it is not automatic runtime virtualization or a framework scheduling policy.",
];

pub fn history(controls: Controls, echo: &str) -> Result<Vec<Row>, &'static str> {
    page(controls, echo, 0, true)
}

pub fn page(
    controls: Controls,
    echo: &str,
    offset: usize,
    eager: bool,
) -> Result<Vec<Row>, &'static str> {
    // Revalidate at the public boundary even if callers construct Controls directly.
    let controls = Controls::new(controls.count, controls.revision, controls.batch)?;
    if echo.chars().take(MAX_DRAFT_CHARS + 1).count() > MAX_DRAFT_CHARS {
        return Err("local echo is limited to 512 Unicode scalar values");
    }
    if offset >= controls.count || !offset.is_multiple_of(PAGE_SIZE) {
        return Err("page offset must be a multiple of 100 below the history size");
    }
    let start = if eager { 0 } else { offset };
    let end = if eager {
        controls.count
    } else {
        (start + PAGE_SIZE).min(controls.count)
    };
    let mut rows = Vec::with_capacity(end - start + usize::from(!echo.is_empty()));
    for i in start..end {
        let outgoing = i % 3 == 0;
        let mut body = BODIES[i % BODIES.len()].to_string();
        if controls.revision > 0 && i >= controls.count - controls.batch {
            body.push_str(&format!(
                "\n\nSynthetic stream revision {}. {}",
                controls.revision,
                "token ".repeat(controls.revision % 12 + 1)
            ));
        }
        rows.push(Row {
            id: format!("m-{i:06}"),
            sender: if outgoing {
                "You (synthetic)"
            } else {
                ["Ada (synthetic)", "Sam (synthetic)", "Jo (synthetic)"][i % 3]
            }
            .into(),
            body,
            outgoing,
            meta: format!("Message {} · {:02}:{:02}", i + 1, (i / 60) % 24, i % 60),
        });
    }
    if !echo.is_empty() {
        rows.push(Row {
            id: "local-echo".into(),
            sender: "You (local echo)".into(),
            body: echo.into(),
            outgoing: true,
            meta: "Local only · one echo retained · not sent anywhere".into(),
        });
    }
    Ok(rows)
}
