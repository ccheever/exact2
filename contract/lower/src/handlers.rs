//! A handler attribute: its action named, its arity and payload types
//! checked against the event (LLP 1006 §8, LLP 1017 P1b), its bound
//! arguments lowered.
use crate::{err, LowerError, Lowerer};
use contract_syntax::{Attr, Expr};
use contract_types::{Scope, Ty};
use exact_plan::{ActionsId, Code, EventKind};

impl Lowerer<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handler(
        &mut self,
        tag: &str,
        event: &str,
        control: Option<&str>,
        a: &Attr,
        scope: &Scope,
        locals: u16,
        handlers: &mut Vec<(EventKind, ActionsId, Vec<Code>)>,
    ) -> Result<(), LowerError> {
        // HTML submits implicitly from a single-line input, never a
        // textarea, whose Enter breaks the line: one behaviour on
        // every host (kanban F21, chat F2 in the x2apps diaries).
        if event == "submit" && tag == "textarea" {
            return err(
                "lower-handler-tag",
                "a `textarea` has no `submit`: its Enter breaks the line, as HTML's does; for Enter to send, take the key's event (`key=compose` with `action compose(k: string, e: KeyboardEvent)`) and, when `k == \"Enter\" and not e.shiftKey`, send and call `preventDefault()`",
                a.span,
            );
        }
        let (name, args): (&str, &[Expr]) = match &a.value {
            Expr::Ident(n, _) => (n, &[]),
            Expr::Call(n, args, _) => (n, args),
            _ => {
                return err(
                    "lower-handler",
                    "a handler is an action name or `action(args)`",
                    a.span,
                )
            }
        };
        let Some(ai) = self.root.actions.iter().position(|x| x.name == name) else {
            return err(
                "lower-unknown-action",
                format!("`{name}` is not an action of the root"),
                a.span,
            );
        };
        // The view is inlined, so a handler behind a child's `action`
        // prop names the real action here: its arity is checked now,
        // not at dispatch (LLP 1006 §8's circle-back; LLP 1017 P1b).
        let valid = contract_analyze::handler_accepts(
            event,
            args.len(),
            &self.types.components[0].actions[ai],
        );
        if !valid {
            let params: Vec<(String, Ty)> = (self.root.actions[ai].params.iter())
                .map(|p| p.name.clone())
                .zip(self.types.components[0].actions[ai].iter().cloned())
                .collect();
            let arg_types: Vec<Option<Ty>> = args
                .iter()
                .map(|a| contract_types::infer(a, scope, &self.types.shapes).ok())
                .collect();
            return err(
                "lower-handler-arity",
                contract_analyze::handler_arity_message(
                    event, control, name, args, &params, &arg_types,
                ),
                a.span,
            );
        }
        if event == "reorderdrop"
            && self.types.components[0].actions[ai][args.len()..]
                != [Ty::String, Ty::Option(Box::new(Ty::String))]
        {
            return err(
                "lower-handler-type",
                "`reorderdrop` supplies string and option<string>",
                a.span,
            );
        }
        if matches!(
            event,
            "pan" | "panrelease" | "heightrelease" | "transformgeometry" | "transformrelease"
        ) && self.types.components[0].actions[ai][args.len()..]
            .iter()
            .any(|ty| *ty != Ty::Number)
        {
            return err(
                "lower-handler-type",
                format!("`{event}` supplies only numeric payload parameters"),
                a.span,
            );
        }
        let mut codes = Vec::new();
        for arg in args {
            codes.push(self.expr_code(arg, scope, locals)?);
        }
        let kind = EventKind::from_name(event).expect("tag table admitted an unknown handler");
        handlers.push((kind, self.actions[ai], codes));
        Ok(())
    }
}
