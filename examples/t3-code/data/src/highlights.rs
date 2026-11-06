//! The reference's code colours: @pierre/highlights 0.2.0's lexers (`highlights.wasm`, Apache-2.0; its licence and
//! notice ship in assets/vendor/highlights/), which the reference T3 Code highlights all code with. Hermes runs no
//! WebAssembly, so this crate runs them with wasmi, and the TypeScript module asks for their tokens through the root
//! (`highlightCode`; src/highlight/highlights.ts says how).
//!
//! The module answers in two ways, each its own JavaScript's:
//! - A whole text (dist/highlighter.js): its UTF-8 bytes at 64 KiB with a NUL after them, the lexer id at byte 0, the
//!   output mode at byte 1 and the length at bytes 2-5; `highlight()` leaves the output's offset and length at bytes
//!   6-13. Mode 3 is UTF-16 line records: pairs of a token's end offset and its type (dist/token-types.js), and at each
//!   line break the next line's start offset and `u32::MAX`.
//! - A document (dist/live.js, the LiveTokenizer @pierre/diffs' stream tokenizer appends to): an instance of its own
//!   keeps a text's lines and the lexer state each starts in. An edit marks the lines from the one it changes, and
//!   `liveRun` lexes them until a line ends in the state it ended in before. A long text is a document lexed a budget
//!   at a time over several calls, and a streaming block appends to its document, so only the line it continues and
//!   its new lines are lexed again.
//!
//! The lexers share the crate's one worker thread with the chat Markdown, so a call spends about [`BUDGET`] on them:
//! staging a document's text counts, and is done whole lines at a time too. What does not fit what is left waits for
//! the next call, except a call's first step, which runs past the budget by at most one line staged or one slice of a
//! line lexed (a line that does not fit is lexed a slice at a time; a whole text that cannot fit a call of its own is a
//! document instead). Every call into the module carries fuel and every store a memory limit: a lexer that loops or
//! grows without end traps, which drops its instance, and its text paints plain.

use exact_plan::Value;
use std::cell::RefCell;
use wasmi::{
    Caller, Config, Engine, Instance, Linker, Memory, Module, Store, StoreLimits,
    StoreLimitsBuilder, TypedFunc, WasmParams, WasmResults,
};

const LEXERS: &[u8] = include_bytes!("../highlights.wasm");
const PAGE: usize = 65_536;
/// The output mode that answers UTF-16 line records.
const LINE_RECORDS: u8 = 3;
/// The fuel one `highlightCode` call spends lexing: about 17 ms on an M5 Pro (23 ms at most measured, beside other
/// builds), where wasmi runs the lexers at about a billion fuel a second: 3 MB/s of TypeScript, 1.5 MB/s of the
/// slowest text measured. A text of 1M characters takes 30 calls.
pub const BUDGET: u64 = 16_000_000;
/// A text of up to this many UTF-8 bytes is lexed whole in one call, which takes at most about a budget; a longer one
/// is a document (highlights.ts sends it as one).
pub const WHOLE_BYTES: usize = 32 * 1024;
/// The fuel lexing is planned to take when a call decides what fits its budget: about 410 a byte and 4,000 a line
/// (measured: a TypeScript line of 80 bytes took 37,000, a blank line 3,900).
const PLANNED_PER_BYTE: u64 = 410;
const PLANNED_PER_LINE: u64 = 4_000;
/// A call into the module may spend this, and this much for each byte and line it is handed, before it traps: about
/// five times what the slowest texts measured take (860 a byte, 3,900 a blank line), so only a lexer that loops runs
/// out.
const FUEL_BASE: u64 = 4_000_000;
const FUEL_PER_BYTE: u64 = 4_000;
const FUEL_PER_LINE: u64 = 20_000;
/// Staging a document's text (its line table) takes up to about 290 fuel a byte, for a text of blank lines; an append
/// stages the line it continues again. Its calls may spend more than three times that.
const STAGE_PER_BYTE: u64 = 1_000;
/// The fuel staging is planned to take: about 12 a byte and 400 a line (measured: TypeScript took 990 a line of 80
/// bytes, blank lines 210 to 290 each).
const STAGE_PLANNED_PER_BYTE: u64 = 12;
const STAGE_PLANNED_PER_LINE: u64 = 400;
/// A store's memory: a document of a 1 MB text takes about 4 MiB.
const MEMORY_BYTES: usize = 64 << 20;
/// dist/highlighter.js assertShared: past this many pages the shared instance is made again for a text of one page.
const RECYCLE_PAGES: usize = 128;
/// A line of more bytes than this, or one that does not fit what is left of a call's budget, is lexed in slices:
/// `liveRun`'s work as dist/live.js #workSlice passes it, a slice taking up to about 1.7 million fuel (measured).
const LONG_LINE: usize = 16 * 1024;
const WORK: u32 = 4_096;
const SLICE_PLANNED: u64 = 1_800_000;
/// The documents kept, and the pages (4 MiB, a document of about 1 MB) past which a finished one is dropped: its next
/// edit starts it again.
const DOCUMENTS: usize = 8;
const KEEP_PAGES: usize = 64;

/// One instance of the module, in a store of its own.
struct Lexers {
    store: Store<StoreLimits>,
    instance: Instance,
    memory: Memory,
}

/// The module's two imports: whether the code point whose UTF-8 bytes are at `ptr` is Unicode ID_Start (`start`) or
/// ID_Continue, as its JavaScript answers with `\p{ID_Start}` and `[‌‍\p{ID_Continue}]`. The lexers ask
/// only for code points outside ASCII.
fn id_class(caller: &Caller<'_, StoreLimits>, ptr: i32, len: i32, start: bool) -> i32 {
    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        return 0;
    };
    let (ptr, len) = (ptr as usize, len as usize);
    let character = memory
        .data(caller)
        .get(ptr..ptr + len)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .and_then(|text| text.chars().next());
    character.is_some_and(|c| {
        if start {
            is_id_start(c)
        } else {
            is_id_continue(c)
        }
    }) as i32
}

/// Unicode's ID_Start: letters, letter numbers and Other_ID_Start, less Pattern_Syntax.
fn is_id_start(c: char) -> bool {
    use unicode_general_category::GeneralCategory::*;
    matches!(
        c,
        '\u{1885}' | '\u{1886}' | '\u{2118}' | '\u{212E}' | '\u{309B}' | '\u{309C}'
    ) || c != '\u{2E2F}'
        && matches!(
            unicode_general_category::get_general_category(c),
            UppercaseLetter
                | LowercaseLetter
                | TitlecaseLetter
                | ModifierLetter
                | OtherLetter
                | LetterNumber
        )
}

/// Unicode's ID_Continue (with ZWNJ and ZWJ): ID_Start, marks, digits, connector punctuation and Other_ID_Continue.
fn is_id_continue(c: char) -> bool {
    use unicode_general_category::GeneralCategory::*;
    is_id_start(c)
        || matches!(
            c,
            '\u{00B7}' | '\u{0387}' | '\u{1369}'
                ..='\u{1371}' | '\u{19DA}' | '\u{200C}' | '\u{200D}' | '\u{30FB}' | '\u{FF65}'
        )
        || matches!(
            unicode_general_category::get_general_category(c),
            NonspacingMark | SpacingMark | DecimalNumber | ConnectorPunctuation
        )
}

fn failed(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// How many lines `text` has: its line breaks and one.
fn lines_of(text: &str) -> u64 {
    text.bytes().filter(|&byte| byte == b'\n').count() as u64 + 1
}

#[cfg(test)]
thread_local! {
    /// Tests: a call's budget, so a short text takes several calls, and the fuel any one call into the module gets at
    /// most, so one can make it trap.
    static TESTING: std::cell::Cell<(u64, u64)> = const { std::cell::Cell::new((BUDGET, u64::MAX)) };
}

fn budget() -> u64 {
    #[cfg(test)]
    return TESTING.with(std::cell::Cell::get).0;
    #[cfg(not(test))]
    BUDGET
}

impl Lexers {
    fn new(engine: &Engine, module: &Module) -> Result<Lexers, String> {
        let limits = StoreLimitsBuilder::new()
            .memory_size(MEMORY_BYTES)
            .memories(1)
            .instances(1)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(engine, limits);
        store.limiter(|limits| limits);
        store.set_fuel(FUEL_BASE).map_err(failed)?;
        let mut linker = Linker::<StoreLimits>::new(engine);
        linker
            .func_wrap(
                "env",
                "is_id_start",
                |caller: Caller<'_, StoreLimits>, ptr: i32, len: i32| {
                    id_class(&caller, ptr, len, true)
                },
            )
            .and_then(|linker| {
                linker.func_wrap(
                    "env",
                    "is_id_continue",
                    |caller: Caller<'_, StoreLimits>, ptr: i32, len: i32| {
                        id_class(&caller, ptr, len, false)
                    },
                )
            })
            .map_err(failed)?;
        let instance = linker
            .instantiate_and_start(&mut store, module)
            .map_err(failed)?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or("the lexers export no memory")?;
        Ok(Lexers {
            store,
            instance,
            memory,
        })
    }

    fn pages(&self) -> usize {
        self.memory.data(&self.store).len() / PAGE
    }

    fn export<P: WasmParams, R: WasmResults>(&self, name: &str) -> Result<TypedFunc<P, R>, String> {
        self.instance
            .get_typed_func::<P, R>(&self.store, name)
            .map_err(|error| format!("{name}: {error}"))
    }

    /// `function(params)` with `fuel`, and the fuel it took.
    fn call<P: WasmParams, R: WasmResults>(
        &mut self,
        function: &TypedFunc<P, R>,
        params: P,
        fuel: u64,
    ) -> Result<(R, u64), String> {
        #[cfg(test)]
        let fuel = fuel.min(TESTING.with(std::cell::Cell::get).1);
        self.store.set_fuel(fuel).map_err(failed)?;
        let result = function.call(&mut self.store, params).map_err(failed)?;
        Ok((result, fuel - self.store.get_fuel().unwrap_or(0)))
    }

    fn word(&self, at: usize) -> Result<u32, String> {
        let bytes = self
            .memory
            .data(&self.store)
            .get(at..at + 4)
            .ok_or("a read outside the lexers' memory")?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn write(&mut self, at: usize, bytes: &[u8]) -> Result<(), String> {
        self.memory
            .data_mut(&mut self.store)
            .get_mut(at..at + bytes.len())
            .ok_or("a write outside the lexers' memory")?
            .copy_from_slice(bytes);
        Ok(())
    }

    /// The UTF-16 line records of the whole of `text` under lexer `language`, and the fuel they took.
    fn whole(&mut self, language: u8, text: &str) -> Result<(Vec<u32>, u64), String> {
        let bytes = text.as_bytes();
        // dist/highlighter.js #growMemoryIfNeeded: one page for the header, the input and 96 bytes of slack after it.
        let pages = 1 + (bytes.len() + 96).div_ceil(PAGE);
        let have = self.pages();
        if pages > have {
            self.memory
                .grow(&mut self.store, (pages - have) as u64)
                .map_err(failed)?;
        }
        self.write(PAGE, bytes)?;
        self.write(PAGE + bytes.len(), &[0])?;
        self.write(0, &[language, LINE_RECORDS])?;
        self.write(2, &(bytes.len() as u32).to_le_bytes())?;
        let highlight = self.export::<(), ()>("highlight")?;
        let fuel = FUEL_BASE + FUEL_PER_BYTE * bytes.len() as u64 + FUEL_PER_LINE * lines_of(text);
        let ((), used) = self.call(&highlight, (), fuel)?;
        let (start, length) = (self.word(6)? as usize, self.word(10)? as usize);
        let output = self
            .memory
            .data(&self.store)
            .get(start..start + length)
            .ok_or("the lexers' output is outside their memory")?;
        let records = output
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        Ok((records, used))
    }
}

/// A document's entry points (dist/live.js LiveWasmExports).
struct Live {
    stage: TypedFunc<u32, u32>,
    init: TypedFunc<(u32, u32, u32), ()>,
    edit: TypedFunc<(u32, u32), ()>,
    run: TypedFunc<(u32, u32, u32), u32>,
    stats: TypedFunc<u32, u32>,
    count: TypedFunc<(), u32>,
    len: TypedFunc<u32, u32>,
    bytes: TypedFunc<u32, u32>,
    flags: TypedFunc<u32, u32>,
    tokens: TypedFunc<u32, u32>,
    records: TypedFunc<u32, u32>,
}

/// A text the module keeps: a long one lexed over several calls, or a streaming block appended to.
struct Document {
    id: u32,
    language: u8,
    lexers: Lexers,
    live: Live,
    /// The text's length in UTF-16 units, where an append goes: what the module holds and `unstaged`.
    length: usize,
    /// The end of the text, not yet handed to the module (`stage`).
    unstaged: String,
    /// The lines whose records an answer has given as final: all but the last line of the text it answered.
    delivered: u32,
    used: u64,
}

/// The fuel of one of the small reads of a document's lines.
const READ_FUEL: u64 = 100_000;
/// `liveStats` entries: non-zero while lines wait to be lexed, and the first of them (dist/live.js
/// pendingTokenization, #workSlice).
const WAITING: u32 = 9;
const NEXT_LINE: u32 = 10;

impl Document {
    fn new(
        engine: &Engine,
        module: &Module,
        id: u32,
        language: u8,
        text: &str,
    ) -> Result<(Document, u64), String> {
        let mut lexers = Lexers::new(engine, module)?;
        let live = Live {
            stage: lexers.export("liveStage")?,
            init: lexers.export("liveInitDoc")?,
            edit: lexers.export("liveApplyEdits")?,
            run: lexers.export("liveRun")?,
            stats: lexers.export("liveStats")?,
            count: lexers.export("liveLineCount")?,
            len: lexers.export("liveLineLen")?,
            bytes: lexers.export("liveLineByteLen")?,
            flags: lexers.export("liveLineFlags")?,
            tokens: lexers.export("liveLineTokCount")?,
            records: lexers.export("liveLineTokPtr")?,
        };
        // An empty document, which `stage` hands the text a budget at a time.
        let (ptr, staged) = lexers.call(&live.stage, 0, FUEL_BASE)?;
        let ((), split) = lexers.call(&live.init, (ptr, 0, language as u32), FUEL_BASE)?;
        let document = Document {
            id,
            language,
            lexers,
            live,
            length: text.encode_utf16().count(),
            unstaged: text.to_string(),
            delivered: 0,
            used: 0,
        };
        Ok((document, staged + split))
    }

    fn read(
        &mut self,
        function: fn(&Live) -> &TypedFunc<u32, u32>,
        line: u32,
    ) -> Result<u32, String> {
        let function = *function(&self.live);
        Ok(self.lexers.call(&function, line, READ_FUEL)?.0)
    }

    fn line_count(&mut self) -> Result<u32, String> {
        let count = self.live.count;
        Ok(self.lexers.call(&count, (), READ_FUEL)?.0)
    }

    /// Hands the module the start of `unstaged`, whole lines while they are planned to fit `allowance`, and with `first`
    /// at least one: an append stages the line it continues again, so a line is staged whole, not in parts. The fuel
    /// it took.
    fn stage(&mut self, allowance: u64, first: bool) -> Result<u64, String> {
        if self.unstaged.is_empty() {
            return Ok(0);
        }
        let last = self.line_count()? - 1;
        let mut planned = STAGE_PLANNED_PER_BYTE * self.read(|live| &live.bytes, last)? as u64;
        let mut cut = 0;
        for line in self.unstaged.split_inclusive('\n') {
            let cost = STAGE_PLANNED_PER_BYTE * line.len() as u64 + STAGE_PLANNED_PER_LINE;
            if planned + cost > allowance && !(first && cut == 0) {
                break;
            }
            planned += cost;
            cut += line.len();
        }
        if cut == 0 {
            return Ok(0);
        }
        let lines: String = self.unstaged.drain(..cut).collect();
        self.append(&lines)
    }

    /// Appends `text` at the end, as one edit at the last line's end (dist/live.js #stageEdits); the fuel it took.
    fn append(&mut self, text: &str) -> Result<u64, String> {
        let last = self.line_count()? - 1;
        let at = self.read(|live| &live.len, last)?;
        let bytes = text.as_bytes();
        let staged = bytes.len() as u64 + self.read(|live| &live.bytes, last)? as u64;
        let fuel = FUEL_BASE + STAGE_PER_BYTE * staged;
        let stage = self.live.stage;
        // The edit count, then the edit's start and end (line, UTF-16 column), its text's offset and length.
        let (ptr, used) = self.lexers.call(&stage, (28 + bytes.len()) as u32, fuel)?;
        let header: Vec<u8> = [1, last, at, last, at, 28, bytes.len() as u32]
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        self.lexers.write(ptr as usize, &header)?;
        self.lexers.write(ptr as usize + 28, bytes)?;
        let edit = self.live.edit;
        let ((), split) = self.lexers.call(&edit, (ptr, 0), fuel)?;
        Ok(used + split)
    }

    /// Lexes the lines an edit left waiting while they are planned to fit `budget`: whole lines, or a slice of a line
    /// too long or too big for what is left, and with `first` at least one step, so a call that starts here always
    /// gets on. The fuel spent, and whether the document is done.
    fn run(&mut self, budget: u64, mut first: bool) -> Result<(u64, bool), String> {
        let mut spent = 0;
        loop {
            if self.read(|live| &live.stats, WAITING)? == 0 {
                return Ok((spent, true));
            }
            let (next, count) = (
                self.read(|live| &live.stats, NEXT_LINE)?,
                self.line_count()?,
            );
            if next >= count {
                return Err("a document waits past its last line".into());
            }
            let (mut end, mut planned, mut handed) = (next, 0, 0);
            while end < count {
                let bytes = self.read(|live| &live.bytes, end)? as u64 + 1;
                let cost = PLANNED_PER_BYTE * bytes + PLANNED_PER_LINE;
                if bytes > LONG_LINE as u64 || spent + planned + cost > budget {
                    break;
                }
                (planned, handed, end) = (planned + cost, handed + bytes, end + 1);
            }
            if end == next && !first && spent + SLICE_PLANNED > budget {
                return Ok((spent, false));
            }
            first = false;
            let run = self.live.run;
            let used = if end > next {
                let fuel = FUEL_BASE + FUEL_PER_BYTE * handed + FUEL_PER_LINE * (end - next) as u64;
                self.lexers.call(&run, (end - next, end, 0), fuel)?.1
            } else {
                let fuel = FUEL_BASE + FUEL_PER_BYTE * WORK as u64;
                self.lexers.call(&run, (1, next + 1, WORK), fuel)?.1
            };
            spent += used.max(1);
        }
    }

    /// The records of lines `from` on, as mode 3 answers them, with offsets from the start of line `from`.
    fn records(&mut self, from: u32) -> Result<Vec<u32>, String> {
        let count = self.line_count()?;
        let (mut records, mut start) = (Vec::new(), 0);
        for line in from..count {
            let tokens = self.read(|live| &live.tokens, line)?;
            let flags = self.read(|live| &live.flags, line)?;
            if tokens > 0 {
                // dist/live.js getLineRecords: `packed24` words of the type and the end, or `wide32` pairs.
                let at = self.read(|live| &live.records, line)? as usize;
                for token in 0..tokens as usize {
                    let (end, kind) = if flags & 4 != 0 {
                        (
                            self.lexers.word(at + token * 8)?,
                            self.lexers.word(at + token * 8 + 4)?,
                        )
                    } else {
                        let word = self.lexers.word(at + token * 4)?;
                        (word & 0xFF_FFFF, word >> 24)
                    };
                    records.extend([start + end, kind]);
                }
            }
            if line + 1 < count {
                // The line's terminator: bits 0-1 are its length, one or two (CRLF).
                start += self.read(|live| &live.len, line)? + (flags & 3);
                records.extend([start, u32::MAX]);
            }
        }
        Ok(records)
    }
}

/// One request of `highlightCode` (highlight.contract HighlightRequest).
struct Request<'a> {
    id: f64,
    language: f64,
    doc: u32,
    from: usize,
    text: &'a str,
}

/// What a request answers: its records from a line on, or a word for the module (highlight.contract
/// HighlightRecords `line`).
#[derive(Debug, PartialEq)]
enum Outcome {
    Records(u32, Vec<u32>),
    /// Its document has lines left to lex: the next call goes on.
    More,
    /// Its document is not here, or not at `from`: it starts again from its first line.
    Again,
    /// The lexers trapped: the text paints plain.
    Failed,
}

impl Outcome {
    fn line(&self) -> f64 {
        match self {
            Outcome::Records(line, _) => *line as f64,
            Outcome::More => -1.0,
            Outcome::Again => -2.0,
            Outcome::Failed => -3.0,
        }
    }
}

/// This thread's lexers: the compiled module, the instance whole texts share, and the documents.
struct Here {
    engine: Engine,
    module: Module,
    shared: Option<Lexers>,
    documents: Vec<Document>,
    uses: u64,
}

thread_local! {
    static HERE: RefCell<Option<Here>> = const { RefCell::new(None) };
}

fn lexer(language: f64) -> Option<u8> {
    (1.0..=255.0).contains(&language).then_some(language as u8)
}

impl Here {
    fn new() -> Result<Here, String> {
        let mut config = Config::default();
        // The lexers use the SIMD proposal (wasmi's `simd` feature, Cargo.toml) and none of relaxed SIMD.
        config
            .consume_fuel(true)
            .wasm_simd(true)
            .wasm_relaxed_simd(false);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, LEXERS).map_err(failed)?;
        Ok(Here {
            engine,
            module,
            shared: None,
            documents: Vec::new(),
            uses: 0,
        })
    }

    /// Each request's outcome, `None` for one past the budget, and the fuel spent. The requests come in order; a
    /// document takes at most half the budget at first, staging included, so one long text leaves the other half to
    /// the requests after it, and what is left goes to the documents still lexing.
    fn answer(&mut self, requests: &[Request<'_>]) -> (Vec<Option<Outcome>>, u64) {
        let (budget, mut spent) = (budget(), 0);
        let mut outcomes: Vec<Option<Outcome>> = Vec::with_capacity(requests.len());
        for request in requests {
            outcomes.push(if request.doc == 0 {
                self.whole(request, budget, &mut spent)
            } else {
                let share = (budget / 2).min(budget.saturating_sub(spent));
                Some(self.document(request, share, &mut spent))
            });
        }
        for (request, outcome) in requests.iter().zip(outcomes.iter_mut()) {
            if spent < budget && *outcome == Some(Outcome::More) {
                let at = self
                    .documents
                    .iter()
                    .position(|document| document.id == request.doc);
                *outcome = Some(match at {
                    Some(at) => self.lex(at, budget - spent, &mut spent, false),
                    None => Outcome::Again,
                });
            }
        }
        (outcomes, spent)
    }

    fn whole(&mut self, request: &Request<'_>, budget: u64, spent: &mut u64) -> Option<Outcome> {
        let Some(language) = lexer(request.language) else {
            return Some(Outcome::Records(0, Vec::new()));
        };
        if request.text.len() > WHOLE_BYTES {
            return Some(Outcome::Again);
        }
        let planned = PLANNED_PER_BYTE * request.text.len() as u64
            + PLANNED_PER_LINE * lines_of(request.text);
        if *spent + planned > budget {
            // Left for the next call, or, past what a call of its own may plan, lexed as a document.
            return if *spent > 0 {
                None
            } else {
                Some(Outcome::Again)
            };
        }
        if self
            .shared
            .as_ref()
            .is_some_and(|shared| shared.pages() > RECYCLE_PAGES && request.text.len() <= PAGE)
        {
            self.shared = None;
        }
        if self.shared.is_none() {
            self.shared = Lexers::new(&self.engine, &self.module).ok();
        }
        let result = match self.shared.as_mut() {
            Some(shared) => shared.whole(language, request.text),
            None => Err("the lexers could not be made".into()),
        };
        match result {
            Ok((records, used)) => {
                *spent += used;
                Some(Outcome::Records(0, records))
            }
            // A trap leaves the instance's state unknown: the next text gets a new one.
            Err(_) => {
                self.shared = None;
                *spent += planned;
                Some(Outcome::Failed)
            }
        }
    }

    /// A document's request: start it (`from` 0) or append to it, then stage and lex it within `share`.
    fn document(&mut self, request: &Request<'_>, share: u64, spent: &mut u64) -> Outcome {
        self.uses += 1;
        let start = *spent;
        let at = if request.from == 0 {
            self.documents.retain(|document| document.id != request.doc);
            let Some(language) = lexer(request.language) else {
                return Outcome::Records(0, Vec::new());
            };
            if self.documents.len() >= DOCUMENTS {
                let oldest = (0..self.documents.len()).min_by_key(|&at| self.documents[at].used);
                self.documents.remove(oldest.unwrap_or(0));
            }
            match Document::new(
                &self.engine,
                &self.module,
                request.doc,
                language,
                request.text,
            ) {
                Ok((document, used)) => {
                    *spent += used;
                    self.documents.push(document);
                    self.documents.len() - 1
                }
                Err(_) => return Outcome::Failed,
            }
        } else {
            let at = self.documents.iter().position(|document| {
                document.id == request.doc
                    && document.length == request.from
                    && lexer(request.language) == Some(document.language)
            });
            let Some(at) = at else {
                return Outcome::Again;
            };
            let document = &mut self.documents[at];
            document.unstaged.push_str(request.text);
            document.length += request.text.encode_utf16().count();
            at
        };
        self.documents[at].used = self.uses;
        let share = share.saturating_sub(*spent - start);
        self.lex(at, share, spent, start == 0)
    }

    /// Stages and lexes document `at` within `share`, with `first` at least one step of each when nothing else of the
    /// call has run: its records from the first line not yet final when it is done.
    fn lex(&mut self, at: usize, share: u64, spent: &mut u64, first: bool) -> Outcome {
        let document = &mut self.documents[at];
        let result = document.stage(share, first).and_then(|staged| {
            *spent += staged;
            let (used, done) = document.run(share.saturating_sub(staged), first && staged == 0)?;
            *spent += used;
            if !done || !document.unstaged.is_empty() {
                return Ok(None);
            }
            let from = document.delivered;
            let records = document.records(from)?;
            document.delivered = document.line_count()? - 1;
            Ok(Some((from, records)))
        });
        match result {
            Ok(None) => Outcome::More,
            Ok(Some((from, records))) => {
                if document.lexers.pages() > KEEP_PAGES {
                    self.documents.remove(at);
                }
                Outcome::Records(from, records)
            }
            Err(_) => {
                self.documents.remove(at);
                Outcome::Failed
            }
        }
    }
}

fn request(value: &Value) -> Option<Request<'_>> {
    let Value::Record(fields) = value else {
        return None;
    };
    let number = |at: usize| fields.get(at).and_then(Value::as_number).unwrap_or(0.0);
    Some(Request {
        id: number(0),
        language: number(1),
        doc: number(2).max(0.0) as u32,
        from: number(3).max(0.0) as usize,
        text: fields.get(4).and_then(Value::as_str).unwrap_or(""),
    })
}

/// `highlightCode(requests)`: each request (`id`, `language`, `doc`, `from`, `text`) answered as its id, a line and
/// records, in order, within the call's [`BUDGET`]. A whole text past the budget is left out, and the TypeScript module
/// asks again; a document always answers, `More` while it has lines left. A text whose lexer is unknown answers no
/// records (plain).
pub fn highlight_code(args: &[Value]) -> Value {
    let requests: Vec<Request<'_>> = match args.first() {
        Some(Value::List(items)) => items.iter().filter_map(request).collect(),
        _ => Vec::new(),
    };
    let outcomes = HERE.with(|cell| {
        let mut here = cell.borrow_mut();
        if here.is_none() {
            *here = Here::new().ok();
        }
        match here.as_mut() {
            Some(here) => here.answer(&requests).0,
            None => requests.iter().map(|_| Some(Outcome::Failed)).collect(),
        }
    });
    let results = requests
        .iter()
        .zip(outcomes)
        .filter_map(|(request, outcome)| {
            let outcome = outcome?;
            let records = match &outcome {
                Outcome::Records(_, records) => records
                    .iter()
                    .map(|&record| Value::Number(record as f64))
                    .collect(),
                _ => Vec::new(),
            };
            Some(Value::record(vec![
                Value::Number(request.id),
                Value::Number(outcome.line()),
                Value::list(records),
            ]))
        })
        .collect();
    Value::record(vec![Value::list(results)])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TypeScript's lexer id (dist/languages.js `ts`).
    const TS: u8 = 72;
    /// The budget these tests give a call: about 2.5 KB of TypeScript.
    const SMALL: u64 = 1_000_000;

    fn here<R>(f: impl FnOnce(&mut Here) -> R) -> R {
        HERE.with(|cell| {
            let mut here = cell.borrow_mut();
            f(here.get_or_insert_with(|| Here::new().unwrap()))
        })
    }

    /// Sets this thread's call budget and fuel cap.
    fn testing(budget: u64, fuel_cap: u64) {
        TESTING.with(|cell| cell.set((budget, fuel_cap)));
    }

    /// Whole TypeScript texts in one call.
    fn ask(texts: &[&str]) -> (Vec<Option<Outcome>>, u64) {
        let requests: Vec<Request<'_>> = texts
            .iter()
            .enumerate()
            .map(|(id, text)| Request {
                id: id as f64,
                language: TS as f64,
                doc: 0,
                from: 0,
                text,
            })
            .collect();
        here(|here| here.answer(&requests))
    }

    /// One document request: `text` at `from` of document `doc`.
    fn edit(doc: u32, from: usize, text: &str) -> (Outcome, u64) {
        let request = Request {
            id: 1.0,
            language: TS as f64,
            doc,
            from,
            text,
        };
        let (mut outcomes, spent) = here(|here| here.answer(&[request]));
        (
            outcomes.remove(0).expect("a document always answers"),
            spent,
        )
    }

    /// The whole text's records and their fuel, on lexers of their own.
    fn whole(text: &str) -> (Vec<u32>, u64) {
        here(|here| {
            let mut lexers = Lexers::new(&here.engine, &here.module).unwrap();
            lexers.whole(TS, text).unwrap()
        })
    }

    /// What highlights.ts highlightStore paints from a document's answers, in records over the whole text: the lines
    /// already final, then the answer's records moved to where its first line starts. Every line but the last is final
    /// after each answer.
    #[derive(Default)]
    struct Painted {
        lines: u32,
        final_chars: u32,
        final_records: Vec<u32>,
    }

    impl Painted {
        fn answer(&mut self, line: u32, records: &[u32]) -> Vec<u32> {
            assert_eq!(line, self.lines, "the answer's first line");
            let moved: Vec<u32> = records
                .chunks_exact(2)
                .flat_map(|pair| [pair[0] + self.final_chars, pair[1]])
                .collect();
            let shown = [self.final_records.as_slice(), &moved].concat();
            if let Some(last) = records
                .chunks_exact(2)
                .rposition(|pair| pair[1] == u32::MAX)
            {
                self.final_records
                    .extend_from_slice(&moved[..(last + 1) * 2]);
                self.final_chars += records[last * 2];
                self.lines += records
                    .chunks_exact(2)
                    .filter(|pair| pair[1] == u32::MAX)
                    .count() as u32;
            }
            shown
        }
    }

    /// Streams `versions`, each the last and more, into document `doc` as highlights.ts sends them (the first from 0,
    /// the rest appended) and returns the first whose painted records are not the whole text's, with both.
    fn stream(doc: u32, language: u8, versions: &[String]) -> Option<String> {
        let (mut painted, mut held) = (Painted::default(), String::new());
        for (n, version) in versions.iter().enumerate() {
            let at = held.encode_utf16().count();
            let request = |from: usize, text: &str| {
                let request = Request {
                    id: 1.0,
                    language: language as f64,
                    doc,
                    from,
                    text,
                };
                here(|here| here.answer(&[request])).0.remove(0).unwrap()
            };
            let mut outcome = request(at, &version[held.len()..]);
            let length = version.encode_utf16().count();
            while outcome == Outcome::More {
                outcome = request(length, "");
            }
            let Outcome::Records(line, records) = outcome else {
                return Some(format!("version {n}: {outcome:?}"));
            };
            let (shown, expected) = (
                painted.answer(line, &records),
                records_of(language, version),
            );
            if shown != expected {
                return Some(format!(
                    "version {n} {version:?}: painted {shown:?}, whole {expected:?}"
                ));
            }
            held = version.clone();
        }
        None
    }

    /// The whole text's records under `language`, on lexers of their own.
    fn records_of(language: u8, text: &str) -> Vec<u32> {
        here(|here| {
            let mut lexers = Lexers::new(&here.engine, &here.module).unwrap();
            lexers.whole(language, text).unwrap().0
        })
    }

    /// `code` cut after every `step` characters: each version the last and more.
    fn prefixes(code: &str, step: usize) -> Vec<String> {
        let chars: Vec<char> = code.chars().collect();
        (1..=chars.len().div_ceil(step))
            .map(|n| chars[..(n * step).min(chars.len())].iter().collect())
            .collect()
    }

    fn typescript(lines: usize) -> String {
        (0..lines)
            .map(|i| format!("export function f{i}(a: number, b = \"x{i}\") {{ return a * {i} + b.length; }} // {i}\n"))
            .collect()
    }

    #[test]
    fn a_typescript_line_has_the_records_the_package_s_javascript_reads() {
        // What dist/highlighter.js answers in mode 3 for this text (its own WebAssembly run, bun 1.4.2).
        let text = "const x: number = 1; // hi\nfunction f(a) { return `s${a}`; }";
        let expected = [
            6,
            19,
            7,
            58,
            9,
            40,
            16,
            56,
            18,
            25,
            19,
            24,
            21,
            35,
            26,
            4,
            27,
            u32::MAX,
            36,
            19,
            37,
            14,
            38,
            32,
            39,
            59,
            43,
            32,
            50,
            18,
            52,
            44,
            54,
            40,
            55,
            58,
            56,
            40,
            57,
            44,
            59,
            35,
            60,
            32,
        ];
        assert_eq!(
            ask(&[text]).0,
            [Some(Outcome::Records(0, expected.to_vec()))]
        );
    }

    #[test]
    fn offsets_count_utf16_units_and_identifiers_outside_ascii_ask_the_unicode_imports() {
        // `é` is one UTF-16 unit and two UTF-8 bytes; `𝑥` is two units and four bytes. A token keeps the spaces after
        // it, and both identifiers are variables (type 58), which the lexer knows from the ID_Start import.
        let text = "let café = 1; let 𝑥 = 2;";
        let records = whole(text).0;
        assert_eq!(records[..4], [4, 19, 9, 58]);
        assert_eq!(records[10..14], [18, 19, 21, 58]);
        assert_eq!(
            records[records.len() - 2],
            text.encode_utf16().count() as u32
        );
    }

    #[test]
    fn a_call_lexes_whole_texts_within_its_budget_and_leaves_the_rest_for_the_next() {
        testing(SMALL, u64::MAX);
        let plain = Request {
            id: 9.0,
            language: 0.0,
            doc: 0,
            from: 0,
            text: "plain",
        };
        assert_eq!(
            here(|here| here.answer(&[plain])).0,
            [Some(Outcome::Records(0, Vec::new()))]
        );
        // Long texts fit while the budget lasts; the rest wait, and a short one after them still fits.
        let long = typescript(10);
        assert!(whole(&long).1 * 2 < SMALL && whole(&long).1 * 6 > SMALL);
        let (outcomes, spent) = ask(&[
            "let a = 1;",
            &long,
            &long,
            &long,
            &long,
            &long,
            &long,
            "let b = 2;",
        ]);
        let answered: Vec<bool> = outcomes.iter().map(Option::is_some).collect();
        let fit = answered[1..7]
            .iter()
            .take_while(|&&answered| answered)
            .count();
        assert!((2..6).contains(&fit), "{answered:?}");
        assert_eq!(answered[1 + fit..7], vec![false; 6 - fit][..]);
        assert!(answered[0] && answered[7]);
        assert!(spent <= SMALL, "{spent} fuel");
        // A text past WHOLE_BYTES is a document, and so is one planned past what a call of its own may spend.
        assert_eq!(
            ask(&[&"x".repeat(WHOLE_BYTES + 1)]).0,
            [Some(Outcome::Again)]
        );
        let past = typescript(40);
        assert!(past.len() as u64 * PLANNED_PER_BYTE > SMALL);
        assert_eq!(ask(&[&past]).0, [Some(Outcome::Again)]);
    }

    #[test]
    fn a_long_text_is_a_document_lexed_a_budget_at_a_time_to_the_records_of_the_whole() {
        testing(SMALL, u64::MAX);
        let text = typescript(100);
        let (records, fuel) = whole(&text);
        assert!(
            fuel > 3 * SMALL,
            "{fuel} fuel: the text should take several calls"
        );
        let (mut outcome, mut spent) = edit(1, 0, &text);
        let mut calls = 1;
        while outcome == Outcome::More {
            assert!(spent <= SMALL + SMALL / 4, "call {calls}: {spent} fuel");
            (outcome, spent) = edit(1, text.len(), "");
            calls += 1;
        }
        assert!(calls > 3, "{calls} calls");
        assert_eq!(outcome, Outcome::Records(0, records));
    }

    #[test]
    fn a_long_line_is_lexed_in_slices() {
        testing(SMALL, u64::MAX);
        let line = format!(
            "const values = [{}];",
            (0..7_000)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert!(line.len() > 2 * LONG_LINE);
        let (mut outcome, mut calls) = (edit(2, 0, &line).0, 1);
        while outcome == Outcome::More {
            outcome = edit(2, line.len(), "").0;
            calls += 1;
        }
        assert!(calls > 1, "{calls} calls");
        assert_eq!(outcome, Outcome::Records(0, whole(&line).0));
    }

    #[test]
    fn an_append_to_a_long_line_has_the_fuel_to_stage_that_line_again() {
        let line = format!(
            "const values = [{}",
            (0..20_000)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let (mut outcome, mut calls) = (edit(6, 0, &line).0, 1);
        while outcome == Outcome::More {
            outcome = edit(6, line.len(), "").0;
            calls += 1;
        }
        assert!(
            matches!(outcome, Outcome::Records(0, _)),
            "{outcome:?} after {calls} calls"
        );
        let mut outcome = edit(6, line.len(), ", 1]").0;
        while outcome == Outcome::More {
            outcome = edit(6, line.len() + 4, "").0;
        }
        assert_eq!(
            outcome,
            Outcome::Records(0, whole(&format!("{line}, 1]")).0)
        );
    }

    #[test]
    fn a_document_answers_what_the_whole_text_does() {
        let texts = [
            "a = 1\r\nb = 2\r\n",
            "x\n\n\ny",
            "let café = 1;\nlet 𝑥 = `\n${é}`;",
            "/* open\nstill\n*/ let a",
        ];
        for (doc, text) in texts.into_iter().enumerate() {
            assert_eq!(
                edit(10 + doc as u32, 0, text).0,
                Outcome::Records(0, whole(text).0),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_streaming_block_lexes_only_the_line_it_continues_and_its_new_lines() {
        let text = typescript(60);
        let (mut painted, mut spent, mut at) = (Painted::default(), 0, 0);
        // A reply's code block, 37 bytes at a time: each version is the last one and more.
        while at < text.len() {
            let end = (at + 37).min(text.len());
            let (outcome, used) = edit(3, at, &text[at..end]);
            let Outcome::Records(line, records) = outcome else {
                panic!("{outcome:?}");
            };
            assert_eq!(
                painted.answer(line, &records),
                whole(&text[..end]).0,
                "after {end} bytes"
            );
            (spent, at) = (spent + used, end);
        }
        // Lexed again whole, each of the 130 versions would cost half the text on average: 65 texts' worth.
        let fuel = whole(&text).1;
        assert!(
            spent < 4 * fuel,
            "{spent} fuel streamed, {fuel} for the whole text"
        );
    }

    #[test]
    fn every_prefix_of_upstream_s_samples_paints_what_the_whole_text_does() {
        // incrementalHighlighting.test.ts's eleven samples at the pin, and a setext heading of the clone's own, each
        // streamed a character at a time and in steps of 3 and 7 (dist/languages.js ids). One with a CR is never
        // appended to (highlights.ts starts its document again).
        let samples: [(&str, u8, &str); 12] = [
            (
                "typescript",
                72,
                "/* multi\nline comment */\nconst x = `template\n${1 + 2}`;\nconst re = /abc/;\n",
            ),
            (
                "python",
                59,
                "#!/usr/bin/python\nx = \"\"\"multi\nline\"\"\"\nprint(x)\n",
            ),
            ("bash", 4, "#!/bin/bash\ncat <<EOF\nhello\nEOF\necho hi\n"),
            (
                "html",
                33,
                "<script>\nconst x = 1;\n</script>\n<style>\np {color:red;}\n</style>\n",
            ),
            (
                "markdown",
                47,
                "# heading\n\n```ts\nconst a = 1;\n```\n\ntext\n",
            ),
            ("rust", 62, "fn main() {\n let x = r#\"multi\nline\"#;\n}\n"),
            (
                "tsx",
                74,
                "const element = <div\n className=\"x\">\n{value}\n</div>;\n",
            ),
            ("json", 36, "{\n \"value\": [1,\n 2, 3]\n}\n"),
            ("yaml", 79, "key: |\n  multiline\n  value\nnext: true\n"),
            (
                "css",
                12,
                "/* comment\n continued */\np::before {\n content: \"text\";\n}\n",
            ),
            ("sql", 67, "SELECT 'multi\nline'\nFROM table_name;\n"),
            (
                "markdown setext",
                47,
                "Title\n=====\n\nSub\n---\n\n- item\n  more\n",
            ),
        ];
        let mut doc = 200;
        for (name, language, code) in samples {
            for step in [1, 3, 7] {
                doc += 1;
                assert_eq!(
                    stream(doc, language, &prefixes(code, step)),
                    None,
                    "{name}, {step} at a time"
                );
            }
        }
    }

    #[test]
    fn a_long_document_leaves_half_a_call_to_the_requests_after_it() {
        testing(SMALL, u64::MAX);
        let (long, block) = (typescript(100), typescript(3));
        let request = |id: f64, doc: u32, text| Request {
            id,
            language: TS as f64,
            doc,
            from: 0,
            text,
        };
        let (outcomes, spent) =
            here(|here| here.answer(&[request(1.0, 30, &long), request(2.0, 31, &block)]));
        assert_eq!(outcomes[0], Some(Outcome::More));
        assert_eq!(outcomes[1], Some(Outcome::Records(0, whole(&block).0)));
        assert!(spent <= SMALL + SMALL / 4, "{spent} fuel");
    }

    #[test]
    fn staging_a_long_text_is_held_to_the_budget_too() {
        testing(SMALL, u64::MAX);
        // Staged in one call, its 30,000 lines would take about six budgets.
        let text = "\n".repeat(30_000);
        let (mut outcome, mut spent) = edit(32, 0, &text);
        let mut calls = 1;
        while outcome == Outcome::More {
            assert!(spent <= SMALL + SMALL / 4, "call {calls}: {spent} fuel");
            (outcome, spent) = edit(32, text.len(), "");
            calls += 1;
        }
        assert!(calls > 5, "{calls} calls");
        assert_eq!(outcome, Outcome::Records(0, whole(&text).0));
    }

    #[test]
    fn a_line_too_big_for_what_is_left_of_a_call_is_lexed_a_slice_at_a_time() {
        let budget = 2 * SMALL;
        testing(budget, u64::MAX);
        let line = format!(
            "const values = [{}];",
            (0..2_500)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        // Shorter than LONG_LINE, and planned at twice the budget: lexed whole, it would take one call past it.
        assert!(line.len() < LONG_LINE && line.len() as u64 * PLANNED_PER_BYTE > 2 * budget);
        let (mut outcome, mut spent) = edit(33, 0, &line);
        let mut calls = 1;
        while outcome == Outcome::More {
            assert!(
                spent <= budget + SLICE_PLANNED,
                "call {calls}: {spent} fuel"
            );
            (outcome, spent) = edit(33, line.len(), "");
            calls += 1;
        }
        assert!(calls > 2, "{calls} calls");
        assert_eq!(outcome, Outcome::Records(0, whole(&line).0));
    }

    #[test]
    fn a_document_out_of_step_or_not_here_starts_again() {
        assert_eq!(edit(4, 4, "x").0, Outcome::Again);
        assert!(matches!(edit(4, 0, "let a").0, Outcome::Records(0, _)));
        assert_eq!(edit(4, 4, " = 1").0, Outcome::Again);
        assert!(matches!(edit(4, 5, " = 1").0, Outcome::Records(0, _)));
        // The least recently used document goes past DOCUMENTS.
        for doc in 5..5 + DOCUMENTS as u32 {
            edit(doc, 0, "let b");
        }
        assert_eq!(edit(4, 9, ";").0, Outcome::Again);
    }

    #[test]
    fn a_trap_drops_its_lexers_and_the_next_text_gets_new_ones() {
        testing(BUDGET, 1_000);
        assert_eq!(ask(&["let a = 1;"]).0, [Some(Outcome::Failed)]);
        assert!(here(|here| here.shared.is_none()));
        assert_eq!(edit(20, 0, &typescript(3)).0, Outcome::Failed);
        assert!(here(|here| here.documents.is_empty()));
        testing(BUDGET, u64::MAX);
        assert_eq!(
            ask(&["let a = 1;"]).0,
            [Some(Outcome::Records(0, whole("let a = 1;").0))]
        );
        assert!(matches!(
            edit(20, 0, &typescript(3)).0,
            Outcome::Records(0, _)
        ));
    }

    #[test]
    fn a_store_s_memory_is_bounded_and_a_grown_shared_instance_is_made_again() {
        here(|here| {
            let mut lexers = Lexers::new(&here.engine, &here.module).unwrap();
            assert!(lexers
                .memory
                .grow(&mut lexers.store, (MEMORY_BYTES / PAGE) as u64)
                .is_err());
        });
        ask(&["let a = 1;"]);
        here(|here| {
            let shared = here.shared.as_mut().unwrap();
            let grow = RECYCLE_PAGES + 1 - shared.pages();
            shared.memory.grow(&mut shared.store, grow as u64).unwrap();
        });
        ask(&["let b = 2;"]);
        assert!(here(|here| here.shared.as_ref().unwrap().pages()) < RECYCLE_PAGES);
    }

    #[test]
    fn the_answer_has_each_request_s_id_line_and_records() {
        let request = |id: f64, doc: f64, text: &str| {
            Value::record(vec![
                Value::Number(id),
                Value::Number(TS as f64),
                Value::Number(doc),
                Value::Number(0.0),
                Value::str(text),
            ])
        };
        let answer = highlight_code(&[Value::list(vec![
            request(7.0, 0.0, "a"),
            request(8.0, 30.0, "b"),
        ])]);
        let Value::Record(fields) = answer else {
            panic!("a record")
        };
        let Some(Value::List(results)) = fields.first() else {
            panic!("a list")
        };
        let heads: Vec<(f64, f64)> = results
            .iter()
            .map(|result| match result {
                Value::Record(fields) => (
                    fields[0].as_number().unwrap(),
                    fields[1].as_number().unwrap(),
                ),
                _ => panic!("a record"),
            })
            .collect();
        assert_eq!(heads, [(7.0, 0.0), (8.0, 0.0)]);
    }
}
