extern crate env_logger;
#[macro_use]
extern crate log;

mod dashboard;
mod db;
mod handlers;
mod motion;
mod state;
mod telegram;

use axum::routing::{delete, get, patch, post};
use axum::Router;
use env_logger::Env;
use log::info;
use state::{AppState, CameraState};
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::handler_status,
        handlers::handler_update_config,
        handlers::handler_delete_camera,
    ),
    components(
        schemas(
            state::StatusResponse,
            state::CameraStatus,
            handlers::ConfigUpdate,
        )
    ),
    tags(
        (name = "cameras", description = "Camera management API")
    ),
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let save_dir = {
        let d = std::env::var("SAVE_DIR").expect("SAVE_DIR env var is required");
        let path = std::path::PathBuf::from(&d);
        std::fs::create_dir_all(&path).expect("Failed to create SAVE_DIR");
        info!("Saving frames to {}", path.display());
        path
    };

    let db_path = std::env::var("DB_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| save_dir.join("db"));
    std::fs::create_dir_all(&db_path).expect("Failed to create DB_DIR");
    let db = db::Db::open(&db_path).expect("Failed to open fjall DB");
    info!("DB opened at {}", db_path.display());

    let initial_configs = db.load_all().expect("Failed to read camera configs");
    info!("Loaded {} camera config(s) from DB", initial_configs.len());

    let mut camera_map: HashMap<String, CameraState> = HashMap::new();
    for (id, cfg) in initial_configs {
        camera_map.insert(id, CameraState::new(cfg));
    }

    let telegram_token = std::env::var("TELEGRAM_TOKEN").ok();
    let telegram_chat_id = std::env::var("TELEGRAM_CHAT_ID").ok();

    let state = AppState {
        cameras: Arc::new(tokio::sync::RwLock::new(camera_map)),
        save_dir,
        telegram_token: telegram_token.clone(),
        telegram_chat_id: telegram_chat_id.clone(),
        db,
    };

    if telegram_token.is_some() && telegram_chat_id.is_some() {
        info!("Telegram command polling enabled");
        let bot_state = state.clone();
        tokio::spawn(async move {
            telegram::poll_telegram_commands(bot_state).await;
        });
    }

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/", get(handlers::handler_index))
        .route("/status.json", get(handlers::handler_status))
        .route("/stream/:camera_id", get(handlers::handler_stream))
        .route("/upload/:camera_id", post(handlers::handler_upload))
        .route("/api/camera/:camera_id/config", patch(handlers::handler_update_config))
        .route("/api/camera/:camera_id", delete(handlers::handler_delete_camera))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(tower_http::catch_panic::CatchPanicLayer::new())
                .into_inner(),
        )
        .with_state(state);

    let bind_addr =
        std::env::var("SERVER_BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
    let port = listener.local_addr().unwrap().port();

    info!("Listening on {}", listener.local_addr().unwrap());
    info!("Live view:  http://localhost:{port}/");
    info!("Stream:     http://localhost:{port}/stream/{{camera_id}}");
    info!("Upload:     POST http://localhost:{port}/upload/{{camera_id}}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .unwrap();
}
