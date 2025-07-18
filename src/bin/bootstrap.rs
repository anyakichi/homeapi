use async_graphql::Request;
use lambda_runtime::{Error, LambdaEvent, run, service_fn, tracing};
use serde::{Deserialize, Serialize};

use homeapi::dynamodb::Client;
use homeapi::graphql::{HomeAPI, PubSub, schema};

#[derive(Debug, Serialize, Deserialize)]
struct Event {
    body: Option<String>,
}

async fn create_client() -> Result<Client, Error> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb = aws_sdk_dynamodb::Client::new(&config);
    let table_name = match std::env::var("TABLE_NAME") {
        Ok(name) => name,
        Err(_) => {
            eprintln!("TABLE_NAME environment variable not set");
            std::process::exit(1);
        }
    };
    Ok(Client::new(dynamodb, table_name))
}

async fn create_schema(client: Client) -> Result<HomeAPI, Error> {
    let pubsub = PubSub::new();
    Ok(schema(client, pubsub))
}

async fn function_handler(event: LambdaEvent<Event>) -> Result<String, Error> {
    let client = create_client().await?;
    let schema = create_schema(client).await?;
    let body = event
        .payload
        .body
        .ok_or_else(|| Error::from("Missing request body"))?;
    let req: Request = serde_json::from_str(&body)?;
    let res = schema.execute(req).await;
    Ok(serde_json::to_string(&res)?)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    run(service_fn(function_handler)).await
}
