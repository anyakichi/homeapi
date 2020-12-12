use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RawData {
    pk: String,

    #[serde(rename = "sk")]
    pub id: String,

    pub body: String,
}

impl RawData {
    pub fn new(id: String) -> Self {
        Self {
            pk: "RAW_DATA".to_owned(),
            id,
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Device {
    pk: String,

    #[serde(rename = "sk")]
    pub id: String,

    pub place: String,
}

impl Device {
    pub fn new(id: String) -> Self {
        Self {
            pk: "DEVICE".to_owned(),
            id,
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Place {
    pk: String,

    #[serde(rename = "sk")]
    pub id: String,

    pub name: String,
}

impl Place {
    pub fn new(id: String) -> Self {
        Self {
            pk: "PLACE".to_owned(),
            id,
            ..Default::default()
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Electricity {
    #[serde(rename = "pk")]
    pub id: String,

    #[serde(rename = "sk")]
    #[serde(with = "dynamodb_timestamp")]
    pub timestamp: DateTime<Utc>,

    #[serde(default)]
    pub place: String,

    pub cumulative_kwh_p: Decimal,
    pub cumulative_kwh_n: Decimal,
    pub current_w: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlaceCondition {
    #[serde(rename = "pk")]
    pub id: String,

    #[serde(rename = "sk")]
    #[serde(with = "dynamodb_timestamp")]
    pub timestamp: DateTime<Utc>,

    pub place: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub humidity: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub illuminance: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub motion: Option<i64>,
}

mod dynamodb_timestamp {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(timestamp: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("TS#{:?}", timestamp))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.strip_prefix("TS#") {
            Some(prefix) => prefix.parse().map_err(serde::de::Error::custom),
            None => Err(serde::de::Error::custom("Invalid prefix")),
        }
    }
}
