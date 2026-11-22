//! The columns of a details table and their widths. Name takes the room the
//! others leave, so the last column always ends at the table's right edge, as
//! in Nautilus. Every other column has a width in pixels; dragging a border
//! sizes the column to its right, Name giving or taking the difference, and
//! the table scrolls sideways when the columns are wider than the pane.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Column {
    Name,
    Size,
    Type,
    Modified,
    /// Where a trashed item came from.
    Location,
    /// When an item went to Trash.
    Deleted,
}

impl Column {
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Size => "Size",
            Self::Type => "Type",
            Self::Modified => "Modified",
            Self::Location => "Original location",
            Self::Deleted => "Deleted",
        }
    }

    /// Sizes line up on the right, as numbers do.
    pub fn right_aligned(self) -> bool {
        self == Self::Size
    }

    fn default_width(self) -> f32 {
        match self {
            Self::Name => 320.,
            Self::Size => 96.,
            Self::Type => 160.,
            Self::Modified | Self::Deleted => 176.,
            Self::Location => 240.,
        }
    }

    fn min_width(self) -> f32 {
        if self == Self::Name { MIN_NAME } else { MIN_COLUMN }
    }
}

const MIN_COLUMN: f32 = 48.;
const MIN_NAME: f32 = 120.;

/// A table of the window: the folder lists, and Trash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    Files,
    Trash,
}

impl Table {
    /// The columns this table can show after Name, in Thunar's order.
    pub fn available(self) -> &'static [Column] {
        match self {
            Self::Files => &[Column::Size, Column::Type, Column::Modified],
            Self::Trash => &[Column::Location, Column::Deleted, Column::Size, Column::Type],
        }
    }

    fn shown_by_default(self) -> Vec<Column> {
        match self {
            Self::Files => vec![Column::Size, Column::Type, Column::Modified],
            // Trash items carry no size.
            Self::Trash => vec![Column::Location, Column::Deleted, Column::Type],
        }
    }
}

/// Which columns a table shows after Name, and the widths dragged by hand.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColumnLayout {
    pub shown: Vec<Column>,
    /// Columns sized by hand; others keep their default width. Name fills.
    pub widths: HashMap<Column, f32>,
}

impl ColumnLayout {
    pub fn new(table: Table) -> Self {
        Self { shown: table.shown_by_default(), widths: HashMap::new() }
    }

    /// Name, then the shown columns this table has, each once.
    pub fn columns(&self, table: Table) -> Vec<Column> {
        let mut columns = vec![Column::Name];
        for column in &self.shown {
            if table.available().contains(column) && !columns.contains(column) {
                columns.push(*column);
            }
        }
        columns
    }

    /// How wide each column is drawn, Name first.
    pub fn specs(&self, table: Table) -> Vec<ColumnSpec> {
        self.columns(table)
            .into_iter()
            .map(|column| ColumnSpec { column, width: if column == Column::Name { Width::Fill(MIN_NAME) } else { Width::Fixed(self.width(column)) } })
            .collect()
    }

    fn width(&self, column: Column) -> f32 {
        self.widths.get(&column).copied().filter(|width| width.is_finite()).unwrap_or(column.default_width()).max(column.min_width())
    }

    /// The layout with the border right of `column` dragged to `x` pixels from
    /// the table's left edge, in a table drawn `table_width` pixels wide. The
    /// columns right of the border keep their place against the right edge, so
    /// the one just right of it takes the move, down to its minimum; Name keeps
    /// its own.
    pub fn resized(&self, table: Table, column: Column, x: f32, table_width: f32) -> Self {
        let mut layout = self.clone();
        layout.widths.remove(&Column::Name);
        let columns = self.columns(table);
        let Some(border) = columns.iter().position(|&shown| shown == column) else { return layout };
        let Some(&sized) = columns.get(border + 1) else { return layout };
        let after: f32 = columns[border + 2..].iter().map(|&column| self.width(column)).sum();
        let before: f32 = columns[1..=border].iter().map(|&column| self.width(column)).sum();
        let right = table_width.max(self.min_width(table));
        let x = x.max(MIN_NAME + before);
        layout.widths.insert(sized, (right - after - x).max(sized.min_width()).round());
        layout
    }

    /// The layout with `column` back at its default width.
    pub fn reset_width(&self, column: Column) -> Self {
        let mut layout = self.clone();
        layout.widths.remove(&column);
        layout
    }

    /// The layout with `column` shown or hidden, in the table's order.
    pub fn toggled(&self, table: Table, column: Column) -> Self {
        let mut layout = self.clone();
        match layout.shown.contains(&column) {
            true => layout.shown.retain(|shown| *shown != column),
            false => {
                layout.shown.push(column);
                layout.shown.sort_by_key(|shown| table.available().iter().position(|available| available == shown));
            }
        }
        layout
    }

    /// The narrowest the table is drawn before it scrolls sideways.
    pub fn min_width(&self, table: Table) -> f32 {
        MIN_NAME + self.columns(table).into_iter().skip(1).map(|column| self.width(column)).sum::<f32>()
    }
}

/// A column's width as drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Width {
    Fixed(f32),
    /// Takes the room the others leave, at least this wide.
    Fill(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnSpec {
    pub column: Column,
    pub width: Width,
}

/// A layout from the settings file, or a table's default when it cannot be
/// read, so a bad entry does not lose the other settings.
pub fn lenient<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<ColumnLayout>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_fills_and_the_last_column_ends_at_the_edge() {
        let specs = ColumnLayout::new(Table::Files).specs(Table::Files);
        assert_eq!(specs.iter().map(|spec| spec.column).collect::<Vec<_>>(), [Column::Name, Column::Size, Column::Type, Column::Modified]);
        assert_eq!(specs[0].width, Width::Fill(MIN_NAME));
        assert_eq!(specs[3].width, Width::Fixed(176.));
    }

    #[test]
    fn a_border_sizes_the_column_to_its_right() {
        // 1000px: Modified spans 824..1000, Type 664..824, Size 568..664.
        let layout = ColumnLayout::new(Table::Files);
        let wider = layout.resized(Table::Files, Column::Type, 800., 1000.);
        assert_eq!(wider.widths.get(&Column::Modified), Some(&200.));
        assert_eq!(wider.specs(Table::Files)[2].width, Width::Fixed(160.), "Type keeps its width and moves");
        // Name's border sizes Size.
        let name = layout.resized(Table::Files, Column::Name, 500., 1000.);
        assert_eq!(name.widths.get(&Column::Size), Some(&164.));
        // A width saved for Name from before is dropped.
        let mut saved = layout.clone();
        saved.widths.insert(Column::Name, 1214.);
        assert!(!saved.resized(Table::Files, Column::Type, 800., 1000.).widths.contains_key(&Column::Name));
    }

    #[test]
    fn columns_keep_a_minimum_and_name_its_own() {
        let layout = ColumnLayout::new(Table::Files);
        assert_eq!(layout.resized(Table::Files, Column::Type, 2000., 1000.).widths.get(&Column::Modified), Some(&MIN_COLUMN));
        // Name stops at its minimum; Size takes no more than that leaves.
        assert_eq!(layout.resized(Table::Files, Column::Name, 0., 1000.).widths.get(&Column::Size), Some(&(1000. - 176. - 160. - MIN_NAME)));
        assert_eq!(layout.min_width(Table::Files), MIN_NAME + 96. + 160. + 176.);
        assert_eq!(layout.resized(Table::Files, Column::Modified, 900., 1000.), layout, "the last border is the table's edge");
    }

    #[test]
    fn hiding_and_showing_keeps_the_tables_order() {
        let layout = ColumnLayout::new(Table::Files).toggled(Table::Files, Column::Size);
        assert_eq!(layout.columns(Table::Files), [Column::Name, Column::Type, Column::Modified]);
        let layout = layout.toggled(Table::Files, Column::Size);
        assert_eq!(layout.columns(Table::Files), [Column::Name, Column::Size, Column::Type, Column::Modified]);
    }

    #[test]
    fn a_column_another_table_owns_is_not_drawn() {
        let layout = ColumnLayout { shown: vec![Column::Deleted, Column::Size], widths: HashMap::new() };
        assert_eq!(layout.columns(Table::Files), [Column::Name, Column::Size]);
    }

    #[test]
    fn an_unreadable_layout_falls_back() {
        #[derive(Deserialize)]
        struct Settings {
            #[serde(default, deserialize_with = "lenient")]
            columns: Option<ColumnLayout>,
            other: bool,
        }
        let settings: Settings = serde_json::from_str(r#"{"columns": [0.2, 0.1, 0.3], "other": true}"#).unwrap();
        assert!(settings.columns.is_none() && settings.other);
    }
}
