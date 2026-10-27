//! The rows UIKit draws for a grouped list (LLP 1084): the accessories the
//! platform gives a row its author left unsaid.

use crate::browser_cases::props;
use crate::support::reader::text as t;
use exact_kernel::*;

/// A grouped list of one section, its card's one row a link to `href` with a title.
fn list(href: &str) -> Kernel {
    list_of(href, vec![], vec![4])
}

/// The same list, `more` applied before the row is given `children`.
fn list_of(href: &str, more: Vec<Op>, children: Vec<u32>) -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::List,
        },
        Op::SetProp {
            id: 1,
            prop: PropId::ListStyle,
            value: PropValue::Str("inset-grouped".into()),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 5,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::Pressable,
        },
        Op::SetProp {
            id: 3,
            prop: PropId::Href,
            value: PropValue::Str(href.into()),
        },
        Op::CreateView {
            id: 4,
            node_type: NodeType::Text,
        },
        Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: PropValue::Str("Map".into()),
        },
    ];
    ops.extend(more);
    ops.extend([
        Op::SetChildren { id: 3, children },
        Op::SetChildren {
            id: 5,
            children: vec![3],
        },
        Op::SetChildren {
            id: 2,
            children: vec![5],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ]);
    kernel.apply(0, 1, &ops).unwrap();
    kernel
}

fn accessory(kernel: &Kernel) -> Accessory {
    kernel.grouped_list(1).unwrap().sections[0].rows[0].accessory
}

/// The row laid out as a column.
fn column() -> Op {
    Op::SetStyle {
        id: 3,
        patch: Box::new(props(&vec![
            (StyleId::Display, t("flex")),
            (StyleId::FlexDirection, t("column")),
        ])),
    }
}

#[test]
fn a_row_that_links_into_the_app_shows_the_disclosure_indicator() {
    assert_eq!(accessory(&list("/map")), Accessory::Disclosure);
    assert_eq!(
        accessory(&list("item/2")),
        Accessory::Disclosure,
        "a relative path too"
    );
}

#[test]
fn a_row_that_links_out_of_the_app_shows_none() {
    assert_eq!(accessory(&list("https://example.com/map")), Accessory::None);
    assert_eq!(accessory(&list("mailto:a@example.com")), Accessory::None);
    assert_eq!(accessory(&list("//example.com/map")), Accessory::None);
    assert_eq!(
        accessory(&list("#intro")),
        Accessory::None,
        "this page's own"
    );
    assert_eq!(
        accessory(&list("?tab=2")),
        Accessory::None,
        "this page's own"
    );
}

#[test]
fn a_row_laid_out_as_a_column_shows_it_too() {
    assert_eq!(
        accessory(&list_of("/map", vec![column()], vec![4])),
        Accessory::Disclosure
    );
}

#[test]
fn a_custom_row_laid_out_as_a_column_shows_none() {
    let ops = vec![
        column(),
        Op::CreateView {
            id: 6,
            node_type: NodeType::View,
        },
    ];
    let kernel = list_of("/map", ops, vec![6]);
    let row = &kernel.grouped_list(1).unwrap().sections[0].rows[0];
    assert!(row.custom);
    assert_eq!(row.accessory, Accessory::None);
}

#[test]
fn an_authored_accessory_is_kept() {
    let check = vec![
        Op::CreateView {
            id: 6,
            node_type: NodeType::Image,
        },
        Op::SetProp {
            id: 6,
            prop: PropId::ImageSource,
            value: PropValue::Str("symbol:sf/checkmark".into()),
        },
    ];
    assert_eq!(
        accessory(&list_of("/map", check, vec![4, 6])),
        Accessory::Checkmark
    );
}
