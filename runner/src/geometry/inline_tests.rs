use super::*;
use exact_kernel::{NodeType, Offer, Op};
use std::{cell::Cell, rc::Rc};

#[derive(Debug)]
struct Read {
    calls: Cell<usize>,
    answer: Cell<GeometryAnswer>,
}

impl InlineGeometry for Read {
    fn frame(&self, view: ViewId) -> GeometryAnswer {
        assert_eq!(view, 3);
        self.calls.set(self.calls.get() + 1);
        self.answer.get()
    }
}

#[test]
fn inline_frames_are_read_on_demand_only_for_live_laid_out_runs() {
    let mut kernel = Kernel::with_monospace();
    let mut ops: Vec<_> = (1..=3)
        .map(|id| Op::CreateView {
            id,
            node_type: NodeType::Text,
        })
        .collect();
    ops.extend([
        Op::SetProp {
            id: 3,
            prop: PropId::Text,
            value: "wrapped link".into(),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3],
        },
        Op::AttachRoot { id: 1 },
    ]);
    kernel.apply(0, 1, &ops).unwrap();
    let answer = GeometryAnswer {
        x: 10.25,
        y: -8.5,
        width: 80.5,
        height: 36.0,
        ..Default::default()
    };
    let read = Rc::new(Read {
        calls: Cell::new(0),
        answer: Cell::new(answer),
    });
    let mut scrolled = Scrolled {
        inline: Some(read.clone()),
        ..Default::default()
    };
    assert_eq!(
        kernel_frame(&kernel, &scrolled, 3),
        GeometryAnswer::UNAVAILABLE
    );
    assert_eq!(read.calls.get(), 0);
    kernel
        .compute_layout(1, Offer::definite(200.0, 200.0))
        .unwrap();
    assert!(!kernel_frame(&kernel, &scrolled, 1).unavailable);
    assert_eq!(
        read.calls.get(),
        0,
        "ordinary boxes never call the presenter"
    );
    scrolled.note(&kernel, None, 0.0, 100.0);
    assert_eq!(
        kernel_frame(&kernel, &scrolled, 3),
        answer,
        "already in viewport space"
    );
    assert_eq!(read.calls.get(), 1);
    for bad in [
        GeometryAnswer {
            x: f64::NAN,
            ..answer
        },
        GeometryAnswer {
            width: -1.0,
            ..answer
        },
        GeometryAnswer::UNAVAILABLE,
    ] {
        read.answer.set(bad);
        assert_eq!(
            kernel_frame(&kernel, &scrolled, 3),
            GeometryAnswer::UNAVAILABLE
        );
    }
    read.answer.set(answer);
    kernel
        .apply(
            0,
            2,
            &[
                Op::SetChildren {
                    id: 2,
                    children: vec![],
                },
                Op::DestroyView { id: 3 },
            ],
        )
        .unwrap();
    let before = read.calls.get();
    assert_eq!(
        kernel_frame(&kernel, &scrolled, 3),
        GeometryAnswer::UNAVAILABLE
    );
    assert_eq!(
        read.calls.get(),
        before,
        "retired identities never reach the presenter"
    );
}

#[derive(Debug)]
struct RangeRead {
    calls: Cell<usize>,
    answer: Cell<GeometryAnswer>,
}
impl InlineGeometry for RangeRead {
    fn frame(&self, _: ViewId) -> GeometryAnswer {
        panic!("a field range is not an inline run frame")
    }
    fn range_frame(&self, view: ViewId, start: u32, end: u32) -> GeometryAnswer {
        assert_eq!((view, start, end), (2, 2, 5));
        self.calls.set(self.calls.get() + 1);
        self.answer.get()
    }
}

#[test]
fn field_ranges_read_the_presented_viewport_only_while_the_field_is_live() {
    let mut kernel = Kernel::with_monospace();
    kernel
        .apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::TextInput,
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2],
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    let answer = GeometryAnswer {
        x: 32.,
        y: -14.,
        width: 50.,
        height: 40.,
        ..Default::default()
    };
    let reader = Rc::new(RangeRead {
        calls: Cell::new(0),
        answer: Cell::new(answer),
    });
    let mut scrolled = Scrolled {
        inline: Some(reader.clone()),
        ..Default::default()
    };
    assert_eq!(
        kernel_range_frame(&kernel, &scrolled, 2, 2., 5.),
        GeometryAnswer::UNAVAILABLE
    );
    assert_eq!(reader.calls.get(), 0);
    kernel
        .compute_layout(1, Offer::definite(200., 200.))
        .unwrap();
    scrolled.note(&kernel, None, 0., 100.);
    assert_eq!(kernel_range_frame(&kernel, &scrolled, 2, 2.8, 5.9), answer);
    assert_eq!(reader.calls.get(), 1);
    for (view, start, end) in [(1, 2., 5.), (99, 2., 5.), (2, f64::NAN, 5.), (2, 5., 2.)] {
        assert_eq!(
            kernel_range_frame(&kernel, &scrolled, view, start, end),
            GeometryAnswer::UNAVAILABLE
        );
    }
    assert_eq!(
        reader.calls.get(),
        1,
        "refused reads do not reach the presenter"
    );
    reader.answer.set(GeometryAnswer {
        width: -1.,
        ..answer
    });
    assert_eq!(
        kernel_range_frame(&kernel, &scrolled, 2, 2., 5.),
        GeometryAnswer::UNAVAILABLE
    );
    kernel
        .apply(
            0,
            2,
            &[
                Op::SetChildren {
                    id: 1,
                    children: vec![],
                },
                Op::DestroyView { id: 2 },
            ],
        )
        .unwrap();
    assert_eq!(
        kernel_range_frame(&kernel, &scrolled, 2, 2., 5.),
        GeometryAnswer::UNAVAILABLE
    );
    assert_eq!(reader.calls.get(), 2);
}
