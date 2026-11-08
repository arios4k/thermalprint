//! Merge a CSV table into a label design: `${column}` placeholders in text,
//! barcode, and QR content are replaced row by row, and each row is one label.
use crate::design::{Element, LabelDocument};
use anyhow::{Context, Result, bail};

pub const MAX_ROWS: usize = 500;
pub const MAX_COLUMNS: usize = 64;
pub const MAX_BYTES: usize = 2_000_000;
/// Built-in variables that the renderer fills in itself.
const BUILT_IN: [&str; 2] = ["date", "serial"];

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Parse CSV, semicolon-, or tab-separated text. The first row names the columns.
pub fn parse_table(bytes: &[u8]) -> Result<Table> {
    if bytes.len() > MAX_BYTES {
        bail!("The file is larger than {} MB", MAX_BYTES / 1_000_000);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let first_line = bytes.split(|&b| b == b'\n').next().unwrap_or_default();
    let delimiter = b",;\t"
        .iter()
        .copied()
        .max_by_key(|d| first_line.iter().filter(|b| *b == d).count())
        .unwrap_or(b',');
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(bytes);
    let headers: Vec<String> = reader
        .headers()
        .context("Could not read the header row")?
        .iter()
        .map(|h| h.trim().to_owned())
        .collect();
    if headers.is_empty() || headers.iter().all(String::is_empty) {
        bail!("The first row must name the columns");
    }
    if headers.len() > MAX_COLUMNS {
        bail!("More than {MAX_COLUMNS} columns");
    }
    if let Some(position) = headers.iter().position(String::is_empty) {
        bail!("Column {} has no name", position + 1);
    }
    let mut seen = std::collections::HashSet::new();
    for header in &headers {
        if !seen.insert(header.to_lowercase()) {
            bail!("Two columns are named \"{header}\"");
        }
    }
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.context("Could not read a data row")?;
        let mut row: Vec<String> = record.iter().map(|c| c.trim().to_owned()).collect();
        row.resize(headers.len(), String::new());
        if row.iter().all(String::is_empty) {
            continue;
        }
        rows.push(row);
        if rows.len() > MAX_ROWS {
            bail!("More than {MAX_ROWS} rows. Split the file into smaller batches");
        }
    }
    if rows.is_empty() {
        bail!("The file has a header but no data rows");
    }
    Ok(Table { headers, rows })
}

fn contents_mut(element: &mut Element) -> Option<&mut String> {
    match element {
        Element::Text { text, .. } => Some(text),
        Element::Barcode { data, .. } | Element::Qr { data } => Some(data),
        _ => None,
    }
}

/// Names used as `${name}` in the design, in order of first use, without built-ins.
pub fn variables(doc: &LabelDocument) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for object in &doc.objects {
        let text = match &object.element {
            Element::Text { text, .. } => text,
            Element::Barcode { data, .. } | Element::Qr { data } => data,
            _ => continue,
        };
        let mut rest = text.as_str();
        while let Some(start) = rest.find("${") {
            let after = &rest[start + 2..];
            let Some(end) = after.find('}') else { break };
            let name = after[..end].trim();
            if !name.is_empty()
                && !BUILT_IN.contains(&name.to_lowercase().as_str())
                && !found.iter().any(|f| f.eq_ignore_ascii_case(name))
            {
                found.push(name.to_owned());
            }
            rest = &after[end + 1..];
        }
    }
    found
}

/// Replace `${column}` (case-insensitive) in one pass, so values are never re-expanded.
fn merge(text: &str, headers: &[String], row: &[String]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = after[..end].trim();
        match headers.iter().position(|h| h.eq_ignore_ascii_case(name)) {
            Some(index) => out.push_str(&row[index]),
            None => out.push_str(&rest[start..start + 2 + end + 1]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The design for one row. `${serial}` counts up from the design's serial number.
pub fn apply_row(doc: &LabelDocument, table: &Table, index: usize) -> LabelDocument {
    let mut label = doc.clone();
    label.serial_number = doc.serial_number.saturating_add(index as u64);
    let row = &table.rows[index];
    for object in &mut label.objects {
        if let Some(text) = contents_mut(&mut object.element) {
            *text = merge(text, &table.headers, row);
        }
    }
    label
}

#[derive(Debug, Default, PartialEq)]
pub struct Match {
    /// Placeholders in the design that have no column.
    pub missing: Vec<String>,
    /// Columns the design never uses.
    pub unused: Vec<String>,
    /// Placeholders that will be filled.
    pub used: usize,
}

pub fn compare(doc: &LabelDocument, table: &Table) -> Match {
    let wanted = variables(doc);
    let missing: Vec<String> = wanted
        .iter()
        .filter(|name| !table.headers.iter().any(|h| h.eq_ignore_ascii_case(name)))
        .cloned()
        .collect();
    let unused = table
        .headers
        .iter()
        .filter(|h| !wanted.iter().any(|name| name.eq_ignore_ascii_case(h)))
        .cloned()
        .collect();
    Match {
        used: wanted.len() - missing.len(),
        missing,
        unused,
    }
}

/// Render every row before anything is sent, so one bad row cannot waste a roll of labels.
/// Returns up to five problems, numbered by data row (1 is the first row after the header).
pub fn preflight(doc: &LabelDocument, table: &Table, dpi: u32) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    let total = table.rows.len();
    for index in 0..total {
        let label = apply_row(doc, table, index);
        if let Err(error) = crate::design_render::render_label(&label, dpi) {
            problems.push(format!("Row {}: {error:#}", index + 1));
            if problems.len() == 5 {
                break;
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::BarcodeKind;

    fn design() -> LabelDocument {
        let mut doc = LabelDocument::blank(101.6, 152.4);
        doc.add(
            Element::Text {
                text: "Ship to ${Name}\n${City} ${date}".into(),
                font_pt: 12.,
                bold: false,
                align: crate::design::TextAlign::Left,
            },
            5.,
            5.,
            80.,
            20.,
        );
        doc.add(
            Element::Barcode {
                kind: BarcodeKind::Code128,
                data: "${sku}".into(),
                show_text: true,
            },
            5.,
            40.,
            80.,
            20.,
        );
        doc.add(
            Element::Qr {
                data: "id=${SKU}&n=${serial}".into(),
            },
            5.,
            70.,
            20.,
            20.,
        );
        doc
    }

    #[test]
    fn parses_commas_semicolons_tabs_quotes_and_a_byte_order_mark() {
        let comma = parse_table(b"\xEF\xBB\xBFName,City\n\"Smith, Jo\",Austin\n").unwrap();
        assert_eq!(comma.headers, ["Name", "City"]);
        assert_eq!(comma.rows[0], ["Smith, Jo", "Austin"]);
        assert_eq!(parse_table(b"a;b\n1;2\n").unwrap().rows[0], ["1", "2"]);
        assert_eq!(parse_table(b"a\tb\n1\t2\n").unwrap().rows[0], ["1", "2"]);
        let multiline = parse_table(b"a,b\n\"line1\nline2\",x\n").unwrap();
        assert_eq!(multiline.rows[0][0], "line1\nline2");
    }

    #[test]
    fn pads_short_rows_skips_blank_rows_and_trims() {
        let table = parse_table(b"a,b,c\n 1 ,2\n,,\n3,4,5\n").unwrap();
        assert_eq!(table.rows, [vec!["1", "2", ""], vec!["3", "4", "5"]]);
    }

    #[test]
    fn rejects_unusable_files_with_a_clear_reason() {
        let error = |bytes: &[u8]| parse_table(bytes).unwrap_err().to_string();
        assert!(error(b"a,b\n").contains("no data rows"));
        assert!(error(b"a,a\n1,2\n").contains("Two columns"));
        assert!(error(b"a,,c\n1,2,3\n").contains("no name"));
        assert!(error(&vec![b'x'; MAX_BYTES + 1]).contains("larger"));
        let many = format!("v\n{}", "1\n".repeat(MAX_ROWS + 1));
        assert!(error(many.as_bytes()).contains("More than"));
    }

    #[test]
    fn finds_variables_ignoring_built_ins_and_case() {
        assert_eq!(variables(&design()), ["Name", "City", "sku"]);
    }

    #[test]
    fn merging_is_case_insensitive_one_pass_and_keeps_unknown_placeholders() {
        let table = parse_table(b"Name,City,SKU\nAda,${City},A-1\nBo,Rome,B-2\n").unwrap();
        let first = apply_row(&design(), &table, 0);
        let text = |label: &LabelDocument, i: usize| match &label.objects[i].element {
            Element::Text { text, .. }
            | Element::Barcode { data: text, .. }
            | Element::Qr { data: text } => text.clone(),
            _ => unreachable!(),
        };
        // A value that looks like a placeholder is not expanded again.
        assert_eq!(text(&first, 0), "Ship to Ada\n${City} ${date}");
        assert_eq!(text(&first, 1), "A-1");
        assert_eq!(text(&first, 2), "id=A-1&n=${serial}");
        let second = apply_row(&design(), &table, 1);
        assert_eq!(second.serial_number, design().serial_number + 1);
        assert_eq!(text(&second, 0), "Ship to Bo\nRome ${date}");
    }

    #[test]
    fn compare_reports_missing_and_unused_columns() {
        let table = parse_table(b"Name,sku,Notes\nAda,A-1,x\n").unwrap();
        let result = compare(&design(), &table);
        assert_eq!(result.missing, ["City"]);
        assert_eq!(result.unused, ["Notes"]);
        assert_eq!(result.used, 2);
    }

    #[test]
    fn preflight_names_the_bad_rows_before_anything_prints() {
        let table =
            parse_table("Name,City,sku\nAda,Rome,A-1\nBo,Rome,\u{1F600}\nCy,Rome,C-3\n".as_bytes())
                .unwrap();
        let problems = preflight(&design(), &table, 203).unwrap_err();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].starts_with("Row 2:"), "{problems:?}");
        let good = parse_table(b"Name,City,sku\nAda,Rome,A-1\n").unwrap();
        assert!(preflight(&design(), &good, 203).is_ok());
    }
}
