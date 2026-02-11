//! Test helper functions for working with FieldStorage-based DataRecord

#[cfg(test)]
use wp_model_core::model::{DataField, DataRecord, FieldStorage};

/// Quick constructor for test DataRecord from Vec<DataField>
///
/// # Example
/// ```
/// let record = test_record(vec![
///     DataField::from_chars("name", "value"),
///     DataField::from_digit("count", 42),
/// ]);
/// ```
#[cfg(test)]
#[allow(dead_code)]
pub fn test_record(fields: Vec<DataField>) -> DataRecord {
    let storage: Vec<FieldStorage> = fields.into_iter().map(FieldStorage::from_owned).collect();
    DataRecord::from(storage)
}

/// Assert that a field exists and matches the expected value
///
/// # Example
/// ```
/// assert_field_eq!(record, "name", DataField::from_chars("name", "value"));
/// ```
#[cfg(test)]
#[macro_export]
macro_rules! assert_field_eq {
    ($record:expr, $name:expr, $expected:expr) => {
        let expected = $expected;
        assert_eq!(
            $record.field($name).map(|s| s.as_field()),
            Some(&expected),
            "Field '{}' mismatch",
            $name
        )
    };
}

/// Assert that a field exists
#[cfg(test)]
#[macro_export]
macro_rules! assert_field_exists {
    ($record:expr, $name:expr) => {
        assert!(
            $record.field($name).is_some(),
            "Field '{}' not found",
            $name
        )
    };
}

/// Get field value for assertion
#[cfg(test)]
#[allow(dead_code)]
pub fn get_field<'a>(record: &'a DataRecord, name: &str) -> Option<&'a DataField> {
    record.field(name).map(|s| s.as_field())
}
