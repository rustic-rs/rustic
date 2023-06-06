use bytesize::ByteSize;

#[must_use]
pub fn bytes_size_to_string(b: u64) -> String {
    ByteSize(b).to_string_as(true)
}

/// Helpers for table output
#[cfg(feature = "cli")]
pub mod table_output {
    use comfy_table::{
        presets::ASCII_MARKDOWN, Attribute, CellAlignment, ContentArrangement, Table,
    };

    // Re-export for internal use
    pub(crate) use comfy_table::Cell;
    use log::info;

    use crate::{
        backend::{ReadBackend, ALL_FILE_TYPES},
        error::RusticResult,
    };

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

    pub fn table_right_from<I: IntoIterator<Item = T>, T: ToString>(
        start: usize,
        titles: I,
    ) -> Table {
        let mut table = table_with_titles(titles);
        // set alignment of all rows except first start row
        table
            .column_iter_mut()
            .skip(start)
            .for_each(|c| c.set_cell_alignment(CellAlignment::Right));

        table
    }

    pub fn print_file_info(text: &str, be: &impl ReadBackend) -> RusticResult<()> {
        info!("scanning files...");

        let mut table = table_right_from(1, ["File type", "Count", "Total Size"]);
        let mut total_count = 0;
        let mut total_size = 0;
        for tpe in ALL_FILE_TYPES {
            let list = be.list_with_size(tpe)?;
            let count = list.len();
            let size = list.iter().map(|f| u64::from(f.1)).sum();
            _ = table.add_row([
                format!("{tpe:?}"),
                count.to_string(),
                super::bytes_size_to_string(size),
            ]);
            total_count += count;
            total_size += size;
        }
        println!("{text}");
        _ = table.add_row([
            "Total".to_string(),
            total_count.to_string(),
            super::bytes_size_to_string(total_size),
        ]);

        println!();
        println!("{table}");
        println!();
        Ok(())
    }
}
