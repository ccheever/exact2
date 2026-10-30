//! LLP 1115.002 D8.2: a native container gives a subtree a containing
//! block (`Host::set_blocks`), and the subtree's frames come back in that
//! block's space with the block's used size. One page, two panes and a
//! sheet: a pane that changes sends nothing for the other pane, the sheet
//! or the page, and a sheet's content that grows sends its new used size.

use crate::host::{count, frame_of, view, NoData};
use exact_apple::Host;
use exact_kernel::{BlockAxis, HostBlock, MonospaceMeasurer};
use exact_runner::Event;

/// The page the Mac's proof uses too (`HostBlockMacTests.swift`).
const PAGE: &str = r##"component Proof
  state rows = [0, 1]
  action more
    rows = rows.concat([rows.length])
  view
    column testId="page" width="100%" height="100%"
      text "Behind the panes" testId="title"
      box testId="stack" position="relative" flex=1 min-height=0
        column testId="list" position="absolute" inset=0
          input testId="field" value="draft"
          scroll testId="scroller" flex=1 min-height=0
            box testId="tall" height=1200
        column testId="detail" position="absolute" inset=0
          box testId="half" width="50%" height="25%"
          box testId="badge" position="absolute" right=10 bottom=10 width=40 height=40
        column testId="sheet" position="absolute" inset=0 padding=16
          row testId="form" flex-wrap="wrap" gap=8
            box width=120 height=30
            box width=160 height=30
            box width=120 height=30
          box press=more testId="more" height=44
            text "More"
          for i in rows key=i
            row height=44
              text `Row ${i}`
      text "Footer" testId="footer"
"##;

fn is(width: f32, height: f32) -> Option<HostBlock> {
    Some(HostBlock {
        width: BlockAxis::Is(width),
        height: BlockAxis::Is(height),
    })
}

/// The `block` op for `id`: its used size.
fn used(batch: &str, id: u32) -> Option<(f32, f32)> {
    let marker = format!("\"op\":\"block\",\"id\":{id},\"w\":");
    let rest = &batch[batch.rfind(&marker)? + marker.len()..];
    let (w, rest) = rest.split_once(",\"h\":")?;
    Some((w.parse().ok()?, rest[..rest.find(',')?].parse().ok()?))
}

/// The ids the batch's `frame` ops name.
fn framed(batch: &str) -> Vec<u32> {
    let marker = "\"op\":\"frame\",\"id\":";
    let ids = batch.match_indices(marker).map(|(at, _)| {
        let rest = &batch[at + marker.len()..];
        rest[..rest.find(',').unwrap()].parse().unwrap()
    });
    ids.collect()
}

/// Whether `node` is `root` or under it.
fn under(host: &Host<NoData>, root: u32, node: u32) -> bool {
    let kernel = host.runner().kernel();
    let mut at = Some(node);
    while let Some(n) = at.filter(|n| *n != root) {
        at = kernel.node(n).and_then(|n| n.parent);
    }
    at == Some(root)
}

#[test]
fn two_panes_and_a_sheet_of_one_page_lay_out_each_in_its_block() {
    let plan = contract::compile(PAGE).unwrap();
    let measurer = Box::new(MonospaceMeasurer::default());
    let (mut host, _) = Host::boot(&plan.encode(), NoData, measurer, 800.0, 600.0).unwrap();
    let [list, detail, sheet] = ["list", "detail", "sheet"].map(|name| view(&host, name));
    let [half, badge, scroller, more] =
        ["half", "badge", "scroller", "more"].map(|name| view(&host, name));
    let page = ["page", "title", "stack", "footer"].map(|name| view(&host, name));
    let stack = host.runner().kernel().node(page[2]).unwrap().frame;
    let maximum = Some(HostBlock {
        width: BlockAxis::AtMost(480.0),
        height: BlockAxis::AtMost(400.0),
    });
    let shown = host.set_blocks(&[
        (list, is(280.0, 560.0)),
        (detail, is(520.0, 560.0)),
        (sheet, maximum),
    ]);
    assert!(shown.contains("\"error\":null"), "{shown}");
    // Each root at its block's corner, a pane the block's size.
    assert_eq!(frame_of(&shown, list), (0.0, 0.0, 280.0, 560.0));
    assert_eq!(frame_of(&shown, detail), (0.0, 0.0, 520.0, 560.0));
    assert_eq!(used(&shown, list), Some((280.0, 560.0)));
    assert_eq!(used(&shown, detail), Some((520.0, 560.0)));
    // A percentage and an absolutely positioned child resolve against it.
    assert_eq!(frame_of(&shown, half), (0.0, 0.0, 260.0, 140.0));
    assert_eq!(frame_of(&shown, badge), (470.0, 510.0, 40.0, 40.0));
    // The sheet is its content's size: the form's three boxes on one line
    // inside the padding, then the button and two rows.
    let (w, h) = used(&shown, sheet).unwrap();
    let line = 120.0 + 8.0 + 160.0 + 8.0 + 120.0;
    assert_eq!(
        (w, h),
        (16.0 + line + 16.0, 16.0 + 30.0 + 44.0 + 88.0 + 16.0)
    );
    assert_eq!(frame_of(&shown, sheet), (0.0, 0.0, w, h));
    // The page behind is laid out as it was: nothing of it is sent.
    assert!(
        framed(&shown).iter().all(|id| !page.contains(id)),
        "{shown}"
    );
    assert_eq!(host.runner().kernel().node(page[2]).unwrap().frame, stack);

    // The divider moves: both panes, and nothing of the sheet or the page.
    let moved = host.set_blocks(&[(list, is(200.0, 560.0)), (detail, is(600.0, 560.0))]);
    assert_eq!(frame_of(&moved, list).2, 200.0);
    assert_eq!(frame_of(&moved, half), (0.0, 0.0, 300.0, 140.0));
    assert_eq!(frame_of(&moved, badge), (550.0, 510.0, 40.0, 40.0));
    assert_eq!(frame_of(&moved, scroller).2, 200.0);
    let panes = |n: &u32| under(&host, list, *n) || under(&host, detail, *n);
    let stray: Vec<u32> = framed(&moved).into_iter().filter(|n| !panes(n)).collect();
    assert!(
        stray.is_empty() && used(&moved, sheet).is_none(),
        "{stray:?} {moved}"
    );

    // The sheet's content grows by a row: its used size, and no pane.
    let grown = host.dispatch_at(more, Event::Press, 0.0);
    assert_eq!(used(&grown, sheet), Some((w, h + 44.0)));
    assert!(
        framed(&grown).iter().all(|n| under(&host, sheet, *n)),
        "{grown}"
    );

    // The same sizes again say nothing; the sheet's block taken back puts
    // the route in its parent again, and its block op has no size.
    let same = host.set_blocks(&[(list, is(200.0, 560.0)), (detail, is(600.0, 560.0))]);
    assert_eq!(
        (count(&same, "frame"), count(&same, "block")),
        (0, 0),
        "{same}"
    );
    let back = host.set_blocks(&[(sheet, None)]);
    assert_eq!(
        frame_of(&back, sheet),
        (0.0, 0.0, stack.width, stack.height)
    );
    assert_eq!(count(&back, "block"), 1, "{back}");
    assert_eq!(used(&back, sheet), None, "{back}");
    // A view that is gone is the batch's error, and the others are set.
    let gone = host.set_blocks(&[(9_999, is(10.0, 10.0)), (list, is(240.0, 560.0))]);
    assert!(
        gone.contains("\"error\":\"block: ") && frame_of(&gone, list).2 == 240.0,
        "{gone}"
    );
}

/// Stage 14 B: a popover is laid out in the viewport, a block the kernel
/// implies, whatever box it was written in. Its frame comes in that
/// block's space, and its `block` op with the block's used size and where
/// the popover sits in it (its margins): what the host reports it by.
#[test]
fn a_popover_comes_in_the_viewports_block() {
    let plan = contract::compile(
        r##"component Pins
  view
    column testId="page" width="100%" height="100%"
      box testId="header" position="relative" width=100 height=40
        column testId="pins" popover="manual" width="50%" margin=8
          box height=30
"##,
    )
    .unwrap();
    let measurer = Box::new(MonospaceMeasurer::default());
    let (host, batch) = Host::boot(&plan.encode(), NoData, measurer, 800.0, 600.0).unwrap();
    let pins = view(&host, "pins");
    assert_eq!(frame_of(&batch, pins), (8.0, 8.0, 400.0, 30.0));
    let marker = format!("\"op\":\"block\",\"id\":{pins},");
    assert!(
        batch.contains(&format!("{marker}\"w\":416,\"h\":46,\"x\":8,\"y\":8}}")),
        "{batch}"
    );
}

/// A popover leaving the top layer (its author makes it sticky) leaves its
/// block: a `block` op with no size, so the host forgets the rectangle it
/// reported the popover by.
#[test]
fn a_popover_leaving_its_block_is_sent_a_block_op_with_no_size() {
    let plan = contract::compile(
        r##"component Pins
  state inflow = false
  action toggle
    inflow = ! inflow
  view
    column testId="page" width="100%" height="100%"
      button testId="toggle" press=toggle
        text "Toggle"
      column testId="pins" popover="manual" position=(inflow ? "sticky" : "absolute") width="50%" height=30
"##,
    )
    .unwrap();
    let measurer = Box::new(MonospaceMeasurer::default());
    let (mut host, batch) = Host::boot(&plan.encode(), NoData, measurer, 800.0, 600.0).unwrap();
    let pins = view(&host, "pins");
    assert_eq!(used(&batch, pins), Some((400.0, 30.0)));
    let left = host.dispatch_at(view(&host, "toggle"), Event::Press, 0.0);
    assert!(
        left.contains(&format!("{{\"op\":\"block\",\"id\":{pins}}}")),
        "{left}"
    );
    assert!(host.runner().kernel().host_block(pins).is_none());
    let back = host.dispatch_at(view(&host, "toggle"), Event::Press, 10.0);
    assert_eq!(used(&back, pins), Some((400.0, 30.0)), "{back}");
}
