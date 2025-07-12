use anyhow::Result;
use async_graphql::Request;
use lambda_runtime::{Error, LambdaEvent, service_fn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::OnceCell;

use homeapi::dynamodb::Client;
use homeapi::graphql::{HomeAPI, schema};

#[derive(Debug, Serialize, Deserialize)]
struct Event {
    body: Option<String>,
}

static SCHEMA: OnceCell<Arc<HomeAPI>> = OnceCell::const_new();

async fn get_schema() -> Arc<HomeAPI> {
    SCHEMA
        .get_or_init(|| async {
            let config =
                aws_config::load_defaults(aws_config::BehaviorVersion::v2025_01_17()).await;
            let dynamodb = aws_sdk_dynamodb::Client::new(&config);
            Arc::new(schema(Client::new(
                dynamodb,
                std::env::var("TABLE_NAME").unwrap(),
            )))
        })
        .await
        .clone()
}

async fn handler(event: LambdaEvent<Event>) -> Result<String, Error> {
    let schema = get_schema().await;
    let req: Request = serde_json::from_str(&event.payload.body.unwrap())?;
    let res = schema.execute(req).await;
    Ok(serde_json::to_string(&res)?)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(handler)).await?;
    Ok(())
}
