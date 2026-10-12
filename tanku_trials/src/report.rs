//! The report of a trial: tables, which are printed as Markdown for a reader and written as
//! one CSV file for whatever compares two reports.

use std::{fmt::Write, fs, io, path::Path};

/// One table of a report.
pub struct Table {
    pub title: String,
    /// What the table counts, and how to read it.
    pub note: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(title: &str, note: &str, columns: &[&str]) -> Self {
        Self {
            title: title.to_string(),
            note: note.to_string(),
            columns: columns.iter().map(|column| column.to_string()).collect(),
            rows: Vec::new(),
        }
    }

    pub fn row(&mut self, cells: Vec<String>) {
        debug_assert_eq!(cells.len(), self.columns.len(), "{}", self.title);
        self.rows.push(cells);
    }
}

/// What a trial found.
pub struct Report {
    pub title: String,
    /// What was asked for, so that the same report can be made again.
    pub asked: String,
    /// The balance that the trial was made with: the fingerprint of `game.balance` as the
    /// tool was compiled with it. A comparison of two reports is made with none.
    pub balance: Option<u64>,
    pub tables: Vec<Table>,
}

/// What a report says of its balance, in front of the fingerprint.
pub const BALANCE_LINE: &str = "Made with the balance ";

impl Report {
    pub fn markdown(&self) -> String {
        let mut text = format!("# {}\n\n{}\n", self.title, self.asked);
        if let Some(balance) = self.balance {
            let _ = writeln!(text, "\n{BALANCE_LINE}`{balance:016x}`.");
        }
        for table in &self.tables {
            let _ = write!(text, "\n## {}\n\n{}\n\n", table.title, table.note);
            let _ = writeln!(text, "| {} |", table.columns.join(" | "));
            // The first column names the row, and the others hold numbers.
            let rule: Vec<&str> = (0..table.columns.len())
                .map(|column| if column == 0 { "---" } else { "---:" })
                .collect();
            let _ = writeln!(text, "|{}|", rule.join("|"));
            for row in &table.rows {
                let _ = writeln!(text, "| {} |", row.join(" | "));
            }
        }
        text
    }

    /// Every cell on a line of its own: the table, the row, the column and the value.
    pub fn csv(&self) -> String {
        let quoted = |cell: &str| format!("\"{}\"", cell.replace('"', "\"\""));
        let mut text = "table,row,column,value\n".to_string();
        for table in &self.tables {
            for row in &table.rows {
                for (column, cell) in table.columns.iter().zip(row).skip(1) {
                    let cells = [&table.title, &row[0], column, cell];
                    let _ = writeln!(text, "{}", cells.map(|cell| quoted(cell)).join(","));
                }
            }
        }
        text
    }

    /// Writes the report to a directory, under a name: as Markdown and as CSV.
    pub fn write(&self, directory: &Path, name: &str) -> io::Result<()> {
        fs::create_dir_all(directory)?;
        fs::write(directory.join(format!("{name}.md")), self.markdown())?;
        fs::write(directory.join(format!("{name}.csv")), self.csv())
    }
}

/// A share as a percentage without decimals, or a dash where there is nothing to share.
pub fn percent(part: usize, whole: usize) -> String {
    if whole == 0 {
        return "-".to_string();
    }
    format!("{:.0}%", part as f64 * 100.0 / whole as f64)
}

/// The mean of some values with one decimal, or a dash where there is none.
pub fn mean(values: impl Iterator<Item = f64>) -> String {
    let (sum, count) = values.fold((0.0, 0usize), |(sum, count), value| {
        (sum + value, count + 1)
    });
    if count == 0 {
        return "-".to_string();
    }
    format!("{:.1}", sum / count as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv_of(report: &Report) -> String {
        let plain = Report {
            title: report.title.clone(),
            asked: report.asked.clone(),
            balance: None,
            tables: report
                .tables
                .iter()
                .map(|table| Table {
                    title: table.title.clone(),
                    note: table.note.clone(),
                    columns: table.columns.clone(),
                    rows: table.rows.clone(),
                })
                .collect(),
        };
        plain.csv()
    }

    #[test]
    fn a_report_is_told_as_markdown_and_as_csv() {
        let mut table = Table::new(
            "Groups",
            "How many tanks.",
            &["sectors", "tanks", "a \"b\""],
        );
        table.row(vec!["1-2".into(), "1.0".into(), "50%".into()]);
        let mut report = Report {
            title: "Census".into(),
            asked: "Two seeds.".into(),
            balance: None,
            tables: vec![table],
        };
        let markdown = report.markdown();
        assert!(markdown.starts_with("# Census\n\nTwo seeds.\n\n## Groups\n\nHow many tanks.\n\n"));
        // A report of a trial says which balance it was made with, under what was asked.
        report.balance = Some(0x00AB_CDEF_0123_4567);
        let made =
            "# Census\n\nTwo seeds.\n\nMade with the balance `00abcdef01234567`.\n\n## Groups";
        assert!(report.markdown().starts_with(made), "{}", report.markdown());
        // Its cells are the same either way.
        assert_eq!(report.csv(), csv_of(&report));
        report.balance = None;
        assert!(
            markdown
                .contains("| sectors | tanks | a \"b\" |\n|---|---:|---:|\n| 1-2 | 1.0 | 50% |\n")
        );
        let csv = report.csv();
        let wanted = "table,row,column,value\n\"Groups\",\"1-2\",\"tanks\",\"1.0\"\n\
                      \"Groups\",\"1-2\",\"a \"\"b\"\"\",\"50%\"\n";
        assert_eq!(csv, wanted);
    }

    #[test]
    fn shares_and_means_say_when_there_is_nothing() {
        assert_eq!(percent(1, 3), "33%");
        assert_eq!(percent(0, 0), "-");
        assert_eq!(mean([1.0, 2.0].into_iter()), "1.5");
        assert_eq!(mean(std::iter::empty()), "-");
    }
}
