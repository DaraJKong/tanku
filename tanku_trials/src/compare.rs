//! The comparison of two reports of the same trial: what a change to the game moved, and by
//! how much. It reads the CSV files that the trials write.

use std::{collections::BTreeMap, fs, path::Path};

use crate::report::{BALANCE_LINE, Report, Table};

/// What a comparison is asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    /// The least change that is told, in percent of the value before.
    pub least: f64,
    /// The tables that are compared: all of them if none is named.
    pub tables: Vec<String>,
}

/// A cell of a report: its table, its row and its column.
type Place = (String, String, String);

/// Reads the lines of a CSV file whose fields are in quotes, with a quote in a field
/// written twice.
fn fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let (mut field, mut quoted) = (String::new(), false);
    let mut letters = line.chars().peekable();
    while let Some(letter) = letters.next() {
        match letter {
            '"' if quoted && letters.peek() == Some(&'"') => {
                field.push('"');
                letters.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut field)),
            letter => field.push(letter),
        }
    }
    fields.push(field);
    fields
}

/// The cells of a report as it was written, in the order they were written in.
fn cells(text: &str) -> Result<Vec<(Place, String)>, String> {
    let mut cells = Vec::new();
    for (number, line) in text.lines().enumerate().skip(1) {
        let Ok([table, row, column, value]) = <[String; 4]>::try_from(fields(line)) else {
            return Err(format!("line {} is no cell of a report", number + 1));
        };
        cells.push(((table, row, column), value));
    }
    Ok(cells)
}

/// The number that a cell holds, if it holds one: `0.52`, `98%` or `+4.47`.
fn number(value: &str) -> Option<f64> {
    let value = value.trim().trim_end_matches('%');
    value.trim_start_matches('+').parse().ok()
}

/// What the report beside a CSV file says it was asked for: the first line under its title.
fn asked_for(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path.with_extension("md")).ok()?;
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    lines.nth(1).map(str::to_string)
}

/// The balance that the report beside a CSV file says it was made with, if it says so.
fn balance_of(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path.with_extension("md")).ok()?;
    balance_in(&text)
}

/// The balance that the text of a report names.
fn balance_in(text: &str) -> Option<String> {
    let line = text.lines().find(|line| line.starts_with(BALANCE_LINE))?;
    let named = line.strip_prefix(BALANCE_LINE)?.trim_end_matches('.');
    Some(named.trim_matches('`').to_string())
}

fn read(path: &Path) -> Result<Vec<(Place, String)>, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    cells(&text).map_err(|error| format!("{}: {error}", path.display()))
}

/// The tables of what changed between the cells of two reports.
fn changes(before: &[(Place, String)], after: &[(Place, String)], asked: &Asked) -> Vec<Table> {
    let now: BTreeMap<&Place, &String> =
        after.iter().map(|(place, value)| (place, value)).collect();
    let then: BTreeMap<&Place, &String> =
        before.iter().map(|(place, value)| (place, value)).collect();
    let wanted = |table: &str| {
        asked.tables.is_empty()
            || asked
                .tables
                .iter()
                .any(|name| name.eq_ignore_ascii_case(table))
    };
    let columns = ["row, column", "before", "after", "change", "in percent"];
    let mut tables: Vec<Table> = Vec::new();
    let mut tell = |table: &str, cells: Vec<String>| {
        if tables.last().is_none_or(|last| last.title != table) {
            tables.push(Table::new(table, "", &columns));
        }
        tables.last_mut().unwrap().row(cells);
    };
    for (place, was) in before.iter().filter(|(place, _)| wanted(&place.0)) {
        let name = format!("{}, {}", place.1, place.2);
        let Some(&is) = now.get(place) else {
            tell(
                &place.0,
                vec![name, was.clone(), "gone".into(), "-".into(), "-".into()],
            );
            continue;
        };
        let told = match (number(was), number(is)) {
            (Some(old), Some(new)) => {
                let change = new - old;
                let share = match old {
                    0.0 => f64::INFINITY,
                    old => change * 100.0 / old.abs(),
                };
                let decimals = was
                    .split('.')
                    .nth(1)
                    .map_or(0, |part| part.trim_end_matches('%').len());
                let percent = match share.is_finite() {
                    true => format!("{share:+.0}%"),
                    false => "-".to_string(),
                };
                (change != 0.0 && share.abs() >= asked.least)
                    .then(|| (format!("{change:+.decimals$}"), percent))
            }
            _ => (was != is).then(|| ("-".to_string(), "-".to_string())),
        };
        if let Some((change, percent)) = told {
            tell(
                &place.0,
                vec![name, was.clone(), is.clone(), change, percent],
            );
        }
    }
    for (place, is) in after.iter().filter(|(place, _)| wanted(&place.0)) {
        if !then.contains_key(place) {
            let name = format!("{}, {}", place.1, place.2);
            tell(
                &place.0,
                vec![name, "none".into(), is.clone(), "-".into(), "-".into()],
            );
        }
    }
    tables
}

/// Compares the report of a trial with a later one of the same trial, from their CSV files.
pub fn compare(before: &Path, after: &Path, asked: &Asked) -> Result<Report, String> {
    let (then, now) = (read(before)?, read(after)?);
    let mut tables = changes(&then, &now, asked);
    let compared = then
        .iter()
        .filter(|(place, _)| now.iter().any(|(other, _)| other == place));
    let changed: usize = tables.iter().map(|table| table.rows.len()).sum();
    let mut told = format!(
        "From `{}` to `{}`: {changed} of {} cells changed by {}% or more.",
        before.display(),
        after.display(),
        compared.count().max(changed),
        asked.least,
    );
    // Two reports that were not asked for the same thing differ by more than the game.
    match (asked_for(before), asked_for(after)) {
        (Some(one), Some(other)) if one != other => {
            told += &format!(
                "\n\nThe two were not asked for the same thing.\n\n- Before: {one}\n- After: {other}"
            );
        }
        _ => {}
    }
    // A change of the balance is what a comparison is mostly made for.
    match (balance_of(before), balance_of(after)) {
        (Some(one), Some(other)) if one != other => {
            told += &format!("\n\nThe balance changed between the two: `{one}`, then `{other}`.");
        }
        (Some(one), Some(_)) => {
            told += &format!("\n\nBoth were made with the balance `{one}`.");
        }
        _ => {}
    }
    if tables.is_empty() {
        tables.push(Table::new(
            "Nothing changed",
            "",
            &["row, column", "before", "after"],
        ));
    }
    Ok(Report {
        title: "Comparison".to_string(),
        asked: told,
        balance: None,
        tables,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(losses: &str, cleared: &str) -> String {
        let mut table = Table::new("Sectors", "", &["sectors", "losses", "cleared", "made of"]);
        table.row(vec![
            "1-2".into(),
            "0.10".into(),
            "98%".into(),
            "a \"b\", c".into(),
        ]);
        table.row(vec![
            "3-5".into(),
            losses.into(),
            cleared.into(),
            "a".into(),
        ]);
        let report = Report {
            title: String::new(),
            asked: String::new(),
            balance: None,
            tables: vec![table],
        };
        report.csv()
    }

    fn asked(least: f64) -> Asked {
        Asked {
            least,
            tables: Vec::new(),
        }
    }

    #[test]
    fn a_report_is_read_back_as_it_was_written() {
        let cells = cells(&report("0.50", "90%")).unwrap();
        assert_eq!(cells.len(), 6);
        let place = (
            "Sectors".to_string(),
            "1-2".to_string(),
            "made of".to_string(),
        );
        assert_eq!(cells[2], (place, "a \"b\", c".to_string()));
        assert_eq!(number("98%"), Some(98.0));
        assert_eq!(number("+4.47"), Some(4.47));
        assert_eq!(number("-"), None);
        assert!(super::cells("table,row,column,value\n\"a\",\"b\"").is_err());
    }

    #[test]
    fn only_what_changed_enough_is_told() {
        let (before, after) = (
            cells(&report("0.50", "90%")).unwrap(),
            cells(&report("0.75", "88%")).unwrap(),
        );
        let tables = changes(&before, &after, &asked(10.0));
        assert_eq!(tables.len(), 1);
        // The losses rose by half, and the fights cleared fell by 2%, which is too little.
        let row = ["3-5, losses", "0.50", "0.75", "+0.25", "+50%"].map(String::from);
        assert_eq!(tables[0].rows, vec![row.to_vec()]);
        assert_eq!(changes(&before, &after, &asked(1.0))[0].rows.len(), 2);
        assert!(changes(&before, &before, &asked(0.0)).is_empty());
        // A table that isn't asked for is left out.
        let other = Asked {
            least: 0.0,
            tables: vec!["classes".to_string()],
        };
        assert!(changes(&before, &after, &other).is_empty());
    }

    #[test]
    fn a_row_that_came_or_went_is_told() {
        let before = cells(&report("0.50", "90%")).unwrap();
        let after: Vec<_> = before.iter().take(3).cloned().collect();
        let gone = changes(&before, &after, &asked(10.0));
        assert_eq!(gone[0].rows.len(), 3);
        assert!(gone[0].rows.iter().all(|row| row[2] == "gone"));
        let come = changes(&after, &before, &asked(10.0));
        assert!(come[0].rows.iter().all(|row| row[1] == "none"));
    }

    #[test]
    fn a_comparison_finds_the_balance_that_each_report_names() {
        let report = |balance: Option<u64>| Report {
            title: "Census".to_string(),
            asked: "Two seeds.".to_string(),
            balance,
            tables: Vec::new(),
        };
        let made = report(Some(0x1234_5678_9ABC_DEF0)).markdown();
        assert_eq!(balance_in(&made), Some("123456789abcdef0".to_string()));
        // A report from before the balance had a file names none, and neither does a
        // comparison.
        assert_eq!(balance_in(&report(None).markdown()), None);
        // What was asked for is still the first line under the title.
        let mut lines = made.lines().filter(|line| !line.trim().is_empty());
        assert_eq!(lines.nth(1), Some("Two seeds."));
    }
}
