use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for mapping Debezium schema types to PostgreSQL types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeMapping {
    /// Custom type mappings that override defaults
    #[serde(default)]
    pub custom_mappings: HashMap<String, String>,
}

impl Default for TypeMapping {
    fn default() -> Self {
        Self {
            custom_mappings: HashMap::new(),
        }
    }
}

impl TypeMapping {
    /// Create a new TypeMapping with default mappings
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the default Debezium to PostgreSQL type mapping
    pub fn get_default_mappings() -> HashMap<String, String> {
        let mut mappings = HashMap::new();

        // Integer types
        mappings.insert("int8".to_string(), "SMALLINT".to_string());
        mappings.insert("int16".to_string(), "SMALLINT".to_string());
        mappings.insert("int32".to_string(), "INTEGER".to_string());
        mappings.insert("int64".to_string(), "BIGINT".to_string());

        // Floating point types
        mappings.insert("float".to_string(), "REAL".to_string());
        mappings.insert("float32".to_string(), "REAL".to_string());
        mappings.insert("float64".to_string(), "DOUBLE PRECISION".to_string());
        mappings.insert("double".to_string(), "DOUBLE PRECISION".to_string());

        // String types
        mappings.insert("string".to_string(), "TEXT".to_string());

        // Boolean
        mappings.insert("boolean".to_string(), "BOOLEAN".to_string());
        mappings.insert("bool".to_string(), "BOOLEAN".to_string());

        // Binary
        mappings.insert("bytes".to_string(), "BYTEA".to_string());

        // Debezium logical types (với name parameter)
        mappings.insert("io.debezium.time.Date".to_string(), "DATE".to_string());
        mappings.insert(
            "io.debezium.time.Time".to_string(),
            "TIME".to_string(),
        );
        mappings.insert(
            "io.debezium.time.MicroTime".to_string(),
            "TIME".to_string(),
        );
        mappings.insert(
            "io.debezium.time.NanoTime".to_string(),
            "TIME".to_string(),
        );
        mappings.insert(
            "io.debezium.time.Timestamp".to_string(),
            "TIMESTAMP".to_string(),
        );
        mappings.insert(
            "io.debezium.time.MicroTimestamp".to_string(),
            "TIMESTAMP".to_string(),
        );
        mappings.insert(
            "io.debezium.time.NanoTimestamp".to_string(),
            "TIMESTAMP".to_string(),
        );
        mappings.insert(
            "io.debezium.time.ZonedTimestamp".to_string(),
            "TIMESTAMP WITH TIME ZONE".to_string(),
        );

        // Decimal/Numeric
        mappings.insert(
            "io.debezium.data.VariableScaleDecimal".to_string(),
            "NUMERIC".to_string(),
        );
        mappings.insert("org.apache.kafka.connect.data.Decimal".to_string(), "NUMERIC".to_string());

        // JSON
        mappings.insert("io.debezium.data.Json".to_string(), "JSONB".to_string());

        // UUID
        mappings.insert("io.debezium.data.Uuid".to_string(), "UUID".to_string());

        // Enum
        mappings.insert("io.debezium.data.Enum".to_string(), "TEXT".to_string());

        // Geometry (PostGIS)
        mappings.insert(
            "io.debezium.data.geometry.Point".to_string(),
            "POINT".to_string(),
        );
        mappings.insert(
            "io.debezium.data.geometry.Geometry".to_string(),
            "GEOMETRY".to_string(),
        );

        // XML
        mappings.insert("io.debezium.data.Xml".to_string(), "XML".to_string());

        mappings
    }

    /// Map a Debezium schema type to PostgreSQL type
    /// 
    /// # Arguments
    /// * `debezium_type` - The Debezium field type (e.g., "int32", "string")
    /// * `debezium_name` - Optional Debezium logical type name (e.g., "io.debezium.time.Date")
    /// 
    /// # Returns
    /// PostgreSQL type name
    pub fn map_type(&self, debezium_type: &str, debezium_name: Option<&str>) -> String {
        // First, check if there's a logical type name
        if let Some(name) = debezium_name {
            // Check custom mappings first
            if let Some(pg_type) = self.custom_mappings.get(name) {
                return pg_type.clone();
            }
            // Check default mappings
            if let Some(pg_type) = Self::get_default_mappings().get(name) {
                return pg_type.clone();
            }
        }

        // Check custom mappings for base type
        if let Some(pg_type) = self.custom_mappings.get(debezium_type) {
            return pg_type.clone();
        }

        // Check default mappings for base type
        if let Some(pg_type) = Self::get_default_mappings().get(debezium_type) {
            return pg_type.clone();
        }

        // Handle struct types (nested objects)
        if debezium_type == "struct" {
            return "JSONB".to_string();
        }

        // Handle array types
        if debezium_type == "array" {
            return "JSONB".to_string();
        }

        // Default fallback
        "TEXT".to_string()
    }

    /// Add a custom type mapping
    pub fn add_custom_mapping(&mut self, debezium_type: String, postgres_type: String) {
        self.custom_mappings.insert(debezium_type, postgres_type);
    }

    /// Get all available mappings (default + custom)
    pub fn get_all_mappings(&self) -> HashMap<String, String> {
        let mut all = Self::get_default_mappings();
        all.extend(self.custom_mappings.clone());
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mappings() {
        let mapping = TypeMapping::new();
        
        assert_eq!(mapping.map_type("int32", None), "INTEGER");
        assert_eq!(mapping.map_type("int64", None), "BIGINT");
        assert_eq!(mapping.map_type("string", None), "TEXT");
        assert_eq!(mapping.map_type("boolean", None), "BOOLEAN");
        assert_eq!(mapping.map_type("float64", None), "DOUBLE PRECISION");
    }

    #[test]
    fn test_logical_types() {
        let mapping = TypeMapping::new();
        
        assert_eq!(
            mapping.map_type("int32", Some("io.debezium.time.Date")),
            "DATE"
        );
        assert_eq!(
            mapping.map_type("int64", Some("io.debezium.time.Timestamp")),
            "TIMESTAMP"
        );
        assert_eq!(
            mapping.map_type("string", Some("io.debezium.data.Uuid")),
            "UUID"
        );
    }

    #[test]
    fn test_custom_mappings() {
        let mut mapping = TypeMapping::new();
        mapping.add_custom_mapping("int32".to_string(), "INT".to_string());
        
        assert_eq!(mapping.map_type("int32", None), "INT");
        assert_eq!(mapping.map_type("string", None), "TEXT");
    }

    #[test]
    fn test_struct_and_array() {
        let mapping = TypeMapping::new();
        
        assert_eq!(mapping.map_type("struct", None), "JSONB");
        assert_eq!(mapping.map_type("array", None), "JSONB");
    }

    #[test]
    fn test_unknown_type_fallback() {
        let mapping = TypeMapping::new();
        
        assert_eq!(mapping.map_type("unknown_type", None), "TEXT");
    }
}
