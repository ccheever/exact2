//! What a node's style rows and an arm's state ask of the whole plan,
//! answered once per emission and looked up per row. Each answer is a walk of
//! the plan's nodes, slots or theme; asked per row, it would make emission
//! quadratic in the plan. Each depends only on the plan and on the
//! thread-local state `emit` installs before it writes the view
//! (`modes::every_for`, `roles::live_properties`,
//! `bridge::set_role_overrides`; `Modes::project` has undone its token edits
//! by then). Writing a row changes neither, so the first row to ask gets what
//! every row would.

use exact_kernel::style::theme::Kind;
use exact_kernel::StyleId;
use exact_plan::Plan;
use std::collections::HashMap;

type Tokens = (Vec<(String, String)>, Vec<(String, StyleId, String)>);

pub(super) struct Answers<'a> {
    plan: &'a Plan,
    /// Whether a type or family token varies by an app axis (`rows.rs`).
    pub varying: bool,
    /// Whether the plan has modes on the web ([`crate::modes::active`]).
    pub modes: bool,
    /// Each arm's own slots, in plan order (`regions.rs`).
    slots: Vec<Vec<usize>>,
    /// [`Answers::styles`].
    styles: Option<Vec<(String, String)>>,
    /// [`Answers::accent`].
    accent: Option<String>,
    /// The tokens' properties ([`crate::modes::properties`]) and the type
    /// tokens' ([`crate::measures::type_properties`]), which a measured
    /// row's map reads.
    tokens: Option<Tokens>,
    /// Each measured row's map ([`crate::measures::map`]), by its row and
    /// its `attribute` and `container` arguments.
    maps: HashMap<(StyleId, bool, bool), Option<String>>,
}

impl<'a> Answers<'a> {
    pub fn new(plan: &'a Plan) -> Answers<'a> {
        let mut slots = vec![Vec::new(); plan.arms.len()];
        for (k, slot) in plan.slots.iter().enumerate() {
            if let Some(own) = slot.owner.and_then(|a| slots.get_mut(a.0 as usize)) {
                own.push(k);
            }
        }
        Answers {
            plan,
            varying: plan.theme.iter().any(|r| {
                r.slices.len > 0
                    && matches!(crate::measures::kind(r), Some(Kind::Type | Kind::Family))
            }),
            modes: crate::modes::active(plan),
            slots,
            styles: None,
            accent: None,
            tokens: None,
            maps: HashMap::new(),
        }
    }

    /// Arm `arm`'s own slots.
    pub fn slots(&self, arm: u32) -> &[usize] {
        self.slots.get(arm as usize).map_or(&[], Vec::as_slice)
    }

    /// Row `id`'s bound map ([`crate::measures::map`]).
    pub fn map(&mut self, id: StyleId, attribute: bool, container: bool) -> Option<&str> {
        let (plan, tokens) = (self.plan, &mut self.tokens);
        self.maps
            .entry((id, attribute, container))
            .or_insert_with(|| {
                let (props, types) = tokens.get_or_insert_with(|| {
                    let every = crate::modes::every_token();
                    (
                        crate::modes::properties(plan),
                        crate::measures::type_properties(plan, every),
                    )
                });
                crate::measures::map(plan, props, types, id, attribute, container)
            })
            .as_deref()
    }

    /// The text styles a bound size's maps read (`text_style::style_map`):
    /// the style a type token's size names, by the token's size property and
    /// by its name.
    pub fn styles(&mut self) -> &[(String, String)] {
        let plan = self.plan;
        self.styles.get_or_insert_with(|| {
            let every = crate::modes::every_token();
            let mut styles = crate::measures::size_styles(plan, every);
            for (name, id, prop) in crate::measures::type_properties(plan, every) {
                if id != StyleId::FontSize {
                    continue;
                }
                if let Some((_, style)) = styles.iter().find(|(v, _)| *v == format!("var({prop})"))
                {
                    styles.push((name, style.clone()));
                }
            }
            styles
        })
    }

    /// The properties of the tokens whose value names the accent in some
    /// tuple, as JSON ([`crate::modes::accent_properties`]).
    pub fn accent(&mut self) -> &str {
        let plan = self.plan;
        self.accent.get_or_insert_with(|| {
            serde_json::to_string(&crate::modes::accent_properties(plan)).unwrap()
        })
    }
}
