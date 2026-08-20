//! Currency formatting utility (Phase 6, `plan/phases/phase-6-business-
//! polish.md` step 4). Pure function, no layout involvement — a small,
//! documented convenience for the single most common German business-
//! document formatting need, not a general locale/i18n system.

/// Formats a cent amount as German currency: thousands-grouped with `.`,
/// decimal comma, trailing `€` — e.g. `format_currency_de(123456) ==
/// "1.234,56 €"`.
pub fn format_currency_de(cents: i64) -> String {
    let negative = cents < 0;
    let abs = cents.unsigned_abs();
    let euros = abs / 100;
    let rest = abs % 100;
    let sign = if negative { "-" } else { "" };
    format!("{sign}{},{rest:02} \u{20ac}", group_thousands_de(euros))
}

fn group_thousands_de(n: u64) -> String {
    let digits = n.to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_typical_invoice_amounts() {
        assert_eq!(format_currency_de(123456), "1.234,56 \u{20ac}");
        assert_eq!(format_currency_de(0), "0,00 \u{20ac}");
        assert_eq!(format_currency_de(5), "0,05 \u{20ac}");
        assert_eq!(format_currency_de(100), "1,00 \u{20ac}");
    }

    #[test]
    fn groups_large_amounts() {
        assert_eq!(format_currency_de(12_345_678_900), "123.456.789,00 \u{20ac}");
        assert_eq!(format_currency_de(100_000_000), "1.000.000,00 \u{20ac}");
        assert_eq!(format_currency_de(99_999), "999,99 \u{20ac}");
    }

    #[test]
    fn formats_negative_amounts() {
        assert_eq!(format_currency_de(-50), "-0,50 \u{20ac}");
        assert_eq!(format_currency_de(-123456), "-1.234,56 \u{20ac}");
    }
}
