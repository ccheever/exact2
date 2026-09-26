//! @ref LLP 1060 D2–D4 — `t(…)` reads the locale slot; the tables it names
//! are baked after every body.

use super::*;
use crate::expr::compile;
use exact_plan::Stdlib;

impl Lowerer<'_> {
    /// `t("key", name=value, …)`, typed already: the locale slot, the key,
    /// and the placeholders as name/value string pairs, into the roster's
    /// `t`. The slot is made by the first call, so an app that never calls
    /// `t` has neither it nor tables.
    pub(crate) fn text_call(
        &mut self,
        asm: &mut Asm,
        args: &[Expr],
        span: Span,
        scope: &Scope,
        locals: &mut u16,
    ) -> Result<Ty, LowerError> {
        let (Some(Expr::Str(key, _)), Some(strings)) = (args.first(), &self.types.shapes.strings)
        else {
            return err(
                "lower-strings",
                "`t` needs its key and the app's tables",
                span,
            );
        };
        let slot = match self.locale {
            Some(slot) => slot,
            None => {
                let ty = self.ty_id(&Ty::String)?;
                let init = self.b.constant(&Value::str(&strings.base));
                let slot = self.b.slot("#locale", ty, init);
                self.locale = Some(slot);
                slot
            }
        };
        self.texts_used.insert(key.clone());
        asm.load_slot(slot);
        asm.str(self.b.str(key));
        for arg in &args[1..] {
            let Expr::NamedArg(name, value, _) = arg else {
                return err(
                    "lower-strings",
                    "a placeholder is filled by name",
                    arg.span(),
                );
            };
            asm.str(self.b.str(name));
            if compile(self, asm, value, scope, locals)? != Ty::String {
                asm.call(Stdlib::ToString);
            }
        }
        asm.list(2 * (args.len() as u32 - 1));
        asm.call(Stdlib::T);
        Ok(Ty::String)
    }

    /// The tables, base first and the rest in name order, each holding only
    /// the keys some `t` names; nothing when none does.
    pub(super) fn bake_texts(&mut self) {
        let (Some(slot), Some(strings)) = (self.locale, self.types.shapes.strings.clone()) else {
            return;
        };
        let base = strings.tables.get_key_value(&strings.base);
        let others = strings.tables.iter().filter(|(l, _)| **l != strings.base);
        for (locale, table) in base.into_iter().chain(others) {
            let texts: Vec<(&str, &str)> = self
                .texts_used
                .iter()
                .filter_map(|key| Some((key.as_str(), table.get(key)?.as_str())))
                .collect();
            self.b.locale(locale, &texts);
        }
        self.b.set_locale(slot);
    }
}
