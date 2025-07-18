use std::net::SocketAddr;

use anyhow::Result;
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket};
use axum::{
    Router,
    extract::{State, WebSocketUpgrade},
    middleware,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use tower_http::trace::TraceLayer;

use homeapi::auth::{AuthUser, auth_middleware};
use homeapi::dynamodb::Client;
use homeapi::graphql::{HomeAPI, PubSub, schema};

async fn create_client() -> Result<Client> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::v2025_01_17()).await;
    let dynamodb = aws_sdk_dynamodb::Client::new(&config);
    let table_name = std::env::var("TABLE_NAME")
        .map_err(|_| anyhow::anyhow!("TABLE_NAME environment variable not set"))?;
    Ok(Client::new(dynamodb, table_name))
}

async fn create_schema(client: Client) -> Result<HomeAPI> {
    let pubsub = PubSub::new();
    Ok(schema(client, pubsub))
}

async fn graphql_post_handler(
    State(schema): State<HomeAPI>,
    auth_user: AuthUser,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let mut request = req.into_inner();
    request = request.data(auth_user);
    schema.execute(request).await.into()
}

async fn graphql_ws_handler(
    State(schema): State<HomeAPI>,
    ws: WebSocketUpgrade,
    protocol: GraphQLProtocol,
) -> Response {
    ws.protocols(["graphql-transport-ws", "graphql-ws"])
        .on_upgrade(move |socket| GraphQLWebSocket::new(socket, schema, protocol).serve())
}

async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(
        GraphQLPlaygroundConfig::new("/graphql").subscription_endpoint("/graphql"),
    ))
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let client = create_client().await?;
    let schema = create_schema(client.clone()).await?;

    let app = Router::new()
        .route("/", get(graphql_playground))
        .route(
            "/graphql",
            get(graphql_ws_handler).post(graphql_post_handler),
        )
        .layer(middleware::from_fn_with_state(
            client.clone(),
            auth_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(schema);

    let addr = SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], 8080));
    println!("GraphQL playground: http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
