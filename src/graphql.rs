use async_graphql::connection::{Connection, CursorType, Edge, EmptyFields, query};
use async_graphql::{Context, Error, ID, Interface, Object, Result, Schema, Subscription};
use chrono::{DateTime, Duration, TimeZone, Utc};
use futures_util::{Stream, StreamExt};
use rust_decimal_macros::dec;
use serde::Deserialize;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::auth::AuthUser;
use crate::dynamodb::{Client, Condition};
use crate::models::{
    ApiKey, Device, DynamoItem, Electricity, ElectricityInput, FinalElectricity,
    FinalElectricityInput, NodeId, Place, PlaceCondition, PlaceConditionInput,
};

fn sk_time(prefix: &str, time: Option<String>, after: bool) -> Result<String> {
    let delta = if after { 1 } else { -1 };
    let time = match time {
        Some(x) => DateTime::parse_from_rfc3339(&x)?.with_timezone(&Utc) + Duration::seconds(delta),
        None => {
            if after {
                Utc.with_ymd_and_hms(1, 1, 1, 0, 0, 0).unwrap()
            } else {
                Utc.with_ymd_and_hms(9999, 12, 31, 23, 59, 59).unwrap()
            }
        }
    };

    Ok(format!("{prefix}{time:?}"))
}

async fn get_items<'de, D>(
    dynamodb: &Client,
    pk: &str,
    sk: Option<Condition>,
    after: Option<String>,
    before: Option<String>,
    first: Option<i32>,
    last: Option<i32>,
) -> Result<Connection<String, D, EmptyFields, EmptyFields>>
where
    D: Deserialize<'de> + DynamoItem + async_graphql::OutputType,
    String: CursorType,
{
    query(
        after,
        before,
        first,
        last,
        |after, before, first, last| async move {
            let has_after = after.is_some();
            let has_before = before.is_some();
            let (items, next): (Vec<D>, _) = dynamodb
                .get_items(pk, sk, after, before, first, last)
                .await?;

            let has_prev = has_after || (last.is_some() && next.is_some());
            let has_next = has_before || (first.is_some() && next.is_some());
            let mut connection = Connection::new(has_prev, has_next);
            connection
                .edges
                .extend(items.into_iter().map(|x| Edge::new(x.sk_value(), x)));
            Ok::<_, Error>(connection)
        },
    )
    .await
}

fn electricity(input: ElectricityInput) -> Electricity {
    Electricity {
        device: input.device,
        timestamp: input.timestamp,
        place: input.place.unwrap_or("".to_owned()),
        cumulative_kwh_p: input.cumulative_kwh_p,
        cumulative_kwh_n: input.cumulative_kwh_n,
        current_w: input.current_w,
    }
}

fn final_electricity(input: FinalElectricityInput) -> FinalElectricity {
    FinalElectricity {
        device: input.device,
        timestamp: input.timestamp,
        place: input.place.unwrap_or("".to_owned()),
        cumulative_kwh_p: input.cumulative_kwh_p.unwrap_or(dec!(0.0)),
        cumulative_kwh_n: input.cumulative_kwh_n.unwrap_or(dec!(0.0)),
    }
}

fn place_condition(input: PlaceConditionInput) -> PlaceCondition {
    PlaceCondition {
        device: input.device,
        timestamp: input.timestamp,
        place: input.place.unwrap_or("".to_owned()),
        temperature: input.temperature,
        humidity: input.humidity,
        illuminance: input.illuminance,
        motion: input.motion,
    }
}

#[derive(Interface)]
#[graphql(field(name = "id", desc = "The ID of the node", ty = "ID"))]
pub enum Node {
    ApiKey(ApiKey),
    Device(Device),
    Electricity(Electricity),
    FinalElectricity(FinalElectricity),
    Place(Place),
    PlaceCondition(PlaceCondition),
}

pub struct Query;

#[Object]
impl Query {
    async fn node(&self, ctx: &Context<'_>, id: ID) -> Result<Node> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let node_id = NodeId::from_global_id(id)?;

        match node_id.prefix.as_ref() {
            "ApiKey" => {
                // For ApiKey nodes, pk is the key_hash and sk is "APIKEY"
                Ok(Node::ApiKey(
                    dynamodb.get_item(node_id.pk, "APIKEY".to_string()).await?,
                ))
            }
            "Device" => Ok(Node::Device(
                dynamodb.get_item(node_id.pk, node_id.sk).await?,
            )),
            "Place" => Ok(Node::Place(
                dynamodb.get_item(node_id.pk, node_id.sk).await?,
            )),
            "Electricity" => Ok(Node::Electricity(
                dynamodb.get_item(node_id.pk, node_id.sk).await?,
            )),
            "FinalElectricity" => Ok(Node::FinalElectricity(
                dynamodb.get_item(node_id.pk, node_id.sk).await?,
            )),
            "PlaceCondition" => Ok(Node::PlaceCondition(
                dynamodb.get_item(node_id.pk, node_id.sk).await?,
            )),
            _ => Err(Error::new("Invalid node prefix")),
        }
    }

    async fn devices(
        &self,
        ctx: &Context<'_>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<Connection<String, Device, EmptyFields, EmptyFields>> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        get_items(dynamodb, "DEVICE", None, after, before, first, last).await
    }

    async fn places(
        &self,
        ctx: &Context<'_>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<Connection<String, Place, EmptyFields, EmptyFields>> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        get_items(dynamodb, "PLACE", None, after, before, first, last).await
    }

    async fn electricity(
        &self,
        ctx: &Context<'_>,
        device: String,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<Connection<String, Electricity, EmptyFields, EmptyFields>> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let prefix = Electricity::sk_prefix();
        let sk = Some(Condition::Between(
            sk_time(&prefix, after, true)?,
            sk_time(&prefix, before, false)?,
        ));
        get_items(dynamodb, &device, sk, None, None, first, last).await
    }

    async fn final_electricity(
        &self,
        ctx: &Context<'_>,
        device: String,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<Connection<String, FinalElectricity, EmptyFields, EmptyFields>> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let prefix = FinalElectricity::sk_prefix();
        let sk = Some(Condition::Between(
            sk_time(&prefix, after, true)?,
            sk_time(&prefix, before, false)?,
        ));
        get_items(dynamodb, &device, sk, None, None, first, last).await
    }

    async fn place_conditions(
        &self,
        ctx: &Context<'_>,
        device: String,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<Connection<String, PlaceCondition, EmptyFields, EmptyFields>> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let prefix = PlaceCondition::sk_prefix();
        let sk = Some(Condition::Between(
            sk_time(&prefix, after, true)?,
            sk_time(&prefix, before, false)?,
        ));
        get_items(dynamodb, &device, sk, None, None, first, last).await
    }

    async fn api_keys(&self, ctx: &Context<'_>) -> Result<Vec<ApiKey>> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let auth_user = ctx.data_unchecked::<AuthUser>();

        // Query GSI by user_email
        let mut expression_attribute_values = std::collections::HashMap::new();
        expression_attribute_values.insert(
            ":email".to_string(),
            aws_sdk_dynamodb::types::AttributeValue::S(auth_user.email.clone()),
        );

        Ok(dynamodb
            .query_gsi::<ApiKey>(
                "user_email-index", // GSI name - you'll need to create this GSI
                "user_email = :email",
                expression_attribute_values,
            )
            .await?)
    }
}

pub struct Mutation;

#[Object]
impl Mutation {
    async fn put_electricity(
        &self,
        ctx: &Context<'_>,
        input: ElectricityInput,
    ) -> Result<Electricity> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let pubsub = &ctx.data_unchecked::<PubSub>();
        let new = electricity(input);
        dynamodb.put_item(&new).await?;
        pubsub.publish_electricity(new.clone());
        Ok(new)
    }

    async fn put_final_electricity(
        &self,
        ctx: &Context<'_>,
        input: FinalElectricityInput,
    ) -> Result<FinalElectricity> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let pubsub = &ctx.data_unchecked::<PubSub>();
        let new = final_electricity(input);
        dynamodb.put_item(&new).await?;
        pubsub.publish_final_electricity(new.clone());
        Ok(new)
    }

    async fn put_place_condition(
        &self,
        ctx: &Context<'_>,
        input: PlaceConditionInput,
    ) -> Result<PlaceCondition> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let pubsub = &ctx.data_unchecked::<PubSub>();
        let new = place_condition(input);
        dynamodb.put_item(&new).await?;
        pubsub.publish_place_condition(new.clone());
        Ok(new)
    }

    async fn update_electricity(
        &self,
        ctx: &Context<'_>,
        input: ElectricityInput,
    ) -> Result<Electricity> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let pubsub = &ctx.data_unchecked::<PubSub>();
        let new: Electricity = dynamodb.update_item(&input).await?;
        pubsub.publish_electricity(new.clone());
        Ok(new)
    }

    async fn update_final_electricity(
        &self,
        ctx: &Context<'_>,
        input: FinalElectricityInput,
    ) -> Result<FinalElectricity> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let pubsub = &ctx.data_unchecked::<PubSub>();
        let new: FinalElectricity = dynamodb.update_item(&input).await?;
        pubsub.publish_final_electricity(new.clone());
        Ok(new)
    }

    async fn update_place_condition(
        &self,
        ctx: &Context<'_>,
        input: PlaceConditionInput,
    ) -> Result<PlaceCondition> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let pubsub = &ctx.data_unchecked::<PubSub>();
        let new: PlaceCondition = dynamodb.update_item(&input).await?;
        pubsub.publish_place_condition(new.clone());
        Ok(new)
    }

    async fn create_api_key(
        &self,
        ctx: &Context<'_>,
        name: String,
        expires_at: Option<String>,
    ) -> Result<ApiKeyCreated> {
        use sha2::{Digest, Sha256};
        use uuid::Uuid;

        let dynamodb = &ctx.data_unchecked::<Client>();
        let auth_user = ctx.data_unchecked::<AuthUser>();

        // Generate API key: "ha_" + UUID v4 without hyphens
        let key_id = Uuid::new_v4().to_string().replace("-", "");
        let api_key = format!("ha_{key_id}");

        // Hash the API key
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        let key_hash = format!("{:x}", hasher.finalize());

        // Create the API key record
        let mut api_key_record = ApiKey::new(auth_user.email.clone(), key_hash, name);

        // Parse expires_at if provided
        if let Some(expires_str) = expires_at {
            api_key_record.expires_at =
                Some(DateTime::parse_from_rfc3339(&expires_str)?.with_timezone(&Utc));
        }

        // Save to database
        dynamodb.put_item(&api_key_record).await?;

        Ok(ApiKeyCreated {
            api_key_record,
            key: api_key, // Return the actual key only on creation
        })
    }

    async fn delete_api_key(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let auth_user = ctx.data_unchecked::<AuthUser>();
        let node_id = NodeId::from_global_id(id)?;

        if node_id.prefix != "ApiKey" {
            return Err(Error::new("Invalid node ID for API key"));
        }

        // Verify the API key belongs to the authenticated user
        let api_key: ApiKey = dynamodb
            .get_item(node_id.pk.clone(), "APIKEY".to_string())
            .await
            .map_err(|_| Error::new("API key not found"))?;

        if api_key.user_email != auth_user.email {
            return Err(Error::new("Unauthorized"));
        }

        // Delete the API key
        dynamodb.delete_item(&node_id.pk, "APIKEY").await?;
        Ok(true)
    }
}

#[derive(Clone)]
pub struct ApiKeyCreated {
    pub api_key_record: ApiKey,
    pub key: String,
}

#[Object]
impl ApiKeyCreated {
    async fn api_key(&self) -> &ApiKey {
        &self.api_key_record
    }

    async fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Clone)]
pub struct PubSub {
    electricity_sender: broadcast::Sender<Electricity>,
    final_electricity_sender: broadcast::Sender<FinalElectricity>,
    place_condition_sender: broadcast::Sender<PlaceCondition>,
}

impl Default for PubSub {
    fn default() -> Self {
        Self::new()
    }
}

impl PubSub {
    pub fn new() -> Self {
        let (electricity_sender, _) = broadcast::channel(100);
        let (final_electricity_sender, _) = broadcast::channel(100);
        let (place_condition_sender, _) = broadcast::channel(100);

        Self {
            electricity_sender,
            final_electricity_sender,
            place_condition_sender,
        }
    }

    pub fn publish_electricity(&self, electricity: Electricity) {
        let _ = self.electricity_sender.send(electricity);
    }

    pub fn publish_final_electricity(&self, final_electricity: FinalElectricity) {
        let _ = self.final_electricity_sender.send(final_electricity);
    }

    pub fn publish_place_condition(&self, place_condition: PlaceCondition) {
        let _ = self.place_condition_sender.send(place_condition);
    }

    pub fn subscribe_electricity(&self) -> BroadcastStream<Electricity> {
        BroadcastStream::new(self.electricity_sender.subscribe())
    }

    pub fn subscribe_final_electricity(&self) -> BroadcastStream<FinalElectricity> {
        BroadcastStream::new(self.final_electricity_sender.subscribe())
    }

    pub fn subscribe_place_condition(&self) -> BroadcastStream<PlaceCondition> {
        BroadcastStream::new(self.place_condition_sender.subscribe())
    }
}

pub struct Subscription;

#[Subscription]
impl Subscription {
    async fn electricity_updated(
        &self,
        ctx: &Context<'_>,
        device: Option<String>,
    ) -> impl Stream<Item = Electricity> {
        let pubsub = ctx.data_unchecked::<PubSub>();
        pubsub.subscribe_electricity().filter_map(move |result| {
            let device = device.clone();
            async move {
                result.ok().and_then(|x| {
                    if device.is_some_and(|d| x.device != d) {
                        None
                    } else {
                        Some(x)
                    }
                })
            }
        })
    }

    async fn final_electricity_updated(
        &self,
        ctx: &Context<'_>,
        device: Option<String>,
    ) -> impl Stream<Item = FinalElectricity> {
        let pubsub = ctx.data_unchecked::<PubSub>();
        pubsub
            .subscribe_final_electricity()
            .filter_map(move |result| {
                let device = device.clone();
                async move {
                    result.ok().and_then(|x| {
                        if device.is_some_and(|d| x.device != d) {
                            None
                        } else {
                            Some(x)
                        }
                    })
                }
            })
    }

    async fn place_condition_updated(
        &self,
        ctx: &Context<'_>,
        device: Option<String>,
    ) -> impl Stream<Item = PlaceCondition> {
        let pubsub = ctx.data_unchecked::<PubSub>();
        pubsub
            .subscribe_place_condition()
            .filter_map(move |result| {
                let device = device.clone();
                async move {
                    result.ok().and_then(|x| {
                        if device.is_some_and(|d| x.device != d) {
                            None
                        } else {
                            Some(x)
                        }
                    })
                }
            })
    }
}

pub type HomeAPI = Schema<Query, Mutation, Subscription>;

pub fn schema(dynamodb: Client, pubsub: PubSub) -> HomeAPI {
    Schema::build(Query, Mutation, Subscription)
        .data(dynamodb)
        .data(pubsub)
        .finish()
}
