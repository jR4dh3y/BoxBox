//! The columns of a details table and their widths, as Thunar's details view
//! keeps them: each column has a width in pixels, dragging a border sizes the
//! column to its left and moves the ones after it, and the table scrolls
//! sideways when the columns are wider than the pane. Name fills the room left
//! until it is sized by hand; then the last column does.

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
    /// Columns sized by hand; others keep their default width, and Name fills.
    pub widths: HashMap<Column, f32>,
    /// How wide the pane was when a column was last sized. In a narrower pane
    /// Name gives up the difference, so the Details panel or a split moves the
    /// columns over instead of hiding them; widening a column in the same pane
    /// still scrolls the table, so the border follows the pointer.
    #[serde(default)]
    pub fitted: Option<f32>,
}

impl ColumnLayout {
    pub fn new(table: Table) -> Self {
        Self { shown: table.shown_by_default(), widths: HashMap::new(), fitted: None }
    }

    /// The layout as drawn in a pane `pane` pixels wide: a sized Name narrower
    /// by the width the pane lost since it was sized, down to its minimum.
    pub fn fit(&self, pane: f32) -> Self {
        let mut layout = self.clone();
        if let (Some(name), Some(fitted)) = (self.widths.get(&Column::Name), self.fitted) {
            let lost = (fitted - pane).max(0.);
            layout.widths.insert(Column::Name, (name - lost).max(MIN_NAME).round());
        }
        layout.fitted = Some(pane);
        layout
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
        let columns = self.columns(table);
        let last = columns.len() - 1;
        let name_sized = self.widths.contains_key(&Column::Name);
        columns
            .iter()
            .enumerate()
            .map(|(index, &column)| {
                let width = self.width(column);
                let width = match (column, index == last) {
                    (Column::Name, _) if !name_sized => Width::Fill(MIN_NAME),
                    // Only Name: it is the whole row.
                    (Column::Name, true) => Width::Fill(width),
                    (_, true) if name_sized => Width::Fill(width),
                    _ => Width::Fixed(width),
                };
                ColumnSpec { column, width }
            })
            .collect()
    }

    fn width(&self, column: Column) -> f32 {
        self.widths.get(&column).copied().filter(|width| width.is_finite()).unwrap_or(column.default_width()).max(column.min_width())
    }

    /// The layout with `column`'s right border dragged to `x` pixels from the
    /// table's left edge, in a table drawn `table_width` pixels wide.
    pub fn resized(&self, table: Table, column: Column, x: f32, table_width: f32) -> Self {
        let mut layout = self.clone();
        layout.fitted = Some(self.fitted.unwrap_or(table_width));
        // Sizing another column pins Name where it is drawn, as GTK pins every
        // column once one is sized: otherwise a wider column would narrow Name
        // and slide away from the pointer. The last column fills from then on.
        if column != Column::Name && !layout.widths.contains_key(&Column::Name) {
            let fixed: f32 = self.specs(table).iter().filter_map(|spec| spec.fixed()).sum();
            layout.widths.insert(Column::Name, (table_width - fixed).max(MIN_NAME).round());
        }
        let left: f32 = layout
            .specs(table)
            .iter()
            .take_while(|spec| spec.column != column)
            .map(|spec| match spec.width {
                Width::Fixed(width) | Width::Fill(width) => width,
            })
            .sum();
        layout.widths.insert(column, (x - left).max(column.min_width()).round());
        layout
    }

    /// The layout with `column` back at its default width; Name fills again.
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
        self.specs(table).iter().map(|spec| spec.fixed().unwrap_or(match spec.width {
            Width::Fill(min) => min,
            Width::Fixed(width) => width,
        })).sum()
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

impl ColumnSpec {
    fn fixed(&self) -> Option<f32> {
        match self.width {
            Width::Fixed(width) => Some(width),
            Width::Fill(_) => None,
        }
    }
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
    fn name_fills_until_it_is_sized() {
        let layout = ColumnLayout::new(Table::Files);
        let specs = layout.specs(Table::Files);
        assert_eq!(specs.iter().map(|spec| spec.column).collect::<Vec<_>>(), [Column::Name, Column::Size, Column::Type, Column::Modified]);
        assert_eq!(specs[0].width, Width::Fill(MIN_NAME));
        assert_eq!(specs[3].width, Width::Fixed(176.));
    }

    #[test]
    fn dragging_a_border_sizes_only_the_column_to_its_left() {
        let layout = ColumnLayout::new(Table::Files).fit(1000.);
        // 1000px: Name fills 1000 - (96 + 160 + 176) = 568; Size spans 568..664.
        let wider = layout.resized(Table::Files, Column::Size, 700., 1000.);
        assert_eq!(wider.widths.get(&Column::Size), Some(&132.));
        assert_eq!(wider.specs(Table::Files)[0].width, Width::Fixed(568.), "Name stays where it was drawn");
        assert_eq!(wider.specs(Table::Files)[2].width, Width::Fixed(160.), "Type keeps its width and moves");
        // Dragging on, the border keeps following the pointer.
        let further = wider.resized(Table::Files, Column::Size, 900., 1000.);
        assert_eq!(further.widths.get(&Column::Size), Some(&332.));
    }

    #[test]
    fn sizing_name_hands_the_spare_room_to_the_last_column() {
        let layout = ColumnLayout::new(Table::Files).resized(Table::Files, Column::Name, 300., 1000.);
        let specs = layout.specs(Table::Files);
        assert_eq!(specs[0].width, Width::Fixed(300.));
        assert_eq!(specs[3].width, Width::Fill(176.));
        assert_eq!(layout.reset_width(Column::Name).specs(Table::Files)[0].width, Width::Fill(MIN_NAME));
    }

    #[test]
    fn columns_keep_a_minimum_and_the_table_its_width() {
        let layout = ColumnLayout::new(Table::Files).resized(Table::Files, Column::Type, 0., 1000.);
        assert_eq!(layout.widths.get(&Column::Type), Some(&MIN_COLUMN));
        // Name was pinned at the 568px it was drawn.
        assert_eq!(layout.min_width(Table::Files), 568. + 96. + MIN_COLUMN + 176.);
    }

    #[test]
    fn a_narrower_pane_takes_its_room_from_name() {
        let sized = ColumnLayout::new(Table::Files).fit(1000.).resized(Table::Files, Column::Size, 700., 1000.);
        assert_eq!(sized.fitted, Some(1000.));
        // A Details panel takes 300px: Name gives them up, the others keep theirs.
        let narrow = sized.fit(700.);
        assert_eq!(narrow.specs(Table::Files)[0].width, Width::Fixed(268.));
        assert_eq!(narrow.specs(Table::Files)[1].width, Width::Fixed(132.));
        assert_eq!(narrow.fit(100.).widths.get(&Column::Name), Some(&MIN_NAME));
        // Closed again, Name is back; a wider window leaves it be.
        assert_eq!(sized.fit(1000.), sized);
        assert_eq!(sized.fit(1400.).specs(Table::Files)[0].width, Width::Fixed(568.));
        // A drag in the narrow pane follows the pointer from where Name is drawn.
        let typed = narrow.resized(Table::Files, Column::Type, 600., 700.);
        assert_eq!(typed.widths.get(&Column::Type), Some(&200.));
        assert_eq!(typed.fitted, Some(700.));
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
        let layout = ColumnLayout { shown: vec![Column::Deleted, Column::Size], widths: HashMap::new(), fitted: None };
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
