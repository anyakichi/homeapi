use std::convert::Infallible;

use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::{BadRequest, Response};
use http::StatusCode;
use once_cell::sync::Lazy;
use rusoto_core::Region;
use rusoto_dynamodb::DynamoDbClient;
use warp::{http::Response as HttpResponse, Filter, Rejection};

use homeapi::dynamodb::Client;
use homeapi::graphql::{schema, HomeAPI};

static SCHEMA: Lazy<HomeAPI> = Lazy::new(|| {
    schema(Client::new(
        DynamoDbClient::new(Region::default()),
        std::env::var("TABLE_NAME").unwrap(),
    ))
});

#[tokio::main]
async fn main() {
    env_logger::init();

    let graphql_post = async_graphql_warp::graphql(SCHEMA.clone()).and_then(
        |(schema, request): (HomeAPI, async_graphql::Request)| async move {
            Ok::<_, Infallible>(Response::from(schema.execute(request).await))
        },
    );

    let graphql_playbround = warp::path::end().and(warp::get()).map(|| {
        HttpResponse::builder()
            .header("content-type", "text/html")
            .body(playground_source(GraphQLPlaygroundConfig::new("/")))
    });

    let routes = graphql_playbround
        .or(graphql_post)
        .recover(|err: Rejection| async move {
            if let Some(BadRequest(err)) = err.find() {
                return Ok::<_, Infallible>(warp::reply::with_status(
                    err.to_string(),
                    StatusCode::BAD_REQUEST,
                ));
            }
            Ok(warp::reply::with_status(
                "INTERNAL_SERVER_ERROR".to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        });
    warp::serve(routes).run(([0, 0, 0, 0], 8080)).await
}
