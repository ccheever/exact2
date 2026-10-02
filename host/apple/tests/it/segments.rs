//! The viewport segments a fold makes (LLP 1076 D4): what `exact_segments`
//! tells the host, and what the host sends back.

use crate::host::{boot, count, frame_of, view, NoData};
use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;

/// LLP 1076 D4: `exact_segments` sets the kernel's segment grid and the
/// runner's fold in one batch — the styles that read a segment re-sent and
/// laid out again, nothing for the same grid, a malformed one refused.
#[test]
fn the_segments_re_send_the_styles_that_read_them_and_place_the_panes() {
    let src = "component App\n  view\n    column testId=\"root\" height=\"100%\"\n      column testId=\"list\" width=\"env(viewport-segment-width 0 0)\" height=40\n      column testId=\"detail\" position=\"absolute\" top=0 left=\"env(viewport-segment-left 1 0)\" width=\"env(viewport-segment-width 1 0)\" height=40\n      column testId=\"plain\" height=20 padding-left=\"env(viewport-segment-width 0 0)\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        951.0,
        669.0,
    )
    .unwrap();
    let (list, detail, plain) = (
        view(&host, "list"),
        view(&host, "detail"),
        view(&host, "plain"),
    );
    // Flat: the segment lengths take their initial values (auto, 0).
    assert_eq!(frame_of(&first, list), (0.0, 0.0, 951.0, 40.0));
    assert!(first.contains("\"padding_left\":0"), "{}", &first[..600]);
    let duo = vec![
        exact_kernel::Rect::new(0.0, 0.0, 455.5, 669.0),
        exact_kernel::Rect::new(495.5, 0.0, 455.5, 669.0),
    ];
    let batch = host.set_segments(1, 2, 1, duo.clone());
    assert!(batch.contains("\"error\":null"), "{batch}");
    // Width and left are frame-carried rows the presenter never receives;
    // the padding that reads a segment is re-sent with its points.
    assert_eq!(count(&batch, "style"), 1, "the padded dictionary: {batch}");
    assert!(
        batch.contains(&format!("\"op\":\"style\",\"id\":{plain},"))
            && batch.contains("\"padding_left\":455.5"),
        "{batch}"
    );
    assert_eq!(frame_of(&batch, list), (0.0, 0.0, 455.5, 40.0));
    assert_eq!(frame_of(&batch, detail), (495.5, 0.0, 455.5, 40.0));
    // The same grid again: nothing to say.
    let again = host.set_segments(1, 2, 1, duo);
    assert_eq!(
        count(&again, "style") + count(&again, "frame"),
        0,
        "{again}"
    );
    // A malformed grid is refused by the kernel, as an error on the batch.
    let bad = host.set_segments(1, 2, 1, vec![]);
    assert!(bad.contains("\"error\":\"segments: "), "{bad}");
    let past = host.set_segments(1, 300, 1, vec![]);
    assert!(past.contains("\"error\":\"segments: "), "{past}");
    // Flat again: the initial values return.
    let flat = host.set_segments(0, 1, 1, vec![]);
    assert_eq!(count(&flat, "style"), 1, "{flat}");
    assert!(flat.contains("\"padding_left\":0"), "{flat}");
    assert_eq!(frame_of(&flat, list), (0.0, 0.0, 951.0, 40.0));
    // An app that reads no segment (Caltrain) gets an empty batch.
    let (mut caltrain, _) = boot();
    let none = caltrain.set_segments(
        1,
        2,
        1,
        vec![
            exact_kernel::Rect::new(0.0, 0.0, 100.0, 844.0),
            exact_kernel::Rect::new(290.0, 0.0, 100.0, 844.0),
        ],
    );
    assert_eq!(count(&none, "style") + count(&none, "frame"), 0, "{none}");
}
