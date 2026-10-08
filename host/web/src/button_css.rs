/// Face layout metadata beside the authored CSS; reset on each native button so
/// a parent's gap/direction/alignment never configures its semantic children.
pub(super) fn rows(
    style: &exact_kernel::StyleProps,
    out: &mut String,
    authored: &dyn Fn(&mut String, exact_kernel::StyleId) -> bool,
) {
    use exact_kernel::{FlexDirection, StyleId, TextAlign};
    for (id, value, gap, space) in [
        (
            StyleId::ColumnGap,
            style.column_gap,
            "--exact-button-column-gap",
            "--exact-button-column-space",
        ),
        (
            StyleId::RowGap,
            style.row_gap,
            "--exact-button-row-gap",
            "--exact-button-row-space",
        ),
    ] {
        if style.mask.has(id) {
            out.push_str(gap);
            out.push(':');
            if !authored(out, id) {
                out.push_str(&format!("{value}px"));
            }
            out.push(';');
            out.push_str(space);
            out.push_str(":\"\";");
        }
    }
    if style.mask.has(StyleId::FlexDirection) {
        let column = matches!(
            style.flex_direction,
            FlexDirection::Column | FlexDirection::ColumnReverse
        );
        out.push_str(if column {
            "--exact-button-leading-subtitle-areas:\"image\" \"space\" \"title\" \"subtitle\";--exact-button-trailing-subtitle-areas:\"title\" \"subtitle\" \"space\" \"image\";--exact-button-space-width:0px;--exact-button-space-height:var(--exact-button-row-gap,0px);--exact-button-columns:minmax(0,auto);--exact-button-space:var(--exact-button-row-space);"
        } else {
            "--exact-button-leading-subtitle-areas:\"image space title\" \"image space subtitle\";--exact-button-trailing-subtitle-areas:\"title space image\" \"subtitle space image\";--exact-button-space-width:var(--exact-button-column-gap,auto);--exact-button-space-height:0px;--exact-button-columns:auto auto minmax(0,auto);--exact-button-space:var(--exact-button-column-space);"
        });
    }
    if style.mask.has(StyleId::FlexDirection) {
        out.push_str(if matches!(style.flex_direction, FlexDirection::Column | FlexDirection::ColumnReverse) {
            "--exact-button-leading-areas:\"image\" \"space\" \"title\";--exact-button-trailing-areas:\"title\" \"space\" \"image\";"
        } else {
            "--exact-button-leading-areas:\"image space title\";--exact-button-trailing-areas:\"title space image\";"
        });
    }
    if style.mask.has(StyleId::TextAlign) {
        let align = match style.text_align {
            TextAlign::Left | TextAlign::Start => "start",
            TextAlign::Right | TextAlign::End => "end",
            _ => "center",
        };
        out.push_str("--exact-button-align:");
        out.push_str(align);
        out.push(';');
    }
}
