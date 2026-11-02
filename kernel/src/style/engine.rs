//! Values lowered into the layout engine.
use super::StyleProps;
use crate::generated::{GridAutoFlow, Overflow};

pub(super) fn rare(style: &StyleProps, env: &crate::Env) -> taffy::style::Rare {
    taffy::style::RareStyle {
        grid_template_columns: style.rare.grid_template_columns.taffy_components(env),
        grid_template_column_names: style.rare.grid_template_columns.line_names(),
        grid_template_rows: style.rare.grid_template_rows.taffy_components(env),
        grid_template_row_names: style.rare.grid_template_rows.line_names(),
        grid_column: style.rare.grid_column.taffy(),
        grid_row: style.rare.grid_row.taffy(),
        ..taffy::style::RareStyle::DEFAULT
    }
    .into()
}

pub(super) fn overflow(v: Overflow) -> taffy::style::Overflow {
    match v {
        Overflow::Visible => taffy::style::Overflow::Visible,
        Overflow::Hidden => taffy::style::Overflow::Hidden,
        Overflow::Clip => taffy::style::Overflow::Clip,
        Overflow::Scroll | Overflow::Auto => taffy::style::Overflow::Scroll,
    }
}

pub(super) fn grid_auto_flow(v: GridAutoFlow) -> taffy::style::GridAutoFlow {
    match v {
        GridAutoFlow::Row => taffy::style::GridAutoFlow::Row,
        GridAutoFlow::Column => taffy::style::GridAutoFlow::Column,
        GridAutoFlow::Dense | GridAutoFlow::RowDense => taffy::style::GridAutoFlow::RowDense,
        GridAutoFlow::ColumnDense => taffy::style::GridAutoFlow::ColumnDense,
    }
}
