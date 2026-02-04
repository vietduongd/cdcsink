use serde_json::Value;

#[derive(Debug)]
pub struct DataModel {
    pub value: Value,
    pub data_type: String,
    pub nullable: bool,
    pub simple_type: String,
    // pub primary_key: bool,
}
