use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Json;
use axum::Router;
use axum::extract::{Query, State};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tower_http::cors::CorsLayer;

use crate::core::errors::Result;
use crate::core::logger;
use crate::models::{PlaybackStatus, RecordingFileSummary, RecordingStatus, TelemetryEvent};
use crate::recording::recorder::list_recordings;
use crate::state::{AppState, ControlCommand};

#[derive(serde::Deserialize)]
struct AlertsQuery {
    limit: Option<usize>,
}

#[derive(serde::Deserialize)]
struct StartPlaybackRequest {
    file_path: String,
    speed: Option<f32>,
}

pub async fn run(bind_addr: SocketAddr, app_state: AppState) -> Result<()> {
    let listener = TcpListener::bind(bind_addr).await?;
    let app = build_router(app_state);

    logger::info(&format!(
        "SpectraGuard HTTP/WebSocket server listening on {bind_addr}."
    ));
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn build_router(app_state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            HeaderValue::from_str(&app_state.config.allowed_frontend_origin)
                .unwrap_or_else(|_| HeaderValue::from_static("http://127.0.0.1:3000")),
        )
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(tower_http::cors::Any);

    Router::new()
        .route("/", get(root))
        .route("/ws", get(ws_handler))
        .route("/api/health", get(get_health))
        .route("/api/status", get(get_status))
        .route("/api/alerts", get(get_alerts))
        .route("/api/occupancy", get(get_occupancy))
        .route("/api/recordings", get(get_recordings))
        .route("/api/recordings/start", post(start_recording))
        .route("/api/recordings/stop", post(stop_recording))
        .route("/api/playback/start", post(start_playback))
        .route("/api/playback/stop", post(stop_playback))
        .layer(cors)
        .with_state(app_state)
}

async fn root() -> &'static str {
    "SpectraGuard backend is running."
}

async fn get_health(State(app_state): State<AppState>) -> Json<crate::models::HealthStatus> {
    let snapshots = app_state.snapshots().await;
    Json(snapshots.health)
}

async fn get_status(State(app_state): State<AppState>) -> Json<crate::models::SystemStatus> {
    let snapshots = app_state.snapshots().await;
    Json(snapshots.status)
}

async fn get_alerts(
    State(app_state): State<AppState>,
    Query(query): Query<AlertsQuery>,
) -> Json<Vec<crate::models::AlertEvent>> {
    let snapshots = app_state.snapshots().await;
    let limit = query.limit.unwrap_or(50);
    let mut alerts = snapshots.alerts.into_iter().collect::<Vec<_>>();
    if alerts.len() > limit {
        alerts = alerts.split_off(alerts.len() - limit);
    }
    Json(alerts)
}

async fn get_occupancy(
    State(app_state): State<AppState>,
) -> Json<crate::models::OccupancySnapshot> {
    let snapshots = app_state.snapshots().await;
    Json(snapshots.occupancy)
}

async fn get_recordings(
    State(app_state): State<AppState>,
) -> ApiResult<Vec<RecordingFileSummary>> {
    let recordings = list_recordings(&app_state.config.recordings_dir).map_err(internal_error)?;
    Ok(Json(recordings))
}

async fn start_recording(State(app_state): State<AppState>) -> ApiResult<RecordingStatus> {
    let (respond_to, response_rx) = oneshot::channel();
    app_state
        .control_tx
        .send(ControlCommand::StartRecording { respond_to })
        .await
        .map_err(|error| service_unavailable(error.to_string()))?;
    let response = response_rx
        .await
        .map_err(|error| service_unavailable(error.to_string()))?;
    response.map(Json).map_err(internal_error)
}

async fn stop_recording(State(app_state): State<AppState>) -> ApiResult<RecordingStatus> {
    let (respond_to, response_rx) = oneshot::channel();
    app_state
        .control_tx
        .send(ControlCommand::StopRecording { respond_to })
        .await
        .map_err(|error| service_unavailable(error.to_string()))?;
    let response = response_rx
        .await
        .map_err(|error| service_unavailable(error.to_string()))?;
    response.map(Json).map_err(internal_error)
}

async fn start_playback(
    State(app_state): State<AppState>,
    Json(payload): Json<StartPlaybackRequest>,
) -> ApiResult<PlaybackStatus> {
    let (respond_to, response_rx) = oneshot::channel();
    app_state
        .control_tx
        .send(ControlCommand::StartPlayback {
            file_path: PathBuf::from(payload.file_path),
            speed: payload.speed,
            respond_to,
        })
        .await
        .map_err(|error| service_unavailable(error.to_string()))?;
    let response = response_rx
        .await
        .map_err(|error| service_unavailable(error.to_string()))?;
    response.map(Json).map_err(internal_error)
}

async fn stop_playback(State(app_state): State<AppState>) -> ApiResult<PlaybackStatus> {
    let (respond_to, response_rx) = oneshot::channel();
    app_state
        .control_tx
        .send(ControlCommand::StopPlayback { respond_to })
        .await
        .map_err(|error| service_unavailable(error.to_string()))?;
    let response = response_rx
        .await
        .map_err(|error| service_unavailable(error.to_string()))?;
    response.map(Json).map_err(internal_error)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| client_session(socket, app_state))
}

async fn client_session(mut socket: WebSocket, app_state: AppState) {
    let snapshots = app_state.snapshots().await;
    let mut telemetry_rx = app_state.telemetry_tx.subscribe();

    if !send_event(&mut socket, &TelemetryEvent::Health(snapshots.health)).await {
        return;
    }
    if !send_event(&mut socket, &TelemetryEvent::Status(snapshots.status)).await {
        return;
    }
    if !send_event(
        &mut socket,
        &TelemetryEvent::RecordingStatus(snapshots.recording_status),
    )
    .await
    {
        return;
    }
    if !send_event(
        &mut socket,
        &TelemetryEvent::PlaybackStatus(snapshots.playback_status),
    )
    .await
    {
        return;
    }
    if !send_event(&mut socket, &TelemetryEvent::Occupancy(snapshots.occupancy)).await {
        return;
    }
    for alert in snapshots.alerts {
        if !send_event(&mut socket, &TelemetryEvent::Alert(alert)).await {
            return;
        }
    }

    loop {
        match telemetry_rx.recv().await {
            Ok(event) => {
                if !send_event(&mut socket, &event).await {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                logger::warn(&format!(
                    "A WebSocket client lagged behind and skipped {skipped} telemetry events."
                ));
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

async fn send_event(socket: &mut WebSocket, event: &TelemetryEvent) -> bool {
    match serde_json::to_string(event) {
        Ok(payload) => socket.send(Message::Text(payload.into())).await.is_ok(),
        Err(error) => {
            logger::error(&format!("Failed to serialize telemetry event: {error:#}"));
            false
        }
    }
}

type ApiResult<T> = std::result::Result<Json<T>, ApiError>;

struct ApiError {
    status: StatusCode,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}

fn internal_error(error: impl ToString) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: error.to_string(),
    }
}

fn service_unavailable(message: impl ToString) -> ApiError {
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::config::Config;
    use crate::models::{AlertEvent, AlertSeverity, TelemetryEvent};
    use crate::state::ServiceState;

    use super::build_router;

    async fn test_state() -> crate::state::AppState {
        let config = std::sync::Arc::new(Config::from_env().expect("config should load"));
        let (telemetry_tx, _) = tokio::sync::broadcast::channel(64);
        let (control_tx, _control_rx) = tokio::sync::mpsc::channel(4);
        ServiceState::new(config, telemetry_tx, control_tx, 123)
    }

    #[tokio::test]
    async fn health_endpoint_returns_snapshot() {
        let app_state = test_state().await;
        let response = build_router(app_state)
            .oneshot(Request::builder().uri("/api/health").body(Body::empty()).unwrap())
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn alerts_endpoint_honors_limit() {
        let app_state = test_state().await;
        let hub = app_state.telemetry_hub();
        hub.publish(TelemetryEvent::Alert(AlertEvent {
            id: "1".to_string(),
            alert_type: "burst_activity".to_string(),
            severity: AlertSeverity::High,
            message: "first".to_string(),
            detected_at_ms: 1,
            source_sequence: Some(1),
            frequency_start_hz: Some(1),
            frequency_end_hz: Some(2),
            power: Some(-20.0),
        }))
        .await;
        hub.publish(TelemetryEvent::Alert(AlertEvent {
            id: "2".to_string(),
            alert_type: "power_spike".to_string(),
            severity: AlertSeverity::Critical,
            message: "second".to_string(),
            detected_at_ms: 2,
            source_sequence: Some(2),
            frequency_start_hz: Some(2),
            frequency_end_hz: Some(3),
            power: Some(-10.0),
        }))
        .await;

        let response = build_router(app_state)
            .oneshot(
                Request::builder()
                    .uri("/api/alerts?limit=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request should succeed");
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should read")
            .to_bytes();
        let alerts: Vec<AlertEvent> =
            serde_json::from_slice(&body).expect("body should deserialize");

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].id, "2");
    }
}
