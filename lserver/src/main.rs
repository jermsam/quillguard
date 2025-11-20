use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use harper_core::Dialect;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

pub mod lang;
use lang::{HarperConfig, JSONSuggestion};

// Application state
#[derive(Debug)]
struct AppState {
    app_name: String,
    request_count: Mutex<usize>,
    harper: HarperConfig,
}

// Response models
#[derive(Serialize)]
struct InfoResponse {
    app_name: String,
    version: String,
    request_count: usize,
}

// --- Dialect default helper ------------------------------------

fn default_dialect() -> Dialect {
    Dialect::American
}

#[derive(Deserialize)]
struct GrammarRequest {
    text: String,
    // If client omits `dialect`, this will default to American.
    #[serde(default = "default_dialect")]
    dialect: Dialect,
}

#[derive(Serialize)]
struct GrammarResponse {
    dialect: Dialect,
    suggestion_count: usize,
    suggestions: Vec<JSONSuggestion>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = Arc::new(AppState {
        app_name: "Language Server".to_string(),
        request_count: Mutex::new(0),
        harper: HarperConfig::new(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(root))
        .route("/api/info", get(info))
        .route("/api/grammar", post(check_grammar))
        .with_state(state)
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("Listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Route handlers
async fn root() -> &'static str {
    "Hello, Language Server with FerrisUp & Axum!"
}

async fn info(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut count = state.request_count.lock().await;
    *count += 1;

    let response = InfoResponse {
        app_name: state.app_name.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        request_count: *count,
    };

    Json(response)
}

async fn check_grammar(
    State(state): State<Arc<AppState>>,
    Json(request): Json<GrammarRequest>,
) -> impl IntoResponse {
    let mut count = state.request_count.lock().await;
    *count += 1;

    let suggestions = JSONSuggestion::new(&state.harper, &request.text, request.dialect);

    let response = GrammarResponse {
        dialect: request.dialect,
        suggestion_count: suggestions.len(),
        suggestions,
    };

    (StatusCode::OK, Json(response))
}

