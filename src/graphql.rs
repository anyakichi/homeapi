use async_graphql::connection::{query, Connection, Edge, EmptyFields};
use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Result, Schema};
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::Deserialize;

use crate::dynamodb::{Client, Condition};
use crate::models::{Device, DynamoItem, Electricity, FinalElectricity, PlaceCondition};

pub struct Query;

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
            Ok(connection)
        },
    )
    .await
}

#[Object]
impl Query {
    async fn device(&self, ctx: &Context<'_>, id: String) -> Result<Device> {
        Ok(ctx
            .data_unchecked::<Client>()
            .get_item("DEVICE", &id)
            .await?)
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

    async fn electricity(
        &self,
        ctx: &Context<'_>,
        id: String,
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
        get_items(dynamodb, &id, sk, None, None, first, last).await
    }

    async fn final_electricity(
        &self,
        ctx: &Context<'_>,
        id: String,
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
        get_items(dynamodb, &id, sk, None, None, first, last).await
    }

    async fn place_conditions(
        &self,
        ctx: &Context<'_>,
        id: String,
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
        get_items(dynamodb, &id, sk, None, None, first, last).await
    }
}

pub type HomeAPI = Schema<Query, EmptyMutation, EmptySubscription>;

pub fn schema(dynamodb: Client) -> HomeAPI {
    Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(dynamodb)
        .finish()
}
