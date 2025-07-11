use async_graphql::connection::{query, Connection, Edge, EmptyFields};
use async_graphql::{Context, EmptySubscription, Error, Interface, Object, Result, Schema, ID};
use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal_macros::dec;
use serde::Deserialize;

use crate::dynamodb::{Client, Condition};
use crate::models::{
    Device, DynamoItem, Electricity, ElectricityInput, FinalElectricity, FinalElectricityInput,
    NodeId, Place, PlaceCondition, PlaceConditionInput,
};

fn sk_time(prefix: &str, time: Option<String>, after: bool) -> Result<String> {
    let delta = if after { 1 } else { -1 };
    let time = match time {
        Some(x) => DateTime::parse_from_rfc3339(&x)?.with_timezone(&Utc) + Duration::seconds(delta),
        None => {
            if after {
                Utc.ymd(0, 1, 1).and_hms(0, 0, 0)
            } else {
                Utc.ymd(9999, 12, 31).and_hms(23, 59, 59)
            }
        }
    };

    Ok(format!("{}{:?}", prefix, time))
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
    D: Deserialize<'de> + DynamoItem,
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
            connection.append(items.into_iter().map(|x| Edge::new(x.sk_value(), x)));
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
#[graphql(field(name = "id", type = "ID"))]
pub enum Node {
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
        let new = electricity(input);
        dynamodb.put_item(&new).await?;
        Ok(new)
    }

    async fn put_final_electricity(
        &self,
        ctx: &Context<'_>,
        input: FinalElectricityInput,
    ) -> Result<FinalElectricity> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let new = final_electricity(input);
        dynamodb.put_item(&new).await?;
        Ok(new)
    }

    async fn put_place_condition(
        &self,
        ctx: &Context<'_>,
        input: PlaceConditionInput,
    ) -> Result<PlaceCondition> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let new = place_condition(input);
        dynamodb.put_item(&new).await?;
        Ok(new)
    }

    async fn update_electricity(
        &self,
        ctx: &Context<'_>,
        input: ElectricityInput,
    ) -> Result<Electricity> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let new = dynamodb.update_item(&input).await?;
        Ok(new)
    }

    async fn update_final_electricity(
        &self,
        ctx: &Context<'_>,
        input: FinalElectricityInput,
    ) -> Result<FinalElectricity> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let new = dynamodb.update_item(&input).await?;
        Ok(new)
    }

    async fn update_place_condition(
        &self,
        ctx: &Context<'_>,
        input: PlaceConditionInput,
    ) -> Result<PlaceCondition> {
        let dynamodb = &ctx.data_unchecked::<Client>();
        let new = dynamodb.update_item(&input).await?;
        Ok(new)
    }
}

pub type HomeAPI = Schema<Query, Mutation, EmptySubscription>;

pub fn schema(dynamodb: Client) -> HomeAPI {
    Schema::build(Query, Mutation, EmptySubscription)
        .data(dynamodb)
        .finish()
}
