// Cron parsing / validation / next-occurrence utilities.
//
// Wraps the `croner` crate so the rest of the app only ever sees the project's
// unified `AppError`. Schedules are evaluated in the system local time zone
// (`chrono::Local`); occurrences are returned as millisecond `Timestamp`s, the
// same unit the `jobs` table stores. Standard 5-field cron patterns are the
// contract, but croner's extended syntax (6/7 fields, named days/months,
// `@nicknames`, etc.) is tolerated rather than rejected.

use std::str::FromStr;

use chrono::Local;
use croner::Cron;

use crate::models::AppError;
use crate::storage::types::Timestamp;

/// Maximum number of occurrences `job_preview_schedule` returns by default.
pub const DEFAULT_PREVIEW_COUNT: usize = 5;

/// Parse a cron expression, mapping any croner error into a `VALIDATION_ERROR`
/// `AppError`. Returns the parsed `Cron` for callers that go on to compute
/// occurrences, so the expression is only parsed once.
fn parse(cron: &str) -> Result<Cron, AppError> {
    Cron::from_str(cron).map_err(|e| {
        AppError::with_hint(
            "VALIDATION_ERROR",
            &format!("Invalid cron expression: {e}"),
            "请检查 cron 表达式（标准 5 段：分 时 日 月 周）",
        )
    })
}

/// Validate a cron expression. `Ok(())` when croner accepts it, otherwise a
/// structured `VALIDATION_ERROR`.
pub fn validate(cron: &str) -> Result<(), AppError> {
    parse(cron).map(|_| ())
}

/// Compute the next `n` occurrences of `cron`, each strictly after "now" in the
/// system local time zone, as millisecond timestamps in ascending order.
///
/// - Invalid cron -> `VALIDATION_ERROR`.
/// - Sparse schedules whose visible future yields fewer than `n` occurrences
///   return the real count (possibly empty) rather than padding or erroring;
///   croner's internal search has a bounded horizon, so this also can't hang.
pub fn next_occurrences(cron: &str, n: usize) -> Result<Vec<Timestamp>, AppError> {
    let schedule = parse(cron)?;
    let mut occurrences = Vec::with_capacity(n);

    // Start strictly after the current local instant and step forward, feeding
    // each match back in as the next exclusive lower bound.
    let mut cursor = Local::now();
    for _ in 0..n {
        match schedule.find_next_occurrence(&cursor, false) {
            Ok(next) => {
                occurrences.push(next.timestamp_millis());
                cursor = next;
            }
            // No further occurrence within croner's search horizon (e.g. a
            // sparse pattern whose remaining matches lie beyond it). Return
            // what we have rather than failing.
            Err(_) => break,
        }
    }

    Ok(occurrences)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    #[test]
    fn valid_cron_passes_validation() {
        assert!(validate("0 9 * * *").is_ok());
        assert!(validate("*/5 * * * *").is_ok());
        // Named day-of-week (croner extended syntax) is tolerated.
        assert!(validate("0 0 * * MON").is_ok());
    }

    #[test]
    fn invalid_cron_returns_structured_validation_error() {
        let err = validate("not a cron").expect_err("should reject garbage");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(!err.message.is_empty());
        assert!(err.hint.is_some());

        // Out-of-range field is also rejected with the same shape.
        let err = validate("99 * * * *").expect_err("minute out of range");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(err.hint.is_some());
    }

    #[test]
    fn next_occurrences_returns_five_increasing_future_times() {
        let now_ms = Local::now().timestamp_millis();
        // Every minute -> always five upcoming occurrences.
        let times = next_occurrences("* * * * *", 5).expect("valid cron");
        assert_eq!(times.len(), 5);

        // Each occurrence is strictly after "now" and strictly increasing.
        for &t in &times {
            assert!(t > now_ms, "occurrence {t} must be after now {now_ms}");
        }
        for window in times.windows(2) {
            assert!(
                window[1] > window[0],
                "occurrences must be strictly increasing: {} !< {}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn next_occurrences_invalid_cron_returns_validation_error() {
        let err = next_occurrences("definitely-not-cron", 5).expect_err("invalid cron");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(err.hint.is_some());
    }

    #[test]
    fn sparse_cron_returns_fewer_than_requested_without_error() {
        use chrono::Datelike;

        // A 7-field pattern (croner's extended syntax) pinned to an explicit set
        // of calendar years has only finitely many future occurrences. We build
        // it from the next three years relative to now so the test never expires:
        // "midnight on Jan 1 of year Y" for three specific Y -> exactly three
        // future occurrences, fewer than the five requested, no error, no hang.
        let next_year = Local::now().year() + 1;
        let years = format!("{},{},{}", next_year, next_year + 1, next_year + 2);
        let pattern = format!("0 0 0 1 1 ? {years}"); // sec min hour dom month dow year

        let times = next_occurrences(&pattern, 5).expect("valid sparse cron");
        assert_eq!(
            times.len(),
            3,
            "sparse cron pinned to 3 future years should yield 3 occurrences, got {}",
            times.len()
        );

        // Whatever it returns must still be future + strictly increasing.
        let now_ms = Local::now().timestamp_millis();
        for &t in &times {
            assert!(t > now_ms);
        }
        for window in times.windows(2) {
            assert!(window[1] > window[0]);
        }
    }

    #[test]
    fn respects_requested_count() {
        let times = next_occurrences("* * * * *", 3).expect("valid cron");
        assert_eq!(times.len(), 3);
    }
}
