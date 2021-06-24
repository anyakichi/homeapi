use anyhow::Result;
use async_graphql::Request;
use lambda_runtime::{handler_fn, Error};
use once_cell::sync::Lazy;
use rusoto_core::Region;
use rusoto_dynamodb::DynamoDbClient;
use serde::{Deserialize, Serialize};

use homeapi::dynamodb::Client;
use homeapi::graphql::{schema, HomeAPI};

#[derive(Debug, Serialize, Deserialize)]
struct Event {
    body: Option<String>,
}

static SCHEMA: Lazy<HomeAPI> = Lazy::new(|| {
    schema(Client::new(
        DynamoDbClient::new(Region::default()),
        std::env::var("TABLE_NAME").unwrap(),
    ))
});

async fn handler(event: Event, _context: lambda_runtime::Context) -> Result<String, Error> {
    let req: Request = serde_json::from_str(&event.body.unwrap())?;
    let res = SCHEMA.execute(req).await;
    Ok(serde_json::to_string(&res)?)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(handler_fn(handler)).await?;
    Ok(())
}
