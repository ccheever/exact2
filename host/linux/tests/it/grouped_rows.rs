//! LLP 1084 D7: a grouped list's row spans its card with its insets inside
//! it, and the separator under it is decoration the kernel places
//! (`Kernel::grouped_separator`): drawn over the row's ground and under its
//! children, from its content edge to the card's trailing edge, not under
//! the card's last row, and not where the author wrote that edge, whose own
//! border is drawn instead.
use exact_kernel::{ColorValue, ViewId};
use exact_linux::presenter::PainterChoice;
use exact_linux::Presenter;

const APP: &str = r##"component App
  view
    column width=402 height=600 background-color="#f2f2f7"
      list appearance="auto" flex=1
        section
          row width="100%" testId="full"
            text "Full width" flex=1
            text "42"
          row testId="covered"
            view width="100%" height=52 background-color="#00ff00" testId="cover"
          row border-bottom-width=2 border-bottom-style="solid" border-bottom-color="#ff0000" testId="own"
            text "Own edge"
          row testId="last"
            text "Last"
"##;

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

/// The pixel `(x, y)` into `name`'s box.
fn pixel(p: &mut Presenter<()>, name: &str, x: f32, y: f32) -> [u8; 3] {
    let (left, top, _, _) = p.rect_of(id(p, name)).unwrap();
    let shot = p.frame();
    let c = shot.pixel((left + x) as u32, (top + y) as u32).unwrap();
    [c.red(), c.green(), c.blue()]
}

#[test]
fn a_grouped_rows_separator_is_decoration_under_its_children() {
    let plan = contract::compile(APP).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (402., 600.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let k = p.host().kernel();
    let line = k.grouped_separator(id(&p, "full")).unwrap();
    assert!(matches!(line.color, ColorValue::Role(_)), "the platform's");
    let (_, _, width, height) = p.rect_of(id(&p, "full")).unwrap();
    assert_eq!(width, 370.0, "a full-width row is its card's width");
    let card = pixel(&mut p, "full", 4.0, height / 2.0);
    let foot = height - 1.0;
    assert_eq!(pixel(&mut p, "full", 8.0, foot), card, "not in the inset");
    // The separator's translucent ink over the card.
    let ink = pixel(&mut p, "full", 20.0, foot);
    assert_ne!(ink, card, "from the content edge");
    assert_eq!(
        pixel(&mut p, "full", 20.0, foot - 1.0),
        card,
        "one point high"
    );
    assert_eq!(
        pixel(&mut p, "full", width - 1.0, foot),
        ink,
        "to the card's trailing edge"
    );
    assert_eq!(
        pixel(&mut p, "covered", 200.0, foot),
        [0, 255, 0],
        "under the row's children"
    );
    let (_, _, _, own) = p.rect_of(id(&p, "own")).unwrap();
    assert_eq!(
        pixel(&mut p, "own", 200.0, own - 1.0),
        [255, 0, 0],
        "the author's edge"
    );
    assert_eq!(k_none(&p, "own"), None);
    let (_, _, _, last) = p.rect_of(id(&p, "last")).unwrap();
    assert_eq!(
        pixel(&mut p, "last", 200.0, last - 1.0),
        card,
        "none under the last row"
    );
}

fn k_none(p: &Presenter<()>, name: &str) -> Option<exact_kernel::GroupedSeparator> {
    p.host().kernel().grouped_separator(id(p, name))
}
