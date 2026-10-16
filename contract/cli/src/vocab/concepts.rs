//! The concepts `contract vocab` answers with a whole idiom (LLP 1116.004
//! D5.1): scrolling to a place, loading before data, a count's plural, a
//! choice of one value, a modal, the date, a kept setting, swipe actions, a
//! confirmation, and the grammar's words (`task`, `send`, `match`, `for`,
//! `if`, `let`, `fn`, `shape`, `props`, `derive`, `action`, `resource`,
//! `mutation`, `stepper`; LLP 1116.007 D1). Builders who found no recipe read
//! `contract-grammar` for these; one lookup now holds the repair. The entries
//! are `concepts.md`'s sections, data rather than code, so a new concept is
//! a section there.

/// One concept: its name, what it does, the other names it answers to, and
/// its answer as Markdown (sentences, a `contract` block, the recipe).
pub struct Concept {
    pub name: &'static str,
    pub title: &'static str,
    pub names: Vec<&'static str>,
    pub answer: &'static str,
}

const SOURCE: &str = include_str!("concepts.md");

/// Every concept, in `concepts.md`'s order.
pub fn concepts() -> Vec<Concept> {
    let mut out = Vec::new();
    for section in SOURCE.split("\n## ").skip(1) {
        let (head, rest) = section.split_once('\n').unwrap_or((section, ""));
        let (name, title) = head.split_once(": ").unwrap_or((head, ""));
        let (names, answer) = match rest.strip_prefix("names: ") {
            Some(list) => {
                let (line, answer) = list.split_once('\n').unwrap_or((list, ""));
                (line.split(", ").map(str::trim).collect(), answer)
            }
            None => (Vec::new(), rest),
        };
        out.push(Concept {
            name: name.trim(),
            title: title.trim(),
            names,
            answer: answer.trim(),
        });
    }
    out
}

/// The concept a name looks up: its own name or one it answers to.
pub fn concept(name: &str) -> Option<Concept> {
    concepts()
        .into_iter()
        .find(|c| c.name == name || c.names.contains(&name))
}

impl Concept {
    /// The bodies of its `contract` blocks, each a whole app.
    pub fn contract(&self) -> Vec<String> {
        let mut blocks = Vec::new();
        let mut open: Option<String> = None;
        for line in self.answer.lines() {
            match &mut open {
                Some(body) if line.trim() == "```" => {
                    blocks.push(std::mem::take(body));
                    open = None;
                }
                Some(body) => {
                    body.push_str(line);
                    body.push('\n');
                }
                None if line.trim() == "```contract" => open = Some(String::new()),
                None => {}
            }
        }
        blocks
    }

    /// Its text entry: a head naming the concept, then the answer whole.
    pub fn text(&self) -> String {
        format!(
            "{} (a concept): {}\n{}\n",
            self.name, self.title, self.answer
        )
    }
}
