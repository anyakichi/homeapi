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
use clap::Parser;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use homeapi::auth::{AuthUser, auth_middleware};
use homeapi::dynamodb::Client;
use homeapi::graphql::{HomeAPI, PubSub, schema};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 8080, env = "PORT")]
    port: u16,

    #[arg(long, env = "CORS_ALLOW_ORIGIN")]
    cors_allow_origin: Option<Vec<String>>,

    #[arg(long, env = "CORS_ALLOW_HEADERS")]
    cors_allow_headers: Option<Vec<String>>,

    #[arg(long, env = "CORS_ALLOW_METHODS")]
    cors_allow_methods: Option<Vec<String>>,

    #[arg(long, default_value_t = false, env = "CORS_ALLOW_CREDENTIALS")]
    cors_allow_credentials: bool,
}

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

    let args = Args::parse();

    let client = create_client().await?;
    let schema = create_schema(client.clone()).await?;

    // Configure CORS
    let cors = if let Some(origins) = &args.cors_allow_origin {
        let mut cors_layer = CorsLayer::new();

        // Allow origins
        for origin in origins {
            cors_layer = cors_layer.allow_origin(origin.parse::<axum::http::HeaderValue>()?);
        }

        // Allow methods
        if let Some(methods) = &args.cors_allow_methods {
            for method in methods {
                cors_layer = cors_layer.allow_methods(vec![method.parse::<axum::http::Method>()?]);
            }
        } else {
            // Default methods
            cors_layer = cors_layer.allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::OPTIONS,
            ]);
        }

        // Allow headers
        if let Some(headers) = &args.cors_allow_headers {
            for header in headers {
                cors_layer =
                    cors_layer.allow_headers(vec![header.parse::<axum::http::HeaderName>()?]);
            }
        } else {
            // Default headers
            cors_layer = cors_layer.allow_headers([
                axum::http::header::CONTENT_TYPE,
                axum::http::header::AUTHORIZATION,
            ]);
        }

        // Allow credentials
        if args.cors_allow_credentials {
            cors_layer = cors_layer.allow_credentials(true);
        }

        cors_layer
    } else {
        // No CORS configuration provided, allow any origin
        CorsLayer::very_permissive()
    };

    // Create a router for GraphQL API with authentication
    let graphql_api = Router::new()
        .route(
            "/graphql",
            get(graphql_ws_handler).post(graphql_post_handler),
        )
        .layer(middleware::from_fn_with_state(
            client.clone(),
            auth_middleware,
        ))
        .with_state(schema.clone());

    // Create the main app with playground (no auth needed)
    let app = Router::new()
        .route("/", get(graphql_playground))
        .merge(graphql_api)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(schema);

    let addr = SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], args.port));
    println!("GraphQL playground: http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
