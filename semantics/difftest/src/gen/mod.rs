//! Random Contract programs and event scripts, for the differential run.
//!
//! [`case`] writes one well-typed program from a seed: shapes, `fn`s,
//! states with literal initializers, resources and a mutation over any
//! source name (the harness's data source answers every one from the
//! declared shape), derives, actions with `let`, `if`, `match`, `send`,
//! repeated writes and calls of the root's earlier actions (LLP 1089),
//! timer tasks, child components with their own state, and
//! a view that shows every slot in a `text` with a `testId`. The script taps
//! buttons (and ids that are missing or inert), types into inputs and
//! advances the clock.
//!
//! Some programs also declare a `routes` table and navigate it (verbs,
//! `path(…)` and the router's reads, [`nav`]), give the mutation a `then`,
//! run a frame task (`every(frame, a)`), or print with the `format*`
//! entries.
//!
//! Expressions are typed by construction ([`expr`]); what the checker
//! refuses that the generator otherwise would write is avoided at the
//! source and noted where it is.

mod action;
mod expr;
mod nav;
mod root;
mod ty;
mod view;

use crate::rng::Rng;
use crate::script::{Case, Event};
use ty::{Shape, Ty, STRINGS};

/// How big a program the generator writes. Each count is an upper bound.
#[derive(Debug, Clone)]
pub struct Size {
    /// Root `state` slots.
    pub states: usize,
    /// Root `derive`s.
    pub derives: usize,
    /// Root actions.
    pub actions: usize,
    /// Expression nesting.
    pub depth: usize,
    /// Script events.
    pub events: usize,
}

impl Default for Size {
    fn default() -> Self {
        Size {
            states: 4,
            derives: 3,
            actions: 4,
            depth: 3,
            events: 12,
        }
    }
}

/// The case for a seed: a program and its script. The same seed and size
/// give the same case.
pub fn case(seed: u64, size: &Size) -> Case {
    let written = |calls: bool| {
        let mut g = Gen::new(seed, size);
        g.calls = calls;
        let source = g.program();
        (source, g.events(), g.called)
    };
    let (mut source, mut events, called) = written(true);
    // A call among statements that the compiler refuses (a read another
    // frame made stale, LLP 1089 D3, or a second send through calls, D6) is
    // written again with calls only in bodies that only call, so the sweep
    // keeps its size.
    if called
        && contract::compile(&source).is_err_and(|e| {
            matches!(
                e.id.as_ref(),
                "analyze-call-stale-read" | "analyze-send-twice"
            )
        })
    {
        (source, events, _) = written(false);
    }
    Case {
        name: format!("gen-{seed}"),
        source,
        events,
        path: None,
    }
}

/// Names and their types in scope.
#[derive(Debug, Clone, Default)]
pub(crate) struct Env {
    pub(crate) vars: Vec<(String, Ty)>,
    /// Whether `now()` may be written.
    pub(crate) now: bool,
    /// How many `fn`s (in declaration order) may be called.
    pub(crate) fns: usize,
}

impl Env {
    pub(crate) fn with(&self, name: &str, t: Ty) -> Env {
        let mut e = self.clone();
        e.vars.push((name.to_string(), t));
        e
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FnSig {
    pub(crate) name: String,
    pub(crate) params: Vec<Ty>,
    pub(crate) ret: Ty,
}

/// An action's name and parameters.
#[derive(Debug, Clone)]
pub(crate) struct ActionSig {
    pub(crate) name: String,
    pub(crate) params: Vec<(String, Ty)>,
}

/// A mutation: its slot, its source and the source's argument types (one
/// source, one signature).
#[derive(Debug, Clone)]
pub(crate) struct Mutation {
    pub(crate) name: String,
    pub(crate) source: String,
    pub(crate) args: Vec<Ty>,
}

/// A child component as a use site needs it.
#[derive(Debug, Clone)]
pub(crate) struct Child {
    pub(crate) name: String,
    /// Props after `idx: number`.
    pub(crate) props: Vec<(String, Ty)>,
    /// `cb: action`, invoked with arguments of these types.
    pub(crate) cb: Option<Vec<Ty>>,
    /// Props a state starts from.
    pub(crate) inits: Vec<String>,
    /// `testId` suffixes of its buttons.
    pub(crate) press: Vec<String>,
    /// `testId` suffixes of its texts.
    pub(crate) inert: Vec<String>,
}

/// What the script may aim at.
#[derive(Debug, Default)]
pub(crate) struct Targets {
    /// Elements with a `press` handler.
    pub(crate) press: Vec<String>,
    /// Inputs with a `change` handler.
    pub(crate) inputs: Vec<String>,
    /// Elements without a handler.
    pub(crate) inert: Vec<String>,
    /// Whether a timer task exists.
    pub(crate) clock: bool,
}

pub(crate) struct Gen<'s> {
    pub(crate) rng: Rng,
    pub(crate) size: &'s Size,
    pub(crate) shapes: Vec<Shape>,
    pub(crate) fns: Vec<FnSig>,
    pub(crate) actions: Vec<ActionSig>,
    pub(crate) mutations: Vec<Mutation>,
    pub(crate) children: Vec<Child>,
    pub(crate) targets: Targets,
    /// Child uses so far.
    pub(crate) uses: u64,
    /// Whether this program declares `routes nav` (its router slot `nav`).
    pub(crate) routes: bool,
    /// Whether this program's strings may use `formatTime`, `formatDate`
    /// and `formatNumber`.
    pub(crate) formats: bool,
    /// The root action the mutation's `then` names.
    pub(crate) then: Option<usize>,
    /// How many of `actions`, from the first, the body being written may
    /// call: the root's earlier ones, none in a child.
    pub(crate) callable: usize,
    /// Whether a call may stand among other statements, and whether one
    /// was written.
    pub(crate) calls: bool,
    pub(crate) called: bool,
    /// The mutations a path of the body being written has sent, its calls'
    /// included: each is sent once per path (LLP 1088 D8, `analyze-send-twice`).
    pub(crate) sent: Vec<String>,
    /// The mutations any path of the body being written sends.
    pub(crate) sending: Vec<String>,
    /// The mutations each of `actions` may send, by index, once written.
    pub(crate) sends: Vec<Vec<String>>,
    next: usize,
}

const FIELDS: &[&str] = &[
    "id", "name", "n", "flag", "tag", "items", "sub", "score", "label", "note", "on", "at",
];

impl<'s> Gen<'s> {
    fn new(seed: u64, size: &'s Size) -> Self {
        Gen {
            rng: Rng::new(seed),
            size,
            shapes: Vec::new(),
            fns: Vec::new(),
            actions: Vec::new(),
            mutations: Vec::new(),
            children: Vec::new(),
            targets: Targets::default(),
            uses: 0,
            routes: false,
            formats: false,
            then: None,
            callable: 0,
            calls: true,
            called: false,
            sent: Vec::new(),
            sending: Vec::new(),
            sends: Vec::new(),
            next: 0,
        }
    }

    /// A name no other in the program has.
    pub(crate) fn fresh(&mut self, prefix: &str) -> String {
        self.next += 1;
        format!("{prefix}{}", self.next)
    }

    /// Whether a value of `t` holds a string, so a write of it could grow
    /// without bound across events.
    pub(crate) fn holds_str(&self, t: &Ty) -> bool {
        match t {
            Ty::Str => true,
            Ty::Num | Ty::Bool => false,
            Ty::Opt(t) | Ty::List(t) => self.holds_str(t),
            Ty::Rec(i) => self.shapes[*i]
                .fields
                .iter()
                .any(|(_, t)| self.holds_str(t)),
        }
    }

    fn program(&mut self) -> String {
        let mut out = String::new();
        // Each feature in about one program in five, so most cases stay
        // with what every other part of the generator exercises.
        self.routes = self.rng.chance(1, 5);
        self.formats = self.rng.chance(1, 5);
        self.gen_shapes(&mut out);
        self.gen_fns(&mut out);
        if self.routes {
            out.push_str(nav::TABLE);
            out.push('\n');
        }
        let root = self.root_decls();
        // The first component is the root, so the children follow it.
        let mut children = String::new();
        self.gen_children(&mut children);
        out.push_str(&self.root(root));
        out.push('\n');
        out.push_str(&children);
        out
    }

    fn gen_shapes(&mut self, out: &mut String) {
        for k in 0..self.rng.range(0, 3) {
            let mut names: Vec<&str> = FIELDS.to_vec();
            self.maybe_shuffle(&mut names);
            let mut fields = Vec::new();
            for name in names.iter().take(self.rng.range(1, 4)) {
                let t = self.field_ty(k);
                fields.push((name.to_string(), t));
            }
            out.push_str(&format!("shape S{k}\n"));
            for (f, t) in &fields {
                out.push_str(&format!("  {f}: {}\n", self.ty_name(t)));
            }
            out.push('\n');
            self.shapes.push(Shape {
                name: format!("S{k}"),
                fields,
            });
        }
    }

    /// A field of shape `k`: shapes before it only, so none is recursive.
    fn field_ty(&mut self, k: usize) -> Ty {
        let base = if k > 0 && self.rng.chance(1, 4) {
            Ty::Rec(self.rng.below(k as u64) as usize)
        } else {
            self.scalar_ty()
        };
        match self.rng.weighted(&[6, 2, 2]) {
            0 => base,
            1 => Ty::opt(base),
            _ => Ty::list(base),
        }
    }

    fn gen_fns(&mut self, out: &mut String) {
        for k in 0..self.rng.range(0, 3) {
            let params: Vec<(String, Ty)> = (0..self.rng.range(0, 3))
                .map(|_| (self.fresh("p"), self.any_ty()))
                .collect();
            let ret = self.any_ty();
            let env = Env {
                vars: params.clone(),
                now: true,
                fns: k,
            };
            let body = self.expr(&env, &ret, self.size.depth, true);
            let ps: Vec<String> = params
                .iter()
                .map(|(n, t)| format!("{n}: {}", self.ty_name(t)))
                .collect();
            let name = format!("g{k}");
            out.push_str(&format!(
                "fn {name}({}): {} = {body}\n",
                ps.join(", "),
                self.ty_name(&ret)
            ));
            self.fns.push(FnSig {
                name,
                params: params.into_iter().map(|(_, t)| t).collect(),
                ret,
            });
        }
        if !self.fns.is_empty() {
            out.push('\n');
        }
    }

    fn events(&mut self) -> Vec<Event> {
        let n = self
            .rng
            .range(self.size.events / 2, self.size.events.max(1));
        let t = &self.targets;
        let w = [
            if t.press.is_empty() { 0 } else { 14 },
            2,
            if t.inputs.is_empty() { 0 } else { 4 },
            if t.clock { 3 } else { 0 },
        ];
        let mut out = Vec::new();
        for _ in 0..n {
            out.push(match self.rng.weighted(&w) {
                0 => Event::Tap(self.rng.pick(&self.targets.press).clone()),
                1 => {
                    let mut others: Vec<String> = self.targets.inert.clone();
                    others.extend(self.targets.inputs.iter().cloned());
                    others.push("missing".into());
                    others.push("root".into());
                    Event::Tap(self.rng.pick(&others).clone())
                }
                2 => {
                    let id = self.rng.pick(&self.targets.inputs).clone();
                    let text = if self.rng.chance(1, 4) {
                        number_text(self.rng.below(2000) as f64 / 4.0 - 100.0)
                    } else {
                        self.rng.pick(STRINGS).to_string()
                    };
                    Event::Type(id, text)
                }
                _ => Event::Clock(
                    *self
                        .rng
                        .pick(&[1.0, 16.0, 99.0, 100.0, 250.0, 999.0, 1000.0, 1001.0, 2500.0]),
                ),
            });
        }
        out
    }
}

fn number_text(n: f64) -> String {
    format!("{n}")
}
