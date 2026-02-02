use cdc_postgres_destination::TypeMapping;

fn main() {
    println!("=== Debezium to PostgreSQL Type Mapping Examples ===\n");

    let mut mapping = TypeMapping::new();

    // Example 1: Basic type mappings
    println!("1. Basic Type Mappings:");
    println!("   int32 -> {}", mapping.map_type("int32", None));
    println!("   int64 -> {}", mapping.map_type("int64", None));
    println!("   string -> {}", mapping.map_type("string", None));
    println!("   boolean -> {}", mapping.map_type("boolean", None));
    println!("   float64 -> {}", mapping.map_type("float64", None));
    println!("   bytes -> {}", mapping.map_type("bytes", None));
    println!();

    // Example 2: Logical type mappings
    println!("2. Debezium Logical Types:");
    println!(
        "   int32 with io.debezium.time.Date -> {}",
        mapping.map_type("int32", Some("io.debezium.time.Date"))
    );
    println!(
        "   int64 with io.debezium.time.Timestamp -> {}",
        mapping.map_type("int64", Some("io.debezium.time.Timestamp"))
    );
    println!(
        "   string with io.debezium.data.Uuid -> {}",
        mapping.map_type("string", Some("io.debezium.data.Uuid"))
    );
    println!(
        "   string with io.debezium.data.Json -> {}",
        mapping.map_type("string", Some("io.debezium.data.Json"))
    );
    println!();

    // Example 3: Custom mappings
    println!("3. Custom Type Mappings:");
    mapping.add_custom_mapping("int32".to_string(), "INT".to_string());
    mapping.add_custom_mapping("my_custom_type".to_string(), "VARCHAR(255)".to_string());
    println!("   int32 (custom) -> {}", mapping.map_type("int32", None));
    println!("   my_custom_type -> {}", mapping.map_type("my_custom_type", None));
    println!();

    // Example 4: Complex types
    println!("4. Complex Types:");
    println!("   struct -> {}", mapping.map_type("struct", None));
    println!("   array -> {}", mapping.map_type("array", None));
    println!();

    // Example 5: All available mappings
    println!("5. All Available Mappings:");
    let all_mappings = mapping.get_all_mappings();
    for (debezium, postgres) in all_mappings.iter() {
        println!("   {} -> {}", debezium, postgres);
    }
}
