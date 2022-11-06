use anyhow::Result;
use console::{pad_str_with, Alignment, Style, Term};
use std::{cmp, fmt::Display};

pub struct Table {
    titles: Vec<String>,
    rows: Vec<Vec<String>>,
    max_widths: Vec<usize>,
}

const COL_DELIMETER: &'static str = "|";
const TITLE_DELIMETER: &'static str = "-";
const TITLE_COL_DELIMETER: &'static str = "+";

impl Table {
    pub fn new() -> Self {
        Self {
            titles: vec![],
            rows: vec![],
            max_widths: vec![],
        }
    }

    fn calculate(&mut self, items: &Vec<String>) {
        for (idx, item) in items.iter().enumerate() {
            if self.max_widths.len() < idx {
                self.max_widths.push(item.len());
                continue;
            }
            self.max_widths[idx] = cmp::max(self.max_widths[idx], item.len());
        }
    }

    fn transform<I, D>(&self, items: I) -> Vec<String>
    where
        I: IntoIterator<Item = D>,
        D: Display,
    {
        items.into_iter().map(|v| format!(" {} ", v)).collect()
    }

    pub fn set_titles<I, D>(&mut self, titles: I)
    where
        I: IntoIterator<Item = D>,
        D: Display,
    {
        let _titles = self.transform(titles);
        for title in &_titles {
            self.max_widths.push(title.len());
        }
        self.titles = _titles;
    }

    pub fn add_row<I, D>(&mut self, row: I)
    where
        I: IntoIterator<Item = D>,
        D: Display,
    {
        let _row = self.transform(row);
        self.calculate(&_row);
        self.rows.push(_row);
    }

    fn print_row(
        &self,
        term: &Term,
        row: &Vec<String>,
        fill_char: char,
        style: Style,
        common_align: Alignment,
    ) -> Result<()> {
        let mut iter = row.clone().into_iter().enumerate();
        let first = iter.next();
        if first == None {
            return Ok(());
        }
        let first = first.unwrap();
        let max = self.max_widths[0];
        let title = pad_str_with(first.1.as_str(), max, Alignment::Left, None, fill_char);
        term.write_str(format!("{}", style.apply_to(title)).as_str())?;
        for (idx, value) in iter {
            let max = self.max_widths[idx];
            let mut align = common_align;
            if idx == self.max_widths.len() - 1 {
                align = Alignment::Right;
            }
            let value = pad_str_with(value.as_str(), max, align, None, fill_char);
            term.write_str(COL_DELIMETER)?;
            term.write_str(format!("{}", style.apply_to(value)).as_str())?;
        }
        term.write_line("")?;
        Ok(())
    }

    fn print_header(&self, term: &Term) -> Result<()> {
        self.print_row(term, &self.titles, ' ', term.style().bold(), Alignment::Center)?;
        for (idx, max) in self.max_widths.clone().into_iter().enumerate() {
            let del_row = pad_str_with(
                TITLE_DELIMETER,
                max,
                Alignment::Center,
                None,
                TITLE_DELIMETER.chars().nth(0).unwrap(),
            );
            if idx != 0 {
                term.write_str(TITLE_COL_DELIMETER)?;
            }
            term.write_str(del_row.as_ref())?;
        }
        term.write_line("")?;
        Ok(())
    }

    pub fn print(&mut self, term: Term) -> Result<()> {
        self.print_header(&term)?;
        for row in self.rows.clone() {
            self.print_row(&term, &row, ' ', term.style(), Alignment::Right)?;
        }

        Ok(())
    }

    pub fn printstd(&mut self) -> Result<()> {
        self.print(Term::stdout())
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printing() {
        let mut table = Table::new();
        table.set_titles(vec!["h1", "h2", "h3"]);
        table.add_row(vec!["1", "222", "333"]);
        table.add_row(vec!["4444", "55", "6"]);

        table.printstd().unwrap();
    }
}
