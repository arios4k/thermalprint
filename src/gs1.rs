//! GS1 element strings for GS1-128 and GS1 Data Matrix, such as `(01)09506000134352(10)LOT7`.
//!
//! Only the common application identifiers (AIs) are supported, each checked for length,
//! characters, dates, and check digits, so a typo is reported instead of printed.
use anyhow::{Result, bail};

/// Group separator that marks the end of a variable-length field inside an element string.
pub const GS: char = '\u{1D}';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub ai: String,
    pub value: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Exactly this many digits; `check` marks a GS1 mod-10 check digit at the end.
    Fixed { len: usize, check: bool },
    /// Up to this many digits.
    VarDigits(usize),
    /// Up to this many printable characters.
    VarText(usize),
    /// Six digits, YYMMDD.
    Date,
}

fn spec(ai: &str) -> Option<Kind> {
    use Kind::*;
    Some(match ai {
        "00" => Fixed {
            len: 18,
            check: true,
        },
        "01" | "02" => Fixed {
            len: 14,
            check: true,
        },
        "10" | "21" => VarText(20),
        "11" | "13" | "15" | "17" => Date,
        "20" => Fixed {
            len: 2,
            check: false,
        },
        "30" | "37" => VarDigits(8),
        "240" | "241" | "400" | "401" => VarText(30),
        "242" => VarDigits(6),
        "420" => VarText(20),
        "410" | "411" | "412" | "413" | "414" | "415" => Fixed {
            len: 13,
            check: true,
        },
        _ => {
            // 310n to 316n and 320n to 325n: a measurement with n decimal places.
            let bytes = ai.as_bytes();
            let measure = bytes.len() == 4
                && bytes.iter().all(u8::is_ascii_digit)
                && (matches!(
                    &ai[..3],
                    "310" | "311" | "312" | "313" | "314" | "315" | "316"
                ) || matches!(&ai[..3], "320" | "321" | "322" | "323" | "324" | "325"));
            if measure {
                Fixed {
                    len: 6,
                    check: false,
                }
            } else {
                return None;
            }
        }
    })
}

fn is_variable(ai: &str) -> bool {
    matches!(spec(ai), Some(Kind::VarDigits(_) | Kind::VarText(_)))
}

/// The GS1 mod-10 check digit for the digits before it.
pub fn check_digit(body: &str) -> Option<char> {
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let sum: u32 = body
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| (b - b'0') as u32 * if i % 2 == 0 { 3 } else { 1 })
        .sum();
    char::from_digit((10 - sum % 10) % 10, 10)
}

fn valid_date(value: &str) -> bool {
    let month: u32 = value[2..4].parse().unwrap_or(99);
    let day: u32 = value[4..6].parse().unwrap_or(99);
    (1..=12).contains(&month) && day <= 31
}

fn check_value(ai: &str, value: &str) -> Result<String> {
    let kind =
        spec(ai).ok_or_else(|| anyhow::anyhow!("({ai}) is not a supported GS1 identifier"))?;
    let digits = |v: &str| v.bytes().all(|b| b.is_ascii_digit());
    match kind {
        Kind::Fixed { len, check } => {
            if !digits(value) {
                bail!("({ai}) must be digits only");
            }
            if check && value.len() == len - 1 {
                // The check digit may be left off; it is added.
                let digit = check_digit(value).unwrap_or('0');
                return Ok(format!("{value}{digit}"));
            }
            if value.len() != len {
                bail!(
                    "({ai}) needs {len} digits{}",
                    if check {
                        format!(", or {} without the check digit", len - 1)
                    } else {
                        String::new()
                    }
                );
            }
            if check && check_digit(&value[..len - 1]) != value.chars().last() {
                bail!("({ai}) has a wrong check digit");
            }
        }
        Kind::Date => {
            if value.len() != 6 || !digits(value) || !valid_date(value) {
                bail!("({ai}) must be a date as YYMMDD");
            }
        }
        Kind::VarDigits(max) => {
            if value.is_empty() || value.len() > max || !digits(value) {
                bail!("({ai}) needs 1 to {max} digits");
            }
        }
        Kind::VarText(max) => {
            if value.is_empty() || value.len() > max {
                bail!("({ai}) needs 1 to {max} characters");
            }
            if !value
                .bytes()
                .all(|b| (33..=126).contains(&b) && b != b'(' && b != b')')
            {
                bail!("({ai}) accepts letters, digits, and symbols but no spaces or brackets");
            }
        }
    }
    Ok(value.to_string())
}

/// Read `(AI)value(AI)value…` into checked fields.
pub fn parse(text: &str) -> Result<Vec<Field>> {
    let text = text.trim();
    if text.is_empty() {
        bail!("Enter GS1 data such as (01)09506000134352(10)LOT7");
    }
    let mut fields = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let Some(after) = rest.strip_prefix('(') else {
            bail!("GS1 data must look like (01)09506000134352(10)LOT7");
        };
        let Some(close) = after.find(')') else {
            bail!("A bracket is not closed");
        };
        let ai = &after[..close];
        let tail = &after[close + 1..];
        let end = tail.find('(').unwrap_or(tail.len());
        let value = &tail[..end];
        if fields.iter().any(|f: &Field| f.ai == ai) {
            bail!("({ai}) appears twice");
        }
        fields.push(Field {
            ai: ai.to_string(),
            value: check_value(ai, value)?,
        });
        rest = &tail[end..];
    }
    Ok(fields)
}

/// The text printed under the bars, with every check digit filled in.
pub fn human(fields: &[Field]) -> String {
    fields
        .iter()
        .map(|f| format!("({}){}", f.ai, f.value))
        .collect()
}

/// AI and value joined, with `separator` after every variable-length field that is not last.
pub fn element_string(fields: &[Field], separator: char) -> String {
    let mut out = String::new();
    for (i, field) in fields.iter().enumerate() {
        out.push_str(&field.ai);
        out.push_str(&field.value);
        if i + 1 < fields.len() && is_variable(&field.ai) {
            out.push(separator);
        }
    }
    out
}

/// The text for the Code 128 encoder: a start code, FNC1, then the data in the shortest
/// mix of character sets (pairs of digits are packed in set C).
pub fn code128_symbol(fields: &[Field]) -> Result<String> {
    if fields.is_empty() {
        bail!("Enter GS1 data such as (01)09506000134352(10)LOT7");
    }
    let chars: Vec<char> = element_string(fields, GS).chars().collect();
    let run = |from: usize| {
        chars[from..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count()
    };
    let mut out = String::new();
    let first = run(0);
    let mut in_c = first >= 4 || (first >= 2 && first == chars.len());
    out.push(if in_c { 'Ć' } else { 'Ɓ' });
    out.push('Ź');
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == GS {
            out.push('Ź');
            i += 1;
            continue;
        }
        let digits = run(i);
        if in_c {
            if digits >= 2 {
                out.push(chars[i]);
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            out.push('Ɓ');
            in_c = false;
        }
        if digits >= 4 || (digits >= 2 && i + digits == chars.len()) {
            if digits % 2 == 1 {
                out.push(chars[i]);
                i += 1;
            }
            out.push('Ć');
            in_c = true;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    Ok(out)
}

/// The data for a GS1 Data Matrix. The encoder used here can only mark the start with FNC1, so
/// a variable-length field has to come last.
pub fn data_matrix_payload(fields: &[Field]) -> Result<String> {
    if fields.is_empty() {
        bail!("Enter GS1 data such as (01)09506000134352(10)LOT7");
    }
    if fields[..fields.len() - 1]
        .iter()
        .any(|f| is_variable(&f.ai))
    {
        bail!("In a GS1 Data Matrix, put the variable-length field (such as batch or serial) last");
    }
    Ok(element_string(fields, GS))
}

/// An SSCC (pallet or parcel number): extension digit, company prefix, serial reference,
/// and the check digit, as 18 digits.
pub fn sscc(extension: u8, company_prefix: &str, serial: &str) -> Result<String> {
    if extension > 9 {
        bail!("The extension digit is 0 to 9");
    }
    let digits = |v: &str| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit());
    if !digits(company_prefix) || !(6..=12).contains(&company_prefix.len()) {
        bail!("The company prefix is 6 to 12 digits");
    }
    if !digits(serial) {
        bail!("The serial reference is digits only");
    }
    let room = 16 - company_prefix.len();
    if serial.len() > room {
        bail!("The serial reference can have at most {room} digits with this prefix");
    }
    let body = format!("{extension}{company_prefix}{serial:0>room$}");
    let digit = check_digit(&body).unwrap_or('0');
    Ok(format!("{body}{digit}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_digits_follow_the_gs1_rule() {
        assert_eq!(check_digit("9501234567890"), Some('3'));
        assert_eq!(check_digit("0950600013435"), Some('2'));
        assert_eq!(check_digit("1234567890123456"), Some('0'));
        assert_eq!(check_digit("12a"), None);
        assert_eq!(check_digit(""), None);
    }

    #[test]
    fn a_missing_check_digit_is_added_and_a_wrong_one_is_refused() {
        let fields = parse("(01)0950600013435(10)LOT7").unwrap();
        assert_eq!(human(&fields), "(01)09506000134352(10)LOT7");
        assert!(
            parse("(01)09506000134353")
                .unwrap_err()
                .to_string()
                .contains("check digit")
        );
        assert!(parse("(01)095060001343").is_err(), "wrong length");
    }

    #[test]
    fn every_field_is_validated() {
        for bad in [
            "",
            "01 09506000134352",
            "(01)09506000134352(01)09506000134352",
            "(99)123",
            "(10)",
            "(10)has space",
            "(17)251301",
            "(17)25AB01",
            "(30)12345678901",
            "(10)ABC(",
            "(3103)12",
        ] {
            assert!(parse(bad).is_err(), "{bad:?} should be refused");
        }
        for good in ["(17)251231", "(3103)001234", "(21)A-b_9", "(400)ORDER123"] {
            assert!(parse(good).is_ok(), "{good:?}");
        }
    }

    #[test]
    fn code128_uses_set_c_for_digit_runs_and_fnc1_between_variable_fields() {
        let symbol = |text: &str| code128_symbol(&parse(text).unwrap()).unwrap();
        assert_eq!(symbol("(01)95012345678903"), "ĆŹ0195012345678903");
        assert_eq!(
            symbol("(01)95012345678903(10)ABC"),
            "ĆŹ019501234567890310ƁABC"
        );
        assert_eq!(
            symbol("(10)AB(01)95012345678903"),
            "ƁŹ10ABŹĆ0195012345678903"
        );
        assert_eq!(symbol("(10)7(21)8"), "ƁŹ107Ź2Ć18");
    }

    #[test]
    fn the_symbols_can_be_encoded_by_the_barcode_library() {
        for text in [
            "(00)106141411234567897",
            "(01)95012345678903(17)251231(10)LOT-7",
            "(10)AB(21)12345(01)95012345678903",
            "(02)95012345678903(37)24",
        ] {
            let symbol = code128_symbol(&parse(text).unwrap()).unwrap();
            let bars = barcoders::sym::code128::Code128::new(&symbol)
                .unwrap_or_else(|e| panic!("{text}: {e:?}"))
                .encode();
            assert!(bars.len() > 90, "{text}");
        }
    }

    #[test]
    fn data_matrix_needs_the_variable_field_last() {
        let ok = parse("(01)95012345678903(10)LOT7").unwrap();
        assert_eq!(
            data_matrix_payload(&ok).unwrap(),
            "0195012345678903".to_string() + "10LOT7"
        );
        let bad = parse("(10)LOT7(01)95012345678903").unwrap();
        assert!(data_matrix_payload(&bad).is_err());
    }

    #[test]
    fn sscc_numbers_are_padded_and_checked() {
        let number = sscc(1, "0614141", "123456789").unwrap();
        assert_eq!(number, "106141411234567897");
        assert_eq!(number.len(), 18);
        assert!(parse(&format!("(00){number}")).is_ok());
        assert_eq!(sscc(3, "0614141", "1").unwrap().len(), 18);
        assert!(sscc(10, "0614141", "1").is_err());
        assert!(sscc(0, "12345", "1").is_err(), "prefix too short");
        assert!(sscc(0, "0614141", "1234567890").is_err(), "serial too long");
        assert!(sscc(0, "0614141", "12x").is_err());
    }
}
