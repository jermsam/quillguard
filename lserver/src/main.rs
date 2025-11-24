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
use crate::lang::{HarperConfig, JSONSuggestion, Corrector};
use crate::lang::lint::{check_grammar_professional, GrammarResponse as ProfessionalGrammarResponse};

// Application state
#[derive(Debug)]
struct AppState {
    app_name: String,
    request_count: Mutex<usize>,
    harper: HarperConfig,
    t5_corrector: Corrector,
}

#[derive(Serialize)]
struct InfoResponse {
    app_name: String,
    version: String,
    request_count: usize,
}

fn default_dialect() -> Dialect {
    Dialect::American
}

#[derive(Deserialize)]
struct GrammarRequest {
    text: String,
    // If client omits `dialect`, this will default to American.
    #[serde(default = "default_dialect")]
    dialect: Dialect,
    // Optional flag to enable T5 contextual correction
    #[serde(default)]
    use_t5: bool,
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

    // Initialize T5 corrector
    let t5_corrector = Corrector::new().await;

    let state = Arc::new(AppState {
        app_name: "Language Server".to_string(),
        request_count: Mutex::new(0),
        harper: HarperConfig::new(),
        t5_corrector,
    });

    let app = Router::new()
        .route("/api/info", get(info))
        .route("/api/grammar", post(check_grammar))
        .route("/api/grammar/professional", post(check_grammar_pro))
        .with_state(state)
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Route handlers

async fn info(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut count = state.request_count.lock().await;
    *count += 1;

    Json(InfoResponse {
        app_name: state.app_name.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        request_count: *count,
    })
}

async fn check_grammar(
    State(state): State<Arc<AppState>>,
    Json(request): Json<GrammarRequest>,
) -> impl IntoResponse {
    let mut count = state.request_count.lock().await;
    *count += 1;

    let suggestions = if request.use_t5 {
        JSONSuggestion::new_with_t5(&state.harper, &request.text, request.dialect, Some(&state.t5_corrector)).await
    } else {
        JSONSuggestion::new(&state.harper, &request.text, request.dialect)
    };

    (StatusCode::OK, Json(GrammarResponse {
        dialect: request.dialect,
        suggestion_count: suggestions.len(),
        suggestions,
    }))
}

/// Professional UX-focused grammar checking endpoint
async fn check_grammar_pro(
    State(state): State<Arc<AppState>>,
    Json(request): Json<GrammarRequest>,
) -> impl IntoResponse {
    let mut count = state.request_count.lock().await;
    *count += 1;

    let corrector = if request.use_t5 {
        Some(&state.t5_corrector)
    } else {
        None
    };

    let response = check_grammar_professional(
        &state.harper,
        &request.text,
        request.dialect,
        corrector
    ).await;

    (StatusCode::OK, Json(response))
}
