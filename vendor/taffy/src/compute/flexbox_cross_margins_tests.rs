use crate::geometry::Rect;
use crate::prelude::*;
use crate::style::Direction;

// Chrome 151.0.7922.34, 2026-10-18: auto margins consume positive free space
// and remain zero on overflow, including RTL columns whose position is negative.
#[test]
fn cross_axis_auto_margins_never_consume_negative_space() {
    let mut failures = Vec::new();
    for (flex_direction, direction) in [
        (FlexDirection::Column, Direction::Ltr),
        (FlexDirection::Column, Direction::Rtl),
        (FlexDirection::ColumnReverse, Direction::Ltr),
        (FlexDirection::ColumnReverse, Direction::Rtl),
        (FlexDirection::Row, Direction::Ltr),
        (FlexDirection::RowReverse, Direction::Ltr),
    ] {
        let column = flex_direction.is_column();
        for cross_size in [20.0, 50.0, 80.0] {
            for (start_auto, end_auto) in [(true, false), (false, true), (true, true)] {
                let mut tree = TaffyTree::<()>::new();
                let start = if start_auto { auto() } else { length(0.0) };
                let end = if end_auto { auto() } else { length(0.0) };
                let item = tree
                    .new_leaf(Style {
                        size: if column {
                            Size {
                                width: length(cross_size),
                                height: length(80.0),
                            }
                        } else {
                            Size {
                                width: length(80.0),
                                height: length(cross_size),
                            }
                        },
                        margin: if column {
                            Rect {
                                left: start,
                                right: end,
                                ..zero()
                            }
                        } else {
                            Rect {
                                top: start,
                                bottom: end,
                                ..zero()
                            }
                        },
                        ..Default::default()
                    })
                    .unwrap();
                let container = tree
                    .new_with_children(
                        Style {
                            flex_direction,
                            direction,
                            size: if column {
                                Size {
                                    width: length(50.0),
                                    height: length(100.0),
                                }
                            } else {
                                Size {
                                    width: length(100.0),
                                    height: length(50.0),
                                }
                            },
                            ..Default::default()
                        },
                        &[item],
                    )
                    .unwrap();
                tree.compute_layout(container, Size::MAX_CONTENT).unwrap();
                let margin = tree.layout(item).unwrap().margin;
                let got = if column {
                    [margin.left, margin.right]
                } else {
                    [margin.top, margin.bottom]
                };
                let share = match cross_size {
                    20.0 if start_auto && end_auto => 15.0,
                    20.0 => 30.0,
                    _ => 0.0,
                };
                let expected = [
                    if start_auto { share } else { 0.0 },
                    if end_auto { share } else { 0.0 },
                ];
                if got != expected {
                    failures.push(format!(
                        "{flex_direction:?} {direction:?}, cross {cross_size}, auto {start_auto}/{end_auto}: \
                         margins {got:?}, expected {expected:?}"
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
