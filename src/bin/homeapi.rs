use anyhow::Result;
use juniper::http::GraphQLRequest;
use lambda_runtime::{error::HandlerError, lambda};
use once_cell::sync::Lazy;
use rusoto_core::Region;
use rusoto_dynamodb::DynamoDbClient;
use serde::{Deserialize, Serialize};

use homeapi::dynamodb::Client;
use homeapi::graphql::{schema, Context, Schema};

#[derive(Debug, Serialize, Deserialize)]
struct Event {
    body: Option<String>,
}

static CONTEXT: Lazy<Context> = Lazy::new(|| Context {
    dynamodb: Client::new(
        DynamoDbClient::new(Region::default()),
        std::env::var("TABLE_NAME").unwrap(),
    ),
});

static SCHEMA: Lazy<Schema> = Lazy::new(|| schema());

async fn handler(event: Event, _context: lambda_runtime::Context) -> Result<String, HandlerError> {
    let request: GraphQLRequest = serde_json::from_str(&event.body.unwrap())?;
    let res = request.execute(&SCHEMA, &CONTEXT).await;
    Ok(serde_json::to_string(&res)?)
}

fn main() -> Result<()> {
    let mut rt = tokio::runtime::Runtime::new()?;

    lambda!(move |event, context| rt.block_on(handler(event, context)));
    Ok(())
}

#[cfg(test)]
mod tests {
    use lambda_runtime::Context;

    #[test]
    fn test_lambda_handler() {
        let mut rt = tokio::runtime::Runtime::new().unwrap();

        let context = Context {
            aws_request_id: "0123456789".to_string(),
            function_name: "nature-remo-import".to_string(),
            memory_limit_in_mb: 128,
            function_version: "$LATEST".to_string(),
            invoked_function_arn: "arn:aws:lambda".to_string(),
            xray_trace_id: Some("0987654321".to_string()),
            client_context: Option::default(),
            identity: Option::default(),
            log_stream_name: "logStreamName".to_string(),
            log_group_name: "logGroupName".to_string(),
            deadline: 0,
        };

        let result = rt.block_on(super::handler(serde_json::Value::Null, context));

        assert_eq!(result.is_err(), false);
    }
}
