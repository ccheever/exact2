//! Folding and binding a design for the styles that follow (LLP 1121 D3,
//! D5, D7): the compiler's `declare_theme` and the design server's write
//! check (`rows`) share these, so a row is judged under the same table.

use super::*;

/// `design` folded with its synthetic tokens (`~i`, each of `synthetic`):
/// one refused that isn't strict is dropped quietly, one that is said
/// (`refuse`, with the list then) and dropped too, so the design and the
/// expressions that folded stay bound; a refusal in the design itself is
/// said and binds none (`None`). The compiler's fold and the design
/// server's write check's ([`rows::check`]).
pub(crate) fn fold(
    design: &tamber::Design,
    synthetic: &mut Vec<scan::Synthetic>,
    refuse: &mut dyn FnMut(&tamber::Diagnostic, &[scan::Synthetic]),
) -> Option<tamber::Folding> {
    loop {
        let mut checked = design.clone();
        for (i, s) in synthetic.iter().enumerate() {
            let at = |value: String| tamber::At {
                value,
                line: SYNTHETIC + i as u32,
                col: 1,
            };
            checked.tokens.push(tamber::Token {
                name: at(format!("~{i}")),
                kind: None,
                value: at(s.expr.clone()),
                whens: Vec::new(),
                meta: Vec::new(),
            });
        }
        match tamber::check_design(checked, &vocab()) {
            Ok(f) => return Some(f),
            Err(all) => {
                let loose = |d: &tamber::Diagnostic| {
                    d.line >= SYNTHETIC && !synthetic[(d.line - SYNTHETIC) as usize].strict
                };
                for d in all.iter().filter(|d| !loose(d)) {
                    refuse(d, synthetic);
                }
                if !all.iter().all(|d| d.line >= SYNTHETIC) {
                    return None;
                }
                let dropped: Vec<usize> =
                    all.iter().map(|d| (d.line - SYNTHETIC) as usize).collect();
                let mut i = 0;
                synthetic.retain(|_| {
                    i += 1;
                    !dropped.contains(&(i - 1))
                });
            }
        }
    }
}

/// Takes the profile colours (LLP 1100 D3) out of the crate's fold: an
/// entry that is one, and an entry naming such an entry whole, are
/// declared to it as `transparent` (so their names are still checked)
/// and carried as written. Another use of one's name is refused, as the
/// crate cannot fold it.
pub(crate) fn profiles(
    sink: &mut dyn Refusals,
    design: &mut tamber::Design,
    origin: &Origin,
) -> Vec<Profiled> {
    let same = same_name;
    let mut out: Vec<Profiled> = Vec::new();
    loop {
        // A profile's colour is the default app tuple's, which is all the
        // plan carries until it carries the axes (stage 2, commit 3).
        // (one past the checker's bounds is left as written, for the check
        // to refuse as the design server's does: Astra's round 2)
        let found = design.tokens.iter().position(|t| {
            let text = t.value.value.trim();
            tamber::within_bounds(&t.value.value)
                && !out.iter().any(|p| same(&p.name, &t.name.value))
                && (text.to_ascii_lowercase().starts_with("color(--") && parses(text)
                    || t.whens.is_empty() && out.iter().any(|p| same(&p.name, text)))
        });
        let Some(i) = found else { break };
        // A `when` that holds at the default modes would be the default's.
        let axes = &design.axes;
        let defaults = |w: &tamber::When| {
            w.conds.iter().all(|(a, v)| {
                axes.iter()
                    .any(|x| x.name.value == *a && x.values.first() == Some(v))
            })
        };
        if let Some(w) = design.tokens[i].whens.iter().find(|w| defaults(w)) {
            let span = sink.span(origin, w.line, w.col);
            sink.push(LowerError {
                id: "lower-theme-value",
                message: format!("`{}`: this `when` holds at the default modes, where the token is a profile's colour (`color(--…)`): the plan carries the default modes' value, which a profile's colour must be", design.tokens[i].name.value),
                span,
            });
        }
        let t = &mut design.tokens[i];
        let text = t.value.value.trim().to_string();
        let value = match out.iter().find(|p| same(&p.name, &text)) {
            Some(p) => p.value.clone(),
            None => text,
        };
        out.push(Profiled {
            name: t.name.value.clone(),
            value,
        });
        t.value.value = "transparent".into();
    }
    for t in &design.tokens {
        // A profile token's default is taken; its `when` values are read.
        let profile = out.iter().any(|p| p.name == t.name.value);
        let values = std::iter::once(&t.value)
            .filter(|_| !profile)
            .chain(t.whens.iter().map(|w| &w.value));
        let names = |v: &tamber::At<String>| {
            scan::words(&v.value).any(|w| out.iter().any(|p| same(&p.name, w)))
        };
        if values.into_iter().any(names) {
            let span = sink.span(origin, t.name.line, t.name.col);
            sink.push(LowerError {
                id: "lower-theme-value",
                message: format!("`{}`: a profile's colour (`color(--…)`) is named whole by another token, not inside an expression or under `when`", t.name.value),
                span,
            });
        }
    }
    out
}

/// Where a design's refusals go: [`profiles`]', [`bind`]'s.
pub(crate) trait Refusals {
    fn push(&mut self, e: LowerError);
    fn span(&self, origin: &Origin, line: u32, col: u32) -> Span;
}

impl Refusals for Lowerer<'_> {
    fn push(&mut self, e: LowerError) {
        Lowerer::push(self, e);
    }
    fn span(&self, origin: &Origin, line: u32, col: u32) -> Span {
        self.token_span(origin, line, col)
    }
}

/// What binding a folded design needs of its caller ([`bind`]): where a
/// token's line is, where a refusal goes, the plan's records of the
/// entries, and the kernel's value of each (a type token's stack).
pub(crate) trait Sink: Refusals {
    /// Whether a refusal at `span` saying `text` was said already.
    fn said(&self, span: Span, text: &str) -> bool;
    /// The plan's axes and kept tokens, of `entries`.
    fn plan(&mut self, entries: &[ThemeEntry], folding: &tamber::Folding);
    /// The kernel's value of each of `entries`.
    fn stacks(&mut self, entries: &[ThemeEntry]) -> Vec<String>;
}

/// Bind `folding` (its synthetic tokens `~i` named by `exprs`, its
/// profile colours `profiled` carried as written) for the styles that
/// follow: the theme table, each entry's refusals, the motion, measure,
/// scale and axis tables. The entries, and the scope the table holds for.
/// The compiler's (`declare_theme`) and the design server's write check's
/// ([`rows::check`]) one binding.
pub(crate) fn bind(
    sink: &mut dyn Sink,
    folding: &tamber::Folding,
    exprs: &[String],
    profiled: &[Profiled],
    origin: &Origin,
) -> (Vec<ThemeEntry>, exact_kernel::style::theme::ThemeScope) {
    let mut entries = Vec::new();
    for (i, t) in folding.tokens.iter().enumerate() {
        let Some(value) = t.values.first() else {
            continue;
        };
        let synthetic = t
            .name
            .strip_prefix('~')
            .and_then(|n| n.parse::<usize>().ok());
        // A token's name as written (LLP 1095 D4's bare `secondary-label`).
        let name = match synthetic {
            Some(n) => exprs[n].clone(),
            None => folding.design.tokens[i].name.value.clone(),
        };
        let role = synthetic.is_none().then(|| role_named(&name)).flatten();
        let at = &folding.design.tokens[i].name;
        if synthetic.is_none()
            && value.color().is_some_and(|v| {
                !admitted(sink, &name, role.as_ref(), v, (at.line, at.col), origin)
            })
        {
            continue;
        }
        // A profile's colour, carried as written, is the default's.
        let profile = profiled
            .iter()
            .find(|p| synthetic.is_none() && p.name == name);
        let mut slices: Vec<String> = t.values.iter().map(|v| v.css()).collect();
        // Every tuple the default stands in, which no `when` replaced.
        if let Some(p) = profile {
            let stand_in = slices[0].clone();
            for s in slices.iter_mut().filter(|s| **s == stand_in) {
                *s = p.value.clone();
            }
        }
        let value = slices[0].clone();
        if slices.iter().all(|s| *s == value) {
            slices.clear();
        }
        // A role crosses to the hosts once, for the surfaces they draw
        // (LLP 1095 D4's reach): it has one value, whatever the modes.
        if role.is_some() && !slices.is_empty() {
            sink.push(LowerError {
                id: "tamber-role-axis",
                message: format!("`{name}` overrides a role and varies by an app axis: a role has one value, which the hosts' own surfaces read; vary a token the rows name instead (LLP 1121 D5)"),
                span: sink.span(origin, at.line, at.col),
            });
            continue;
        }
        // An expression over base names every host can fold (stage 12
        // B2), and a shadow over them every host can draw (B3).
        let refused = std::iter::once(&value)
            .chain(&slices)
            .find_map(|v| crate::scale::refusal(t.kind, &name, v));
        if let Some((id, message)) = refused {
            let span = sink.span(origin, at.line, at.col);
            // A ranged param's own check said it already, at its line.
            if !sink.said(span, "doesn't fold to a length") {
                sink.push(LowerError { id, message, span });
            }
            continue;
        }
        if let Some((_, row)) = &role {
            let span = sink.span(origin, at.line, at.col);
            let role = row.strip_prefix("-exact-").unwrap_or(row).to_string();
            ROLES.with(|r| r.borrow_mut().push((role, span)));
        }
        entries.push(ThemeEntry {
            name,
            role: role.map(|(_, row)| row),
            value,
            slices,
            kind: t.kind,
        });
    }
    // A motion token is written into its rows here (D3): by name.
    MOTION.with(|m| {
        *m.borrow_mut() = entries
            .iter()
            .filter(|e| matches!(e.kind, tamber::Kind::Duration | tamber::Kind::Easing))
            .map(|e| (e.name.clone(), e.value.clone()))
            .collect()
    });
    crate::scale::declare(folding.tokens.iter().map(|t| t.name.clone()).collect());
    MEASURES.with(|m| {
        *m.borrow_mut() = entries
            .iter()
            .filter(|e| e.kind != tamber::Kind::Color && kind_code(e.kind).is_some())
            .map(|e| {
                let mut values = vec![e.value.clone()];
                for v in &e.slices {
                    if !values.contains(v) {
                        values.push(v.clone());
                    }
                }
                (e.name.clone(), e.kind, values)
            })
            .collect()
    });
    // The app's axes, which a pin and `setMode` name (D5).
    AXES.with(|axes| {
        *axes.borrow_mut() = folding
            .design
            .axes
            .iter()
            .map(|a| (a.name.value.clone(), a.values.clone()))
            .collect()
    });
    sink.plan(&entries, folding);
    // A type or family token's family is a stack the plan declares, as
    // a literal `font-family`'s is, and a bound table holds its id
    // (stage 6 C).
    let kernel_values = sink.stacks(&entries);
    let scope = exact_kernel::style::theme::bind(&exact_kernel::style::theme::kinded(
        entries
            .iter()
            .zip(&kernel_values)
            .flat_map(|(e, kernel_value)| {
                let tint = e
                    .role
                    .as_deref()
                    .and_then(roles::role)
                    .is_some_and(roles::tint);
                let kind = kernel_kind(e.kind);
                kind.filter(|_| !tint)
                    .map(|kind| {
                        (
                            e.name.as_str(),
                            kernel_value.as_str(),
                            false,
                            kind,
                            !e.slices.is_empty(),
                        )
                    })
                    .into_iter()
                    .chain(e.role.as_deref().filter(|_| !tint).and_then(|r| {
                        Some((r, kernel_value.as_str(), true, kind?, !e.slices.is_empty()))
                    }))
            }),
    ));
    (entries, scope)
}

/// The refusals a design's token earns beyond the crate's: a name that
/// shadows a value's words, and the tint rule (LLP 1095 D1).
fn admitted(
    sink: &mut dyn Sink,
    name: &str,
    role: Option<&(u8, String)>,
    value: &tamber::Folded,
    (line, col): (u32, u32),
    origin: &Origin,
) -> bool {
    let span = sink.span(origin, line, col);
    let refuse = |id: &'static str, message: String| LowerError { id, message, span };
    let error = match role {
        None if RESERVED.contains(&name.to_ascii_lowercase().as_str()) => Some(refuse(
            "lower-theme-name",
            format!("`{name}` is a CSS keyword; name the token otherwise"),
        )),
        // The tint is the app's: `AccentColor` sets it; a role drawn in
        // it follows (LLP 1095 D1, an authored accent over the design).
        Some((id, _)) if roles::tint(*id) && *id != roles::accent() => Some(refuse(
            "lower-theme-tint",
            format!("`{name}` is drawn in the app's tint: a theme sets the tint with `AccentColor`"),
        )),
        Some((id, _)) if *id == roles::accent() => match value {
            tamber::Folded::Host(tamber::Host::Role(row))
                if roles::role(row).is_some_and(roles::tint) =>
            {
                Some(refuse("lower-theme-tint", format!("`{name}=\"{row}\"` names a role drawn in the tint itself: the tint is a colour, a platform colour or another role")))
            }
            _ => None,
        },
        _ => None,
    };
    match error {
        Some(e) => {
            sink.push(e);
            false
        }
        None => true,
    }
}

/// The compiler's [`Sink`]: its refusals, spans and plan.
pub(super) struct Lowering<'a, 'b> {
    pub(super) l: &'a mut Lowerer<'b>,
    pub(super) synthetic: &'a [scan::Synthetic],
    pub(super) retention: &'a tamber::Retention,
}

impl Refusals for Lowering<'_, '_> {
    fn push(&mut self, e: LowerError) {
        self.l.push(e);
    }
    fn span(&self, origin: &Origin, line: u32, col: u32) -> Span {
        self.l.token_span(origin, line, col)
    }
}

impl Sink for Lowering<'_, '_> {
    fn said(&self, span: Span, text: &str) -> bool {
        self.l
            .errors
            .iter()
            .any(|e| e.span == span && e.message.contains(text))
    }
    fn plan(&mut self, entries: &[ThemeEntry], folding: &tamber::Folding) {
        for axis in &folding.design.axes {
            let values: Vec<&str> = axis.values.iter().map(String::as_str).collect();
            self.l.b.axis(&axis.name.value, &values);
        }
        // D7's retention: what the view may name, every role override, and
        // in an app that can name a token outside its views (generated
        // bindings, D12) every token, or those its `exports` block lists;
        // and each synthetic token its rows write. The design server's
        // `propose` asks the same `Retention` (LLP 1121 stage 5).
        for e in entries {
            let Some(code) = kind_code(e.kind) else {
                continue;
            };
            let synthetic = self.synthetic.iter().any(|s| s.expr == e.name);
            if synthetic
                || self
                    .retention
                    .keeps(&folding.design, &e.name, e.role.as_deref(), e.kind)
            {
                let slices: Vec<&str> = e.slices.iter().map(String::as_str).collect();
                self.l.b.theme_token(
                    &e.name,
                    e.role.as_deref().unwrap_or(""),
                    &e.value,
                    code,
                    &slices,
                );
            }
        }
    }
    fn stacks(&mut self, entries: &[ThemeEntry]) -> Vec<String> {
        self.l.theme = entries.to_vec();
        self.l.theme_stacks()
    }
}
