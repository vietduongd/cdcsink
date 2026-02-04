
use sqlx::PgPool;

pub struct PostgresDestination {
    pub database_url: String,
}

impl PostgresDestination {
    pub fn new(database_url: String) -> Self {
        PostgresDestination { database_url }
    }

    async fn ensure_schema_metadata_table(&self, pool: &PgPool) -> Result<(), String> {
        let query = format!(
            "CREATE TABLE IF NOT EXISTS {}.\"_cdc_schema_metadata\" (
                schema_name TEXT NOT NULL,
                table_name TEXT NOT NULL,
                column_name TEXT NOT NULL,
                data_type TEXT NOT NULL,
                last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
                PRIMARY KEY (schema_name, table_name, column_name)
            )",
            "public"
        );

       let _result = sqlx::query(&query)
            .execute(pool)
            .await
            .map_err(|e| format!("Can not create schema metadata table: {}", e));

        Ok(())
    }

    fn quote_identifier(identifier: &str) -> String {
        // Check if identifier contains uppercase letters
        if identifier.chars().any(|c| c.is_uppercase()) {
            format!("\"{}\"", identifier)
        } else {
            identifier.to_string()
        }
    }
}
