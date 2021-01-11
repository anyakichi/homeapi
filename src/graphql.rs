use chrono::{DateTime, Utc};
use juniper::{graphql_object, EmptyMutation, EmptySubscription, FieldResult};
use serde::Deserialize;

use crate::dynamodb::Client;
use crate::models::{Device, Electricity, PlaceCondition};

pub struct Context {
    pub dynamodb: Client,
}

impl juniper::Context for Context {}

pub struct Query;

async fn get_entries<'de, D>(
    dynamodb: &Client,
    id: String,
    start: Option<String>,
    end: Option<String>,
) -> FieldResult<Vec<D>>
where
    D: Deserialize<'de>,
{
    let entries: Vec<D> = dynamodb
        .get_entries(
            &id,
            start
                .and_then(|x| Some(DateTime::parse_from_rfc3339(&x).ok()?.with_timezone(&Utc)))
                .as_ref(),
            end.and_then(|x| Some(DateTime::parse_from_rfc3339(&x).ok()?.with_timezone(&Utc)))
                .as_ref(),
        )
        .await?;
    Ok(entries)
}

#[graphql_object(context = Context)]
impl Query {
    async fn device(context: &mut Context, id: String) -> FieldResult<Device> {
        Ok(context.dynamodb.get_device(&id).await?)
    }

    async fn devices(context: &mut Context) -> FieldResult<Vec<Device>> {
        Ok(context.dynamodb.get_devices().await?)
    }

    async fn electricity(
        context: &mut Context,
        id: String,
        start: Option<String>,
        end: Option<String>,
    ) -> FieldResult<Vec<Electricity>> {
        get_entries(&context.dynamodb, id, start, end).await
    }

    async fn placeConditions(
        context: &mut Context,
        id: String,
        start: Option<String>,
        end: Option<String>,
    ) -> FieldResult<Vec<PlaceCondition>> {
        get_entries(&context.dynamodb, id, start, end).await
    }
}

pub type Schema =
    juniper::RootNode<'static, Query, EmptyMutation<Context>, EmptySubscription<Context>>;

pub fn schema() -> Schema {
    Schema::new(Query, EmptyMutation::new(), EmptySubscription::new())
}
