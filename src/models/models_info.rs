use serde::Deserialize;
use serde_json::Value;

#[derive(Debug)]
pub struct DataModel {
    pub value: Value,
    pub data_type: String,
    pub nullable: bool,
    pub simple_type: String,
    // pub primary_key: bool,
}

#[derive(Debug, Deserialize)]
pub struct DecimalModel {
    pub scale: i32,
    pub value: String,
}