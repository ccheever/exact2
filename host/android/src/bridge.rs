//! One selected Android runtime owner, exposing the existing borrowed C ABI.

use crate::{core, wire::Encoder};
#[path = "row_reuse.rs"]
mod row_reuse;
use exact_apple::{
    abi::Hooks,
    measure::{CallbackMeasurer, FontsFn},
};
use exact_kernel::{Env, MonospaceMeasurer, TextMeasurer};
use exact_plan::Plan;
use exact_runner::{DataSource, Event};
use std::ffi::c_void;

type General<D> = exact_apple::abi::Bridge<D>;

enum Owner<D: DataSource> {
    Empty,
    Core(Box<core::Core<D>>),
    General(Box<General<D>>),
}

/// Android chooses its receipt-driven core adapter once at boot. The general
/// owner remains available for plans needing the existing executor; a session
/// never keeps two runner/kernel trees.
pub struct Bridge<D: DataSource> {
    owner: Owner<D>,
    fonts: Option<(FontsFn, *mut c_void)>,
    compat: Option<&'static str>,
    input: Vec<u8>,
    output: Vec<u8>,
    binary: bool,
    row_reuse: bool,
    row_reuse_admitted: bool,
}

impl<D: DataSource> Bridge<D> {
    /// Empty owner, before its static baked plan chooses a runtime.
    pub const fn new() -> Self {
        Self {
            owner: Owner::Empty,
            fonts: None,
            compat: None,
            input: Vec::new(),
            output: Vec::new(),
            binary: false,
            row_reuse: false,
            row_reuse_admitted: false,
        }
    }
    /// This publication is already EXA1, without an intermediate native JSON batch.
    pub fn binary_output(&self) -> bool {
        self.binary
    }
    /// Record this carrier's native compatibility receipt before boot.
    pub fn set_compat(&mut self, compat: &'static str) {
        self.compat = Some(compat);
    }
    /// Opt into renewing compatible carriers. Rows containing a ScrollView or
    /// native navigation retain fresh mounts because their state cannot be reset.
    /// The presenter must implement `renew` before enabling this default-off policy.
    pub fn set_row_reuse(&mut self, on: bool) {
        self.row_reuse = on;
        if let Owner::General(b) = &mut self.owner {
            b.set_row_reuse(on && self.row_reuse_admitted);
        }
    }
    /// Install the declared fonts callback before any paragraph measurement.
    pub fn set_fonts(&mut self, fonts: Option<FontsFn>, ctx: *mut c_void) {
        self.fonts = fonts.map(|f| (f, ctx));
        if let Owner::General(b) = &mut self.owner {
            b.set_fonts(fonts, ctx);
        }
    }
    /// Resize one initialized input buffer; the caller fills it synchronously.
    pub fn input(&mut self, len: usize) -> *mut u8 {
        if let Owner::General(b) = &mut self.owner {
            return b.input(len);
        }
        self.input.clear();
        self.input.resize(len, 0);
        self.input.as_mut_ptr()
    }
    /// Copy an agent/event input into the same owner-thread buffer.
    pub fn input_write(&mut self, bytes: &[u8]) -> usize {
        if let Owner::General(b) = &mut self.owner {
            return b.input_write(bytes);
        }
        self.input.clear();
        self.input.extend_from_slice(bytes);
        bytes.len()
    }
    /// Borrow the complete or bounded most recent publication.
    pub fn output_bytes(&self, length: usize) -> &[u8] {
        if !self.binary {
            if let Owner::General(b) = &self.owner {
                return b.output_bytes(length);
            }
        }
        &self.output[..length.min(self.output.len())]
    }
    /// Boot an owned byte slice through the same validation and kernel rules.
    pub fn boot(&mut self, bytes: &[u8], data: D, hooks: Hooks, w: f32, h: f32) -> u32 {
        self.boot_plan(Plan::decode(bytes), data, hooks, (w, h), None)
    }
    /// Decode a linked static plan without copying its immutable resource pool.
    pub fn boot_selected(
        &mut self,
        bytes: &'static [u8],
        make: impl FnOnce() -> D,
        hooks: Hooks,
        w: f32,
        h: f32,
    ) -> u32 {
        self.boot_plan(Plan::decode_static(bytes), make(), hooks, (w, h), None)
    }
    /// Apply one authored press before the initial layout and publication.
    /// The input names its unique test id as UTF-8; effects are refused before boot.
    pub fn boot_selected_initial(
        &mut self,
        bytes: &'static [u8],
        make: impl FnOnce() -> D,
        hooks: Hooks,
        w: f32,
        h: f32,
        len: usize,
    ) -> u32 {
        let target = match self
            .input
            .get(..len)
            .and_then(|b| std::str::from_utf8(b).ok())
        {
            Some(s) if !s.is_empty() => s.to_owned(),
            _ => return self.refuse("initial press must be a nonempty UTF-8 target"),
        };
        self.boot_plan(
            Plan::decode_static(bytes),
            make(),
            hooks,
            (w, h),
            Some(&target),
        )
    }
    fn boot_plan(
        &mut self,
        plan: Result<Plan, exact_plan::PlanError>,
        data: D,
        hooks: Hooks,
        size: (f32, f32),
        initial_press: Option<&str>,
    ) -> u32 {
        let (w, h) = size;
        if let Some(compat) = self.compat {
            if let Err(reason) = exact_runner::delivery::refuse_analysis(compat) {
                return self.refuse(reason);
            }
        }
        let plan = match plan {
            Ok(plan) => plan,
            Err(e) => return self.refuse(&format!("plan: {e:?}")),
        };
        self.row_reuse_admitted = row_reuse::admits(&plan);
        if !core::eligible(&plan, &data) {
            if initial_press.is_some() {
                return self.refuse("initial press requires an effect-free Android core plan");
            }
            let mut b = General::new();
            if let Some(compat) = self.compat {
                b.set_compat(compat);
            }
            if let Some((fonts, ctx)) = self.fonts {
                b.set_fonts(Some(fonts), ctx);
            }
            b.set_row_reuse(self.row_reuse && self.row_reuse_admitted);
            let len = b.boot_decoded(plan, data, hooks, w, h);
            self.owner = Owner::General(Box::new(b));
            self.binary = false;
            return len;
        }
        if let Some((fonts, ctx)) = self.fonts {
            exact_apple::measure::install_fonts(&plan, fonts, ctx);
        }
        let measurer: Box<dyn TextMeasurer> = match hooks.measure {
            Some(measure) => Box::new(CallbackMeasurer::new(measure, hooks.ctx, None)),
            None => Box::new(MonospaceMeasurer::default()),
        };
        match core::Core::boot(plan, data, measurer, w, h, initial_press) {
            Ok((owner, out)) => {
                self.owner = Owner::Core(Box::new(owner));
                self.emit(out)
            }
            Err(error) => self.refuse(&error),
        }
    }
    fn emit(&mut self, output: Vec<u8>) -> u32 {
        // The JVM retains a direct ByteBuffer for this address. Preserve the
        // backing between turns, including when the publication size shrinks.
        if self.output.capacity() >= output.len() {
            self.output.clear();
            self.output.extend_from_slice(&output);
        } else {
            self.output = output;
        }
        self.binary = true;
        self.output.len() as u32
    }
    fn refuse(&mut self, message: &str) -> u32 {
        let json = exact_apple::batch::Batch::new().finish(None, false, 0., Some(message));
        let mut output = Vec::new();
        Encoder::default()
            .encode(json.as_bytes(), &mut output)
            .expect("valid refusal");
        self.emit(output)
    }
    /// Dispatch the platform event vocabulary through the selected owner.
    pub fn dispatch(&mut self, view: u32, kind: u32, len: usize, now: f64) -> u32 {
        if let Owner::General(b) = &mut self.owner {
            self.binary = false;
            return b.dispatch(view, kind, len, now);
        }
        let Some(bytes) = self.input.get(..len) else {
            return self.refuse("event: truncated input");
        };
        let payload = String::from_utf8_lossy(bytes).into_owned();
        let event = match kind {
            0 => Event::Press,
            1 => Event::Change(payload.into()),
            23 => Event::Input(payload.into()),
            2 => Event::Hover(true),
            3 => Event::Hover(false),
            4 => Event::Focus,
            5 => Event::Blur,
            7 => Event::Submit,
            8 => Event::Load,
            9 => Event::Message(payload),
            10 => Event::Contextmenu,
            11 => Event::Dblclick,
            12 => Event::Swiperight,
            22 => Event::Refresh,
            6 | 13 | 19 | 20 | 21 | 28 => match Event::of_host_kind(kind, &payload) {
                Ok(event) => event,
                Err(error) => return self.refuse(error),
            },
            _ => return self.refuse("event kind is outside Android core"),
        };
        let output = match &mut self.owner {
            Owner::Core(c) => c.dispatch(view, event, now),
            _ => return self.refuse("not booted"),
        };
        self.emit(output)
    }
    /// Resize through the CSS layout owner.
    pub fn resize(&mut self, w: f32, h: f32) -> u32 {
        match &mut self.owner {
            Owner::General(b) => {
                self.binary = false;
                b.resize(w, h)
            }
            Owner::Core(c) => {
                let out = c.resize(w, h);
                self.emit(out)
            }
            Owner::Empty => self.refuse("not booted"),
        }
    }
    /// Drive timers and frame tasks once for a display frame.
    pub fn frame(&mut self, now: f64) -> u32 {
        match &mut self.owner {
            Owner::General(b) => {
                self.binary = false;
                b.frame(now)
            }
            Owner::Core(c) => {
                let out = c.advance(now, true);
                self.emit(out)
            }
            Owner::Empty => self.refuse("not booted"),
        }
    }
    /// Seek application timers on the existing controllable clock.
    pub fn advance(&mut self, now: f64, until: bool) -> u32 {
        match &mut self.owner {
            Owner::General(b) => {
                self.binary = false;
                b.advance(now, u32::from(until))
            }
            Owner::Core(c) => {
                let out = c.advance(now, false);
                self.emit(out)
            }
            Owner::Empty => self.refuse("not booted"),
        }
    }
    /// Core plans have no animated transition; sampling leaves their tree still.
    pub fn tick(&mut self, now: f64) -> u32 {
        match &mut self.owner {
            Owner::General(b) => {
                self.binary = false;
                b.tick(now)
            }
            Owner::Core(c) => {
                let out = c.quiet();
                self.emit(out)
            }
            Owner::Empty => self.refuse("not booted"),
        }
    }
    /// Drain native executor work; no executor exists for a selected core plan.
    pub fn pump(&mut self, now: f64) -> u32 {
        if let Owner::General(b) = &mut self.owner {
            self.binary = false;
            return b.pump(now);
        }
        self.tick(now)
    }
    /// Activate deferred data after the first drawn pixel.
    pub fn data_ready(&mut self) -> u32 {
        match &mut self.owner {
            Owner::General(b) => {
                self.binary = false;
                b.data_ready()
            }
            Owner::Core(c) => {
                let out = c.painted();
                self.emit(out)
            }
            Owner::Empty => self.refuse("not booted"),
        }
    }
    /// Re-answer the platform display preferences.
    pub fn set_preferences(&mut self, bits: u32) -> u32 {
        match &mut self.owner {
            Owner::General(b) => {
                self.binary = false;
                b.set_preferences(bits)
            }
            Owner::Core(c) => {
                let out = c.preferences(bits);
                self.emit(out)
            }
            Owner::Empty => self.refuse("not booted"),
        }
    }
    /// Set the CSS safe-area environment and re-layout its consumers.
    pub fn insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) -> u32 {
        match &mut self.owner {
            Owner::General(b) => {
                self.binary = false;
                b.insets(top, right, bottom, left)
            }
            Owner::Core(c) => {
                let out = c.insets(Env::new(top, right, bottom, left));
                self.emit(out)
            }
            Owner::Empty => self.refuse("not booted"),
        }
    }
    /// Set or clear one replaced element's natural size.
    pub fn intrinsic(&mut self, view: u32, w: f32, h: f32) -> u32 {
        match &mut self.owner {
            Owner::General(b) => {
                self.binary = false;
                b.intrinsic(view, w, h)
            }
            Owner::Core(c) => {
                let clear = w.is_finite() && h.is_finite() && (w <= 0. || h <= 0.);
                let out = c.intrinsic(&[(view, (!clear).then_some((w, h)))]);
                self.emit(out)
            }
            Owner::Empty => self.refuse("not booted"),
        }
    }
    /// Apply the same fixed 12-byte intrinsic records in one layout turn.
    pub fn intrinsics(&mut self, len: usize) -> u32 {
        if let Owner::General(b) = &mut self.owner {
            self.binary = false;
            return b.intrinsics(len);
        }
        let Some(bytes) = self.input.get(..len).filter(|b| b.len() % 12 == 0) else {
            return self.refuse("intrinsics: truncated record");
        };
        let sizes: Vec<_> = bytes
            .chunks_exact(12)
            .map(|r| {
                let word = |i: usize| r[i..i + 4].try_into().unwrap();
                let (w, h) = (f32::from_le_bytes(word(4)), f32::from_le_bytes(word(8)));
                let clear = w.is_finite() && h.is_finite() && (w <= 0. || h <= 0.);
                (u32::from_le_bytes(word(0)), (!clear).then_some((w, h)))
            })
            .collect();
        match &mut self.owner {
            Owner::Core(c) => {
                let out = c.intrinsic(&sizes);
                self.emit(out)
            }
            _ => self.refuse("not booted"),
        }
    }
    /// Query controls through the existing portable host without a second tree.
    /// The reply is JSON, not a publication; callers must consume its lease first.
    pub fn control_query(&mut self, view: u32, kind: u32) -> u32 {
        self.binary = false;
        if let Owner::General(b) = &mut self.owner {
            return match kind {
                0 => b.press_face(view),
                1 => b.select_options(view),
                2 => b.radio_group(view),
                _ => {
                    let len = b.input_write(br#"{"op":"invalid Android control query"}"#);
                    b.agent(len)
                }
            };
        }
        self.output.clear();
        self.output.extend_from_slice(
            br#"{"error":"control query requires a general native owner and known query"}"#,
        );
        self.output.len() as u32
    }
    /// Record coalesced native scroll facts before the next authored turn.
    pub fn scrolled(&mut self, view: u32, left: f64, top: f64) -> bool {
        if view == 0 || !left.is_finite() || !top.is_finite() {
            return false;
        }
        match &mut self.owner {
            Owner::General(b) => b.scrolled(false, view, left, top),
            Owner::Core(c) => c.scrolled(view, left, top),
            Owner::Empty => return false,
        }
        true
    }
    /// Forward fixed-width collection facts to the shared native runner.
    pub fn collection_feedback(&mut self, len: usize, now: f64) -> u32 {
        if let Owner::General(b) = &mut self.owner {
            self.binary = false;
            return b.collection_feedback(len, now);
        }
        self.refuse("collection feedback requires a general native owner")
    }
    /// Read the shared agent's runner/kernel state; replies remain JSON.
    pub fn agent(&mut self, len: usize) -> u32 {
        if let Owner::General(b) = &mut self.owner {
            self.binary = false;
            return b.agent(len);
        }
        self.binary = false;
        let request = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        self.output = match &self.owner {
            Owner::Core(c) => c.agent(&request).into_bytes(),
            _ => br#"{"error":"not booted"}"#.to_vec(),
        };
        self.output.len() as u32
    }
}

impl<D: DataSource> Default for Bridge<D> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "fallback_plan_tests.rs"]
mod fallback_plan_tests;
