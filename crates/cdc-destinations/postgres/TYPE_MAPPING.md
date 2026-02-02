# Type Mapping Configuration

## Debezium to PostgreSQL Type Mapping

File này cung cấp cấu hình mapping giữa các kiểu dữ liệu Debezium schema và PostgreSQL.

### Default Mappings

#### Số nguyên (Integer Types)
- `int8`, `int16` → `SMALLINT`
- `int32` → `INTEGER`
- `int64` → `BIGINT`

#### Số thực (Floating Point Types)
- `float`, `float32` → `REAL`
- `float64`, `double` → `DOUBLE PRECISION`

#### Chuỗi (String Types)
- `string` → `TEXT`

#### Boolean
- `boolean`, `bool` → `BOOLEAN`

#### Binary
- `bytes` → `BYTEA`

#### Date/Time Types (Debezium Logical Types)
- `io.debezium.time.Date` → `DATE`
- `io.debezium.time.Time` → `TIME`
- `io.debezium.time.MicroTime` → `TIME`
- `io.debezium.time.NanoTime` → `TIME`
- `io.debezium.time.Timestamp` → `TIMESTAMP`
- `io.debezium.time.MicroTimestamp` → `TIMESTAMP`
- `io.debezium.time.NanoTimestamp` → `TIMESTAMP`
- `io.debezium.time.ZonedTimestamp` → `TIMESTAMP WITH TIME ZONE`

#### Decimal/Numeric
- `io.debezium.data.VariableScaleDecimal` → `NUMERIC`
- `org.apache.kafka.connect.data.Decimal` → `NUMERIC`

#### JSON
- `io.debezium.data.Json` → `JSONB`

#### UUID
- `io.debezium.data.Uuid` → `UUID`

#### Enum
- `io.debezium.data.Enum` → `TEXT`

#### Geometry (PostGIS)
- `io.debezium.data.geometry.Point` → `POINT`
- `io.debezium.data.geometry.Geometry` → `GEOMETRY`

#### XML
- `io.debezium.data.Xml` → `XML`

#### Complex Types
- `struct` → `JSONB`
- `array` → `JSONB`

### Custom Mappings

Bạn có thể thêm custom mappings trong config:

```json
{
  "type_mapping": {
    "custom_mappings": {
      "int32": "INT",
      "my_special_type": "VARCHAR(255)"
    }
  }
}
```

### Usage Example

```rust
use cdc_postgres_destination::TypeMapping;

let mut mapping = TypeMapping::new();

// Basic mapping
let pg_type = mapping.map_type("int32", None);
// Returns: "INTEGER"

// Logical type mapping
let pg_type = mapping.map_type("int32", Some("io.debezium.time.Date"));
// Returns: "DATE"

// Custom mapping
mapping.add_custom_mapping("int32".to_string(), "INT".to_string());
let pg_type = mapping.map_type("int32", None);
// Returns: "INT"
```

### Configuration trong PostgresConfig

```json
{
  "url": "postgresql://user:pass@localhost:5432/db",
  "schema": "public",
  "type_mapping": {
    "custom_mappings": {
      "int32": "INT",
      "custom_type": "VARCHAR(100)"
    }
  }
}
```

### Run Example

```bash
cargo run --example type_mapping_example --package cdc-postgres-destination
```
