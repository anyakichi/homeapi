use async_graphql::*;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub trait DynamoItem {
    fn sk_prefix() -> String {
        "".to_owned()
    }

    fn pk(&self) -> String;

    fn sk_value(&self) -> String;

    fn sk(&self) -> String {
        format!("{}{}", Self::sk_prefix(), self.sk_value())
    }
}

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

impl DynamoItem for Device {
    fn pk(&self) -> String {
        self.pk.to_owned()
    }

    fn sk_value(&self) -> String {
        self.id.to_owned()
    }
}

#[Object]
impl Device {
    async fn id(&self) -> &str {
        self.id.as_str()
    }

    async fn place(&self) -> &str {
        self.place.as_str()
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

impl DynamoItem for Place {
    fn pk(&self) -> String {
        self.pk.to_owned()
    }

    fn sk_value(&self) -> String {
        self.id.to_owned()
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

impl DynamoItem for Electricity {
    fn sk_prefix() -> String {
        "TS#".to_owned()
    }

    fn pk(&self) -> String {
        self.id.to_owned()
    }

    fn sk_value(&self) -> String {
        format!("{:?}", &self.timestamp)
    }
}

#[Object]
impl Electricity {
    async fn id(&self) -> &str {
        self.id.as_str()
    }

    async fn timestamp(&self) -> String {
        format!("{:?}", &self.timestamp)
    }

    async fn place(&self) -> &str {
        self.place.as_str()
    }

    async fn cumulative_kwh_p(&self) -> String {
        format!("{}", &self.cumulative_kwh_p)
    }

    async fn cumulative_kwh_n(&self) -> String {
        format!("{}", &self.cumulative_kwh_n)
    }

    async fn current_w(&self) -> String {
        format!("{}", &self.current_w)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinalElectricity {
    #[serde(rename = "pk")]
    pub id: String,

    #[serde(rename = "sk")]
    #[serde(with = "dynamodb_fin_ts")]
    pub timestamp: DateTime<Utc>,

    #[serde(default)]
    pub place: String,

    pub cumulative_kwh_p: Decimal,
    pub cumulative_kwh_n: Decimal,
}

impl DynamoItem for FinalElectricity {
    fn sk_prefix() -> String {
        "FIN#TS#".to_owned()
    }

    fn pk(&self) -> String {
        self.id.to_owned()
    }

    fn sk_value(&self) -> String {
        format!("{:?}", &self.timestamp)
    }
}

#[Object]
impl FinalElectricity {
    async fn id(&self) -> &str {
        self.id.as_str()
    }

    async fn timestamp(&self) -> String {
        format!("{:?}", &self.timestamp)
    }

    async fn place(&self) -> &str {
        self.place.as_str()
    }

    async fn cumulative_kwh_p(&self) -> String {
        format!("{}", &self.cumulative_kwh_p)
    }

    async fn cumulative_kwh_n(&self) -> String {
        format!("{}", &self.cumulative_kwh_n)
    }
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

impl DynamoItem for PlaceCondition {
    fn sk_prefix() -> String {
        "TS#".to_owned()
    }

    fn pk(&self) -> String {
        self.id.to_owned()
    }

    fn sk_value(&self) -> String {
        format!("{:?}", self.timestamp)
    }
}

#[Object]
impl PlaceCondition {
    async fn id(&self) -> &str {
        self.id.as_str()
    }

    async fn timestamp(&self) -> String {
        format!("{:?}", &self.timestamp)
    }

    async fn place(&self) -> &str {
        self.place.as_str()
    }

    async fn temperature(&self) -> Option<String> {
        self.temperature.map(|x| format!("{}", &x))
    }

    async fn humidity(&self) -> Option<String> {
        self.humidity.map(|x| format!("{}", &x))
    }

    async fn illuminance(&self) -> Option<String> {
        self.illuminance.map(|x| format!("{}", &x))
    }

    async fn motion(&self) -> Option<String> {
        self.motion.map(|x| format!("{}", &x))
    }
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

mod dynamodb_fin_ts {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(timestamp: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("FIN#TS#{:?}", timestamp))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.strip_prefix("FIN#TS#") {
            Some(prefix) => prefix.parse().map_err(serde::de::Error::custom),
            None => Err(serde::de::Error::custom("Invalid prefix")),
        }
    }
}
