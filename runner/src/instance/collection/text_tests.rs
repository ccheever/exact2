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
