use std::net::SocketAddr;

use anyhow::Result;
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    Router,
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
};
use tower_http::trace::TraceLayer;

use homeapi::dynamodb::Client;
use homeapi::graphql::{HomeAPI, schema};

async fn create_schema() -> Result<HomeAPI> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::v2025_01_17()).await;
    let dynamodb = aws_sdk_dynamodb::Client::new(&config);
    let table_name = std::env::var("TABLE_NAME")
        .map_err(|_| anyhow::anyhow!("TABLE_NAME environment variable not set"))?;
    Ok(schema(Client::new(dynamodb, table_name)))
}

async fn graphql_handler(State(schema): State<HomeAPI>, req: GraphQLRequest) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let schema = create_schema().await?;

    let app = Router::new()
        .route("/", get(graphql_playground))
        .route("/graphql", get(graphql_playground).post(graphql_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(schema);

    let addr = SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], 8080));
    println!("GraphQL playground: http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
