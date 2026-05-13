//! Shared formatter for numeric category values.
//!
//! Lives in `aglet-core` so both the TUI (board cells, previews) and the
//! CLI (`aglet show`) render numeric values identically.

use rust_decimal::Decimal;

use crate::model::NumericFormat;

/// Format a numeric value using the supplied `NumericFormat`.
///
/// Returns an en-dash placeholder when `value` is `None`.
pub fn format_numeric_cell(value: Option<Decimal>, format: Option<&NumericFormat>) -> String {
    let Some(v) = value else {
        return "\u{2013}".to_string();
    };
    let fmt = format.cloned().unwrap_or_default();
    let rounded = v.round_dp(fmt.decimal_places as u32);
    let raw = format!("{:.prec$}", rounded, prec = fmt.decimal_places as usize);

    let formatted = if fmt.use_thousands_separator {
        add_thousands_separator(&raw)
    } else {
        raw
    };

    match &fmt.currency_symbol {
        Some(sym) => format!("{sym}{formatted}"),
        None => formatted,
    }
}

fn add_thousands_separator(s: &str) -> String {
    let (integer_part, decimal_part) = match s.find('.') {
        Some(pos) => (&s[..pos], Some(&s[pos..])),
        None => (s, None),
    };
    let negative = integer_part.starts_with('-');
    let digits = if negative {
        &integer_part[1..]
    } else {
        integer_part
    };
    let mut result = String::new();
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    let reversed: String = result.chars().rev().collect();
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&reversed);
    if let Some(dec) = decimal_part {
        out.push_str(dec);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_numeric_cell_none_returns_dash() {
        assert_eq!(format_numeric_cell(None, None), "\u{2013}");
    }

    #[test]
    fn format_numeric_cell_default_format() {
        let result = format_numeric_cell(Some(Decimal::new(24596, 2)), None);
        assert_eq!(result, "245.96");
    }

    #[test]
    fn format_numeric_cell_with_currency_and_thousands() {
        let fmt = NumericFormat {
            decimal_places: 2,
            currency_symbol: Some("$".to_string()),
            use_thousands_separator: true,
        };
        let result = format_numeric_cell(Some(Decimal::new(123456789, 2)), Some(&fmt));
        assert_eq!(result, "$1,234,567.89");
    }

    #[test]
    fn format_numeric_cell_rounds_to_decimal_places() {
        let fmt = NumericFormat {
            decimal_places: 0,
            currency_symbol: None,
            use_thousands_separator: false,
        };
        let result = format_numeric_cell(Some(Decimal::new(2567, 2)), Some(&fmt));
        assert_eq!(result, "26");
    }

    #[test]
    fn format_numeric_cell_integer_shows_decimals() {
        let result = format_numeric_cell(Some(Decimal::new(42, 0)), None);
        assert_eq!(result, "42.00");
    }

    #[test]
    fn format_numeric_cell_negative_with_thousands() {
        let fmt = NumericFormat {
            decimal_places: 2,
            currency_symbol: None,
            use_thousands_separator: true,
        };
        let result = format_numeric_cell(Some(Decimal::new(-123456789, 2)), Some(&fmt));
        assert_eq!(result, "-1,234,567.89");
    }
}
