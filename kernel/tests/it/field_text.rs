//! A text control's own text, measured by the host's control where that
//! control sets it with a text engine of its own (LLP 1115.001 D8: on
//! Android a field is the platform's `EditText`): the measurer's
//! `field_text` answers for a text control and only for one, and a host
//! that paints its fields answers nothing and is measured as before.
use exact_kernel::*;
use std::{cell::RefCell, rc::Rc};

type Asked = Rc<RefCell<Vec<(FieldKind, String)>>>;

/// A paragraph is 100 by 20; the host's own field, when it has one, sets a
/// text 64 by 30.
struct Hosts {
    asked: Asked,
    own: bool,
}
impl TextMeasurer for Hosts {
    fn measure(&mut self, _: &TextMeasureRequest<'_>) -> TextMetrics {
        TextMetrics {
            width: 100.0,
            height: 20.0,
            first_baseline: Some(15.0),
        }
    }
    fn field_text(
        &mut self,
        kind: FieldKind,
        request: &TextMeasureRequest<'_>,
    ) -> Option<TextMetrics> {
        let text: String = request.runs.iter().map(|run| &*run.text).collect();
        self.asked.borrow_mut().push((kind, text));
        self.own.then_some(TextMetrics {
            width: 64.0,
            height: 30.0,
            first_baseline: Some(22.0),
        })
    }
}

/// A field, a text beside it and a textarea, in a root.
fn tree(own: bool) -> (Kernel, Asked) {
    let asked = Asked::default();
    let mut k = Kernel::new(Box::new(Hosts {
        asked: asked.clone(),
        own,
    }));
    let view = |id, node_type| Op::CreateView { id, node_type };
    let prop = |id, prop, value: &str| Op::SetProp {
        id,
        prop,
        value: PropValue::Str(value.into()),
    };
    k.apply(
        0,
        0,
        &[
            view(1, NodeType::View),
            view(2, NodeType::TextInput),
            view(3, NodeType::Text),
            view(4, NodeType::TextInput),
            prop(3, PropId::Text, "beside"),
            prop(4, PropId::SemanticTag, "textarea"),
            Op::SetChildren {
                id: 1,
                children: vec![2, 3, 4],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(600.0, 500.0)).unwrap();
    (k, asked)
}

fn size(k: &Kernel, id: u32) -> (f32, f32) {
    let frame = k.node(id).unwrap().frame;
    (frame.width, frame.height)
}

#[test]
fn a_host_whose_field_sets_its_own_text_measures_it_during_layout() {
    let (k, asked) = tree(true);
    // The field's line and its twenty characters are the control's own.
    assert_eq!(size(&k, 2), (64.0, 30.0));
    // A textarea's two measured lines stand for its two rows.
    assert_eq!(size(&k, 4), (64.0, 30.0));
    // The text beside them is a paragraph, measured as any.
    assert_eq!(size(&k, 3).1, 20.0);
    let asked = asked.borrow();
    assert!(asked.contains(&(FieldKind::Field, "0".repeat(20))));
    assert!(asked
        .iter()
        .any(|(kind, text)| *kind == FieldKind::Textarea && text.contains('\n')));
    assert!(asked.iter().all(|(_, text)| text != "beside"));
}

#[test]
fn a_host_that_paints_its_fields_measures_them_as_paragraphs() {
    let (k, asked) = tree(false);
    assert_eq!(size(&k, 2), (100.0, 20.0));
    assert_eq!(size(&k, 4), (100.0, 20.0));
    assert!(!asked.borrow().is_empty(), "asked, and it answered nothing");
}
