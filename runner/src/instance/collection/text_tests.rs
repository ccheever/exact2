use super::*;

#[test]
fn logical_copy_omits_control_labels_in_unmounted_rows() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let enabled = b.constant(&Value::Bool(true));
    let root = b.node(
        NodeType::List as u8,
        None,
        None,
        0,
        &[binding(
            BindingKind::Prop,
            PropId::Virtualized as u16,
            enabled,
        )],
        &[],
        None,
    );
    let subject = b.constant(&values(128));
    let key = code(&mut b, |a| {
        a.load_item(0);
    });
    let (_, arms) = b.region(RegionKind::Each, Some(root), None, 0, subject, key, 1);
    let row = b.node(NodeType::View as u8, None, Some(arms[0]), 0, &[], &[], None);
    let button = b.constant(&Value::str("button"));
    let control = b.node(
        NodeType::Control as u8,
        Some(row),
        Some(arms[0]),
        0,
        &[binding(BindingKind::Prop, PropId::Type as u16, button)],
        &[],
        None,
    );
    let label = b.constant(&Value::str("Button label"));
    b.node(
        NodeType::Text as u8,
        Some(control),
        Some(arms[0]),
        0,
        &[binding(BindingKind::Prop, PropId::Text as u16, label)],
        &[],
        None,
    );
    let role = b.constant(&Value::str("button"));
    let pressable = b.node(
        NodeType::View as u8,
        Some(row),
        Some(arms[0]),
        1,
        &[binding(
            BindingKind::Prop,
            PropId::AccessibilityRole as u16,
            role,
        )],
        &[],
        None,
    );
    let label = b.constant(&Value::str("Pressable label"));
    b.node(
        NodeType::Text as u8,
        Some(pressable),
        Some(arms[0]),
        0,
        &[binding(BindingKind::Prop, PropId::Text as u16, label)],
        &[],
        None,
    );
    let text = code(&mut b, |a| {
        a.load_item(0).call(exact_plan::Stdlib::ToString);
    });
    b.node(
        NodeType::Text as u8,
        Some(row),
        Some(arms[0]),
        2,
        &[binding(BindingKind::Prop, PropId::Text as u16, text)],
        &[],
        None,
    );
    let h = Harness::from_parts(b.finish().unwrap(), vec![]);
    let view = h.snapshot().view;
    let mounted = h.snapshot().rows.len();
    assert!(mounted < 128);
    let sites = crate::instance::SiteIndex::new(&h.plan);
    let mut ids = Ids::default();
    let mut u = Update::new(env(&h.plan, &h.slots), &sites, &mut ids);
    for policy in [crate::ListTextPolicy::Web, crate::ListTextPolicy::MacOS] {
        let copied = h.tree.list_text(&mut u, view, None, policy).unwrap();
        let web = policy == crate::ListTextPolicy::Web;
        let rows = |range: std::ops::Range<usize>| {
            range
                .map(|i| {
                    if web {
                        format!("Pressable label\n\n{i}")
                    } else {
                        i.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        assert_eq!(copied, rows(0..128));
        let at = |key, offset| crate::ListTextPosition {
            key,
            paragraph: if web { 2 } else { 1 },
            offset,
        };
        let copied = h
            .tree
            .list_text(&mut u, view, Some((at("n:1", 0), at("n:127", 2))), policy)
            .unwrap();
        assert_eq!(
            copied,
            format!(
                "1\n\n{}\n\n{}12",
                rows(2..127),
                if web { "Pressable label\n\n" } else { "" }
            )
        );
    }
    assert_eq!(h.snapshot().rows.len(), mounted, "copy mounts no rows");
    assert!(u.ops.is_empty());
}

/// Text the reader can neither see nor reach is no part of a list's copy,
/// mounted rows or not, as Chrome and AppKit leave it out: a row's text
/// under `visibility: hidden` (a hover card), here or above, unless it
/// computes `visible` again, and `inert` text. Each still counts toward an
/// endpoint's paragraph ordinal, as the host's paragraphs do.
#[test]
fn logical_copy_omits_hidden_and_inert_text() {
    use exact_kernel::StyleId;
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let enabled = b.constant(&Value::Bool(true));
    let root = b.node(
        NodeType::List as u8,
        None,
        None,
        0,
        &[binding(
            BindingKind::Prop,
            PropId::Virtualized as u16,
            enabled,
        )],
        &[],
        None,
    );
    let subject = b.constant(&values(128));
    let key = code(&mut b, |a| {
        a.load_item(0);
    });
    let (_, arms) = b.region(RegionKind::Each, Some(root), None, 0, subject, key, 1);
    let row = b.node(NodeType::View as u8, None, Some(arms[0]), 0, &[], &[], None);
    let hidden = b.constant(&Value::str("hidden"));
    let visible = b.constant(&Value::str("visible"));
    let visibility = |v| binding(BindingKind::Style, StyleId::Visibility as u16, v);
    let text = |b: &mut PlanBuilder, parent, order, value: &str, style: &[BindingsRow]| {
        let value = b.constant(&Value::str(value));
        let mut bindings = vec![binding(BindingKind::Prop, PropId::Text as u16, value)];
        bindings.extend_from_slice(style);
        b.node(
            NodeType::Text as u8,
            Some(parent),
            Some(arms[0]),
            order,
            &bindings,
            &[],
            None,
        );
    };
    let number = code(&mut b, |a| {
        a.load_item(0).call(exact_plan::Stdlib::ToString);
    });
    b.node(
        NodeType::Text as u8,
        Some(row),
        Some(arms[0]),
        0,
        &[binding(BindingKind::Prop, PropId::Text as u16, number)],
        &[],
        None,
    );
    text(&mut b, row, 1, "Card", &[visibility(hidden)]);
    let card = b.node(
        NodeType::View as u8,
        Some(row),
        Some(arms[0]),
        2,
        &[visibility(hidden)],
        &[],
        None,
    );
    text(&mut b, card, 0, "Again", &[visibility(visible)]);
    text(&mut b, card, 1, "Gone", &[]);
    let inert = b.constant(&Value::Bool(true));
    let blocked = b.node(
        NodeType::View as u8,
        Some(row),
        Some(arms[0]),
        3,
        &[binding(BindingKind::Prop, PropId::Inert as u16, inert)],
        &[],
        None,
    );
    text(&mut b, blocked, 0, "Inert", &[]);
    let h = Harness::from_parts(b.finish().unwrap(), vec![]);
    let view = h.snapshot().view;
    let mounted = h.snapshot().rows.len();
    assert!(mounted < 128);
    let sites = crate::instance::SiteIndex::new(&h.plan);
    let mut ids = Ids::default();
    let mut u = Update::new(env(&h.plan, &h.slots), &sites, &mut ids);
    let rows = |range: std::ops::Range<usize>| {
        range
            .map(|i| format!("{i}\n\nAgain"))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    for policy in [crate::ListTextPolicy::Web, crate::ListTextPolicy::MacOS] {
        let copied = h.tree.list_text(&mut u, view, None, policy).unwrap();
        assert_eq!(copied, rows(0..128), "{policy:?}");
        // "Again" is each row's third paragraph: after the number and the card.
        let at = |key, paragraph, offset| crate::ListTextPosition {
            key,
            paragraph,
            offset,
        };
        let copied = h
            .tree
            .list_text(
                &mut u,
                view,
                Some((at("n:1", 0, 0), at("n:127", 2, 3))),
                policy,
            )
            .unwrap();
        assert_eq!(
            copied,
            format!("{}\n\n127\n\nAga", rows(1..127)),
            "{policy:?}"
        );
    }
    assert_eq!(h.snapshot().rows.len(), mounted, "copy mounts no rows");
    assert!(u.ops.is_empty());
}
