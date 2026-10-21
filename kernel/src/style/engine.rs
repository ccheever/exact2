//! Values lowered into the layout engine.
use super::StyleProps;

pub(super) fn rare(style: &StyleProps) -> taffy::style::Rare {
    taffy::style::RareStyle {
        grid_template_columns: style.rare.grid_template_columns.taffy_components(),
        grid_template_column_names: style.rare.grid_template_columns.line_names(),
        grid_template_rows: style.rare.grid_template_rows.taffy_components(),
        grid_template_row_names: style.rare.grid_template_rows.line_names(),
        grid_column: style.rare.grid_column.taffy(),
        grid_row: style.rare.grid_row.taffy(),
        ..taffy::style::RareStyle::DEFAULT
    }
    .into()
}
