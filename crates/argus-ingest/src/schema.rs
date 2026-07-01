use arrow::array::Array;
use arrow::datatypes::{DataType, Field, Schema};
use argus_core::error::ArgusError;

/// The canonical schema version. Bumped on structural changes.
pub const SCHEMA_VERSION: u16 = 1;

/// Returns the canonical Arrow schema for normalized tick data.
pub fn canonical_schema() -> Schema {
    Schema::new(vec![
        Field::new("asset_id", DataType::UInt32, false),
        Field::new("timestamp_ns", DataType::Int64, false),
        Field::new("price", DataType::Float64, false),
        Field::new("volume", DataType::Float64, true),
        Field::new("source", DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)), false),
        Field::new("schema_version", DataType::UInt16, false),
    ])
}

/// Validates that a RecordBatch conforms to the canonical schema.
pub fn validate_batch(batch: &arrow::record_batch::RecordBatch) -> Result<(), ArgusError> {
    let expected = canonical_schema();
    let actual = batch.schema();

    if actual.fields().len() != expected.fields().len() {
        return Err(ArgusError::InvalidSchema(format!(
            "expected {} columns, got {}",
            expected.fields().len(),
            actual.fields().len()
        )));
    }

    for (i, expected_field) in expected.fields().iter().enumerate() {
        let actual_field = &actual.fields()[i];
        if actual_field.name() != expected_field.name() {
            return Err(ArgusError::InvalidSchema(format!(
                "field {}: expected name '{}', got '{}'",
                i,
                expected_field.name(),
                actual_field.name()
            )));
        }
        if actual_field.data_type() != expected_field.data_type() {
            return Err(ArgusError::InvalidSchema(format!(
                "field '{}': expected type {}, got {}",
                expected_field.name(),
                expected_field.data_type(),
                actual_field.data_type()
            )));
        }
    }

    // Check schema_version column values
    let version_col = batch
        .column(5)
        .as_any()
        .downcast_ref::<arrow::array::UInt16Array>()
        .ok_or_else(|| ArgusError::InvalidSchema("schema_version column is not UInt16".into()))?;

    for i in 0..version_col.len() {
        if version_col.is_null(i) {
            return Err(ArgusError::InvalidSchema("schema_version contains nulls".into()));
        }
        let val = version_col.value(i);
        if val != SCHEMA_VERSION {
            return Err(ArgusError::SchemaVersionMismatch {
                expected: SCHEMA_VERSION,
                actual: val,
            });
        }
    }

    Ok(())
}
