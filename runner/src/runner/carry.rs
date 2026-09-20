//! State carried across a restart (LLP 1007 §6, LLP 1027 D5).
use exact_plan::Value;

/// What survives a reload: slots whose names/types still fit, settled
/// resources with matching sources, arguments and logic identity, app secrets, and
/// the clock. A logic edit re-asks resources rather than preserving old answers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Carried {
    /// Router slot name and its value decoded through the old plan's shapes.
    /// @ref LLP 1038 D5 — preserves named params when their field order changes.
    pub router: Option<(String, exact_route::Router)>,
    /// The replaceable data module that produced these resource answers.
    pub data_revision: Option<String>,
    /// Preserve kept-answer persistence when a live candidate is already loaded.
    pub keeps_answers: bool,
    /// Slot name → value.
    pub slots: Vec<(String, Value)>,
    /// Resource name → (source name, arguments, value).
    pub resources: Vec<(String, String, Vec<Value>, Value)>,
    /// (Resource name, source name) pairs whose answer depends on the store.
    pub store_readers: Vec<(String, String)>,
    /// The clock, milliseconds.
    pub now_ms: f64,
    /// The store's kept values (LLP 1018): what the host has persisted.
    pub store: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Answer, DataError, DataSource, Runner, Store};
    use exact_kernel::{Kernel, NodeType};
    use exact_plan::{builder::PlanBuilder, Plan, TypeKind};

    #[derive(Default)]
    struct Source {
        queries: usize,
        deferred: bool,
        pending: bool,
    }
    impl DataSource for Source {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn answer(
            &mut self,
            store: &mut Store,
            source: &str,
            _: &[Value],
        ) -> Result<Answer, DataError> {
            self.queries += 1;
            if self.pending && source == "fresh" {
                return Ok(Answer::Later(crate::Request::get(
                    "https://example.test/answer",
                )));
            }
            Ok(Answer::Now(Value::str(if source == "remember" {
                store.get("token").unwrap_or("")
            } else {
                source
            })))
        }
        fn grants(&self) -> &str {
            "secret.keep token\n"
        }
        fn ready(&self) -> bool {
            !self.deferred
        }
        fn revision(&self) -> Option<&str> {
            Some("unchanged-module")
        }
    }

    fn plan(source: &str, padding: bool) -> Plan {
        let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        if padding {
            b.str("unrelated new declaration");
        }
        let string = b.primitive(TypeKind::String);
        b.resource("answer", source, &[], string, None);
        b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
        b.finish().unwrap()
    }

    #[test]
    fn carried_answers_and_store_dependencies_follow_source_names_not_string_ids() {
        let original = Runner::boot_stored(
            plan("remember", false),
            Source::default(),
            Kernel::with_monospace(),
            vec![("token".into(), "stored answer".into())],
            Default::default(),
            "/",
        )
        .unwrap();
        assert_eq!(
            original.resource("answer"),
            Some(&Value::str("stored answer"))
        );
        assert!(original.resource_reads_store("answer"));
        let carried = original.carry();
        for (source, padding, reused) in [("remember", true, true), ("fresh", false, false)] {
            let next = plan(source, padding);
            assert_eq!(
                original.plan().resources[0].source == next.resources[0].source,
                !reused
            );
            let mut reloaded = Runner::boot_carrying(
                next,
                Source::default(),
                Kernel::with_monospace(),
                &carried,
                Default::default(),
                "/",
            )
            .unwrap();
            assert_eq!(
                reloaded.resource("answer"),
                Some(&Value::str(if reused { "stored answer" } else { "fresh" }))
            );
            assert_eq!(reloaded.data().queries, usize::from(!reused));
            assert_eq!(reloaded.resource_reads_store("answer"), reused);
        }
    }

    #[test]
    fn a_carried_kept_seed_cannot_restore_an_answer_from_another_source() {
        let mut original_plan = plan("remember", false);
        original_plan.resources[0].reader = true;
        let original = Runner::boot_stored(
            original_plan,
            Source {
                deferred: true,
                ..Default::default()
            },
            Kernel::with_monospace(),
            vec![("token".into(), "stored answer".into())],
            Default::default(),
            "/",
        )
        .unwrap();
        let mut carried = original.carry();
        assert!(carried
            .store
            .iter()
            .any(|(name, _)| name == "exact.kept.answer"));
        // Pending resources are omitted from direct carry, but their last kept
        // answer and source-qualified store dependency still survive.
        carried.resources.clear();
        for (source, reused) in [("remember", true), ("fresh", false)] {
            let mut next = plan(source, true);
            next.resources[0].reader = true;
            let mut reloaded = Runner::boot_carrying(
                next,
                Source {
                    deferred: true,
                    ..Default::default()
                },
                Kernel::with_monospace(),
                &carried,
                Default::default(),
                "/",
            )
            .unwrap();
            assert_eq!(
                reloaded.resource("answer"),
                Some(&Value::str(if reused { "stored answer" } else { "fresh" }))
            );
            assert_eq!(reloaded.data().queries, usize::from(!reused));
        }
    }

    #[test]
    fn repeated_reload_cannot_relabel_an_old_kept_seed_as_the_pending_source() {
        let mut old_plan = plan("remember", false);
        old_plan.resources[0].reader = true;
        let original = Runner::boot_stored(
            old_plan,
            Source {
                deferred: true,
                ..Default::default()
            },
            Kernel::with_monospace(),
            vec![("token".into(), "stored answer".into())],
            Default::default(),
            "/",
        )
        .unwrap();
        let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        let string = b.primitive(TypeKind::String);
        b.resource("answer", "fresh", &[], string, Some(&Value::str("loading")));
        b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
        let mut next = b.finish().unwrap();
        next.resources[0].reader = true;
        let mut carried = original.carry();
        for reload in 0..2 {
            let mut reloaded = Runner::boot_carrying(
                next.clone(),
                Source {
                    deferred: true,
                    pending: true,
                    ..Default::default()
                },
                Kernel::with_monospace(),
                &carried,
                Default::default(),
                "/",
            )
            .unwrap();
            assert_eq!(reloaded.resource("answer"), Some(&Value::str("loading")));
            assert_eq!(reloaded.data().queries, 1);
            assert_eq!(reloaded.store().revision(), 0);
            assert_eq!(
                reloaded
                    .store()
                    .writes()
                    .iter()
                    .filter(|write| { write.name == "exact.kept.answer" && write.value.is_none() })
                    .count(),
                usize::from(reload == 0)
            );
            carried = reloaded.carry();
            assert!(carried.resources.is_empty());
            assert!(!carried
                .store
                .iter()
                .any(|(name, _)| name == "exact.kept.answer"));
            assert!(carried
                .store
                .iter()
                .any(|(name, value)| name == "token" && value == "stored answer"));
        }
    }
}
