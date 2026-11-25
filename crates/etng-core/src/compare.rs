//! Structural comparison of exiftool JSON documents, used by the golden
//! corpus diff.

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct FieldDiff {
    pub key: String,
    pub expected: Value,
    pub actual: Value,
}

#[derive(Debug, Default)]
pub struct DiffReport {
    pub only_expected: Vec<String>,
    pub only_actual: Vec<String>,
    pub changed: Vec<FieldDiff>,
}

impl DiffReport {
    pub fn is_empty(&self) -> bool {
        self.only_expected.is_empty() && self.only_actual.is_empty() && self.changed.is_empty()
    }
}

/// Compare two JSON objects (arrays are not expected here).
pub fn compare_documents(expected: &Value, actual: &Value, mask_suffixes: &[&str]) -> DiffReport {
    let mut report = DiffReport::default();
    let (Some(expected), Some(actual)) = (expected.as_object(), actual.as_object()) else {
        return report;
    };
    for (key, expected_value) in expected {
        if masked(key, mask_suffixes) {
            continue;
        }
        match actual.get(key) {
            Some(actual_value) if !values_equal(actual_value, expected_value) => {
                report.changed.push(FieldDiff {
                    key: key.clone(),
                    expected: expected_value.clone(),
                    actual: actual_value.clone(),
                });
            }
            None => report.only_expected.push(key.clone()),
            _ => {}
        }
    }
    for key in actual.keys() {
        if masked(key, mask_suffixes) {
            continue;
        }
        if !expected.contains_key(key) {
            report.only_actual.push(key.clone());
        }
    }
    report
}

fn masked(key: &str, suffixes: &[&str]) -> bool {
    suffixes.iter().any(|suffix| key.ends_with(suffix))
}

/// JSON numbers compare numerically: `7` and `7.0` are the same value.
fn values_equal(a: &Value, b: &Value) -> bool {
    if a == b {
        return true;
    }
    match (a.as_f64(), b.as_f64()) {
        (Some(left), Some(right)) => (left - right).abs() <= 1e-9 * left.abs().max(1.0),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reports_missing_extra_and_changed_fields() {
        let expected = json!({"Make": "Canon", "Model": "R5", "FileAccessDate": "x"});
        let actual = json!({"Make": "Nikon", "Lens": "RF", "FileAccessDate": "y"});
        let report = compare_documents(&expected, &actual, &["FileAccessDate"]);
        assert_eq!(report.changed.len(), 1);
        assert_eq!(report.changed[0].key, "Make");
        assert_eq!(report.only_expected, vec!["Model"]);
        assert_eq!(report.only_actual, vec!["Lens"]);
        assert!(!report.is_empty());
    }

    #[test]
    fn equal_documents_are_empty() {
        let expected = json!({"Make": "Canon"});
        let report = compare_documents(&expected, &expected, &[]);
        assert!(report.is_empty());
    }
}
