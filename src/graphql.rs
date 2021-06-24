use async_graphql::{
    Context, EmptyMutation, EmptySubscription, FieldResult, Object, Result, Schema,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::dynamodb::Client;
use crate::models::{Device, Electricity, FinalElectricity, PlaceCondition};

pub struct Query;

async fn get_entries<'de, D>(
    dynamodb: &Client,
    id: String,
    prefix: &str,
    start: Option<String>,
    end: Option<String>,
) -> Result<Vec<D>>
where
    D: Deserialize<'de>,
{
    let entries: Vec<D> = dynamodb
        .get_entries(
            &id,
            prefix,
            start
                .and_then(|x| Some(DateTime::parse_from_rfc3339(&x).ok()?.with_timezone(&Utc)))
                .as_ref(),
            end.and_then(|x| Some(DateTime::parse_from_rfc3339(&x).ok()?.with_timezone(&Utc)))
                .as_ref(),
        )
        .await?;
    Ok(entries)
}

#[Object]
impl Query {
    async fn device(&self, ctx: &Context<'_>, id: String) -> Result<Device> {
        Ok(ctx.data_unchecked::<Client>().get_device(&id).await?)
    }

    async fn devices(&self, ctx: &Context<'_>) -> Result<Vec<Device>> {
        Ok(ctx.data_unchecked::<Client>().get_devices().await?)
    }

    async fn electricity(
        &self,
        ctx: &Context<'_>,
        id: String,
        start: Option<String>,
        end: Option<String>,
    ) -> FieldResult<Vec<Electricity>> {
        get_entries(&ctx.data_unchecked::<Client>(), id, "TS#", start, end).await
    }

    async fn final_electricity(
        &self,
        ctx: &Context<'_>,
        id: String,
        start: Option<String>,
        end: Option<String>,
    ) -> FieldResult<Vec<FinalElectricity>> {
        get_entries(&ctx.data_unchecked::<Client>(), id, "FIN#TS#", start, end).await
    }

    async fn place_conditions(
        &self,
        ctx: &Context<'_>,
        id: String,
        start: Option<String>,
        end: Option<String>,
    ) -> FieldResult<Vec<PlaceCondition>> {
        get_entries(&ctx.data_unchecked::<Client>(), id, "TS#", start, end).await
    }
}

pub type HomeAPI = Schema<Query, EmptyMutation, EmptySubscription>;

pub fn schema(dynamodb: Client) -> HomeAPI {
    Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(dynamodb)
        .finish()
}
