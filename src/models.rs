use anyhow::{Result, bail};
use async_graphql::{ID, InputObject, Object};
use base64::prelude::*;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub struct NodeId {
    pub prefix: String,
    pub pk: String,
    pub sk: String,
}

impl NodeId {
    pub fn global_id(prefix: &str, pk: &str, sk: &str) -> ID {
        BASE64_STANDARD_NO_PAD
            .encode(format!("{prefix}:{pk}:{sk}"))
            .into()
    }

    pub fn from_global_id(id: ID) -> Result<Self> {
        let id = String::from_utf8(BASE64_STANDARD_NO_PAD.decode(&*id)?)?;
        let v: Vec<&str> = id.splitn(3, ':').collect();
        if v.len() != 3 {
            bail!("Invalid Node ID");
        }
        Ok(Self {
            prefix: v[0].into(),
            pk: v[1].into(),
            sk: v[2].into(),
        })
    }

    pub fn to_global_id(&self) -> ID {
        Self::global_id(&self.prefix, &self.pk, &self.sk)
    }
}

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
    pub async fn id(&self) -> ID {
        NodeId::global_id("Device", "DEVICE", &self.id)
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

#[Object]
impl Place {
    pub async fn id(&self) -> ID {
        NodeId::global_id("Place", "PLACE", &self.id)
    }

    async fn name(&self) -> &str {
        self.name.as_str()
    }
}

#[derive(Debug, Serialize, InputObject)]
pub struct ElectricityInput {
    #[serde(rename = "pk")]
    pub device: String,

    #[serde(rename = "sk")]
    #[serde(with = "dynamodb_timestamp")]
    pub timestamp: DateTime<Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cumulative_kwh_p: Option<Decimal>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cumulative_kwh_n: Option<Decimal>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_w: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Electricity {
    #[serde(rename = "pk")]
    pub device: String,

    #[serde(rename = "sk")]
    #[serde(with = "dynamodb_timestamp")]
    pub timestamp: DateTime<Utc>,

    #[serde(default)]
    pub place: String,

    pub cumulative_kwh_p: Option<Decimal>,
    pub cumulative_kwh_n: Option<Decimal>,
    pub current_w: Option<u32>,
}

impl DynamoItem for Electricity {
    fn sk_prefix() -> String {
        "TS#".to_owned()
    }

    fn pk(&self) -> String {
        self.device.to_owned()
    }

    fn sk_value(&self) -> String {
        format!("{:?}", &self.timestamp)
    }
}

#[Object]
impl Electricity {
    pub async fn id(&self) -> ID {
        NodeId::global_id(
            "Electricity",
            &self.device,
            &format!("{:?}", &self.timestamp),
        )
    }

    async fn device(&self) -> &str {
        self.device.as_str()
    }

    async fn timestamp(&self) -> String {
        format!("{:?}", &self.timestamp)
    }

    async fn place(&self) -> &str {
        self.place.as_str()
    }

    async fn cumulative_kwh_p(&self) -> Option<String> {
        self.cumulative_kwh_p.map(|x| format!("{x}"))
    }

    async fn cumulative_kwh_n(&self) -> Option<String> {
        self.cumulative_kwh_n.map(|x| format!("{x}"))
    }

    async fn current_w(&self) -> Option<String> {
        self.current_w.map(|x| format!("{x}"))
    }
}

#[derive(Debug, Serialize, InputObject)]
pub struct FinalElectricityInput {
    #[serde(rename = "pk")]
    pub device: String,

    #[serde(rename = "sk")]
    #[serde(with = "dynamodb_timestamp")]
    pub timestamp: DateTime<Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cumulative_kwh_p: Option<Decimal>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cumulative_kwh_n: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalElectricity {
    #[serde(rename = "pk")]
    pub device: String,

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
        self.device.to_owned()
    }

    fn sk_value(&self) -> String {
        format!("{:?}", &self.timestamp)
    }
}

#[Object]
impl FinalElectricity {
    pub async fn id(&self) -> ID {
        NodeId::global_id(
            "FinalElectricity",
            &self.device,
            &format!("{:?}", &self.timestamp),
        )
    }

    async fn device(&self) -> &str {
        self.device.as_str()
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

#[derive(Debug, Serialize, InputObject)]
pub struct PlaceConditionInput {
    #[serde(rename = "pk")]
    pub device: String,

    #[serde(rename = "sk")]
    #[serde(with = "dynamodb_timestamp")]
    pub timestamp: DateTime<Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<Decimal>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub humidity: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub illuminance: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub motion: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceCondition {
    #[serde(rename = "pk")]
    pub device: String,

    #[serde(rename = "sk")]
    #[serde(with = "dynamodb_timestamp")]
    pub timestamp: DateTime<Utc>,

    pub place: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<Decimal>,

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
        self.device.to_owned()
    }

    fn sk_value(&self) -> String {
        format!("{:?}", self.timestamp)
    }
}

#[Object]
impl PlaceCondition {
    pub async fn id(&self) -> ID {
        NodeId::global_id(
            "PlaceCondition",
            &self.device,
            &format!("{:?}", &self.timestamp),
        )
    }

    async fn device(&self) -> &str {
        self.device.as_str()
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "pk")]
    pub email: String,

    #[serde(rename = "sk")]
    pub user_type: String, // Always "USER"
}

impl DynamoItem for User {
    fn pk(&self) -> String {
        self.email.clone()
    }

    fn sk_value(&self) -> String {
        "USER".to_owned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    #[serde(rename = "pk")]
    pub key_hash: String, // SHA256 hash of the actual API key

    #[serde(rename = "sk")]
    pub sk_value: String, // Always "APIKEY"

    pub user_email: String, // For GSI
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl ApiKey {
    pub fn new(email: String, key_hash: String, name: String) -> Self {
        Self {
            key_hash,
            sk_value: "APIKEY".to_string(),
            user_email: email,
            name,
            created_at: Utc::now(),
            last_used_at: None,
            expires_at: None,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            expires_at < Utc::now()
        } else {
            false
        }
    }
}

impl DynamoItem for ApiKey {
    fn pk(&self) -> String {
        self.key_hash.clone()
    }

    fn sk_value(&self) -> String {
        "APIKEY".to_string()
    }
}

#[Object]
impl ApiKey {
    pub async fn id(&self) -> ID {
        NodeId::global_id("ApiKey", &self.key_hash, "APIKEY")
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn created_at(&self) -> String {
        self.created_at.to_rfc3339()
    }

    async fn last_used_at(&self) -> Option<String> {
        self.last_used_at.map(|dt| dt.to_rfc3339())
    }

    async fn expires_at(&self) -> Option<String> {
        self.expires_at.map(|dt| dt.to_rfc3339())
    }
}

mod dynamodb_timestamp {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(timestamp: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("TS#{timestamp:?}"))
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
        serializer.serialize_str(&format!("FIN#TS#{timestamp:?}"))
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
