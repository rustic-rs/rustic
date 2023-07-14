use bytesize::ByteSize;
use comfy_table::{
    presets::ASCII_MARKDOWN, Attribute, Cell, CellAlignment, ContentArrangement, Table,
};

/// Helpers for table output

pub fn bold_cell<T: ToString>(s: T) -> Cell {
    Cell::new(s).add_attribute(Attribute::Bold)
}

#[must_use]
pub fn table() -> Table {
    let mut table = Table::new();
    _ = table
        .load_preset(ASCII_MARKDOWN)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

pub fn table_with_titles<I: IntoIterator<Item = T>, T: ToString>(titles: I) -> Table {
    let mut table = table();
    _ = table.set_header(titles.into_iter().map(bold_cell));
    table
}

pub fn table_right_from<I: IntoIterator<Item = T>, T: ToString>(start: usize, titles: I) -> Table {
    let mut table = table_with_titles(titles);
    // set alignment of all rows except first start row
    table
        .column_iter_mut()
        .skip(start)
        .for_each(|c| c.set_cell_alignment(CellAlignment::Right));

    table
}

#[must_use]
pub fn bytes_size_to_string(b: u64) -> String {
    ByteSize(b).to_string_as(true)
}
