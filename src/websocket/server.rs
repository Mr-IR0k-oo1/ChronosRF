use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Json;
use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tower_http::cors::CorsLayer;

use crate::core::errors::Result;
use crate::core::logger;
use crate::models::{
    IgorAssessment, PlaybackStatus, RecordingFileSummary, RecordingStatus, TelemetryEvent,
};
use crate::recording::recorder::list_recordings;
use crate::state::{AppState, ControlCommand};

#[derive(serde::Deserialize)]
struct EventsQuery {
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
        .route("/api/igor", get(get_igor_assessments))
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
    Query(query): Query<EventsQuery>,
) -> Json<Vec<crate::models::AlertEvent>> {
    let snapshots = app_state.snapshots().await;
    let limit = query.limit.unwrap_or(50);
    let mut alerts = snapshots.alerts.into_iter().collect::<Vec<_>>();
    if alerts.len() > limit {
        alerts = alerts.split_off(alerts.len() - limit);
    }
    Json(alerts)
}

async fn get_igor_assessments(
    State(app_state): State<AppState>,
    Query(query): Query<EventsQuery>,
) -> Json<Vec<IgorAssessment>> {
    let snapshots = app_state.snapshots().await;
    let limit = query.limit.unwrap_or(50);
    let mut assessments = snapshots.igor_assessments.into_iter().collect::<Vec<_>>();
    if assessments.len() > limit {
        assessments = assessments.split_off(assessments.len() - limit);
    }
    Json(assessments)
}

async fn get_occupancy(
    State(app_state): State<AppState>,
) -> Json<crate::models::OccupancySnapshot> {
    let snapshots = app_state.snapshots().await;
    Json(snapshots.occupancy)
}

async fn get_recordings(State(app_state): State<AppState>) -> ApiResult<Vec<RecordingFileSummary>> {
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

async fn ws_handler(ws: WebSocketUpgrade, State(app_state): State<AppState>) -> impl IntoResponse {
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
    for assessment in snapshots.igor_assessments {
        if !send_event(&mut socket, &TelemetryEvent::IgorAssessment(assessment)).await {
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
    use std::fs;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::time::Duration;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use futures_util::StreamExt;
    use http_body_util::BodyExt;
    use serde::de::DeserializeOwned;
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;
    use tokio::time::timeout;
    use tokio_tungstenite::connect_async;
    use tower::ServiceExt;

    use crate::config::Config;
    use crate::core::event_bus::EventBus;
    use crate::models::{
        AlertEvent, AlertSeverity, CaptureMode, HealthState, HealthStatus, IgorAssessment,
        IgorFindingKind, OccupancySnapshot, PlaybackStatus, RecordedTelemetry,
        RecordingFileSummary, RecordingStatus, TelemetryEvent,
    };
    use crate::state::{AppState, ServiceState};

    use super::build_router;

    async fn test_state() -> AppState {
        let config = std::sync::Arc::new(Config::from_env().expect("config should load"));
        let event_bus = EventBus::new(64);
        let (telemetry_tx, _) = tokio::sync::broadcast::channel(64);
        let (control_tx, _control_rx) = tokio::sync::mpsc::channel(4);
        ServiceState::new(config, event_bus, telemetry_tx, control_tx, 123)
    }

    async fn read_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should read")
            .to_bytes();
        serde_json::from_slice(&body).expect("body should deserialize")
    }

    async fn spawn_test_server(app_state: AppState) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .expect("listener should bind");
        let addr = listener
            .local_addr()
            .expect("listener should have an address");
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(app_state))
                .await
                .expect("test server should run");
        });
        (format!("ws://{addr}/ws"), server)
    }

    #[tokio::test]
    async fn health_endpoint_returns_snapshot() {
        let app_state = test_state().await;
        let response = build_router(app_state)
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
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

    #[tokio::test]
    async fn igor_endpoint_honors_limit() {
        let app_state = test_state().await;
        let hub = app_state.telemetry_hub();
        hub.publish(TelemetryEvent::IgorAssessment(IgorAssessment {
            id: "igor-1".to_string(),
            generated_at_ms: 1,
            source_sequence: 1,
            finding_kind: IgorFindingKind::PersistentEmitter,
            severity: AlertSeverity::High,
            risk_score: 70,
            frequency_start_hz: 1,
            frequency_end_hz: 2,
            evidence_count: 3,
            distinct_anomaly_types: vec![crate::models::AnomalyType::BurstActivity],
            max_power: -20.0,
            message: "first".to_string(),
        }))
        .await;
        hub.publish(TelemetryEvent::IgorAssessment(IgorAssessment {
            id: "igor-2".to_string(),
            generated_at_ms: 2,
            source_sequence: 2,
            finding_kind: IgorFindingKind::CoordinatedEmitter,
            severity: AlertSeverity::Critical,
            risk_score: 92,
            frequency_start_hz: 2,
            frequency_end_hz: 3,
            evidence_count: 4,
            distinct_anomaly_types: vec![crate::models::AnomalyType::PowerSpike],
            max_power: -10.0,
            message: "second".to_string(),
        }))
        .await;

        let response = build_router(app_state)
            .oneshot(
                Request::builder()
                    .uri("/api/igor?limit=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request should succeed");
        let assessments: Vec<IgorAssessment> = read_json(response).await;

        assert_eq!(assessments.len(), 1);
        assert_eq!(assessments[0].id, "igor-2");
    }

    #[tokio::test]
    async fn recordings_endpoint_returns_enriched_recording_summaries() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let recordings_dir = temp_dir.path().join("recordings");
        let dated_dir = recordings_dir.join("20260511");
        fs::create_dir_all(&dated_dir).expect("dated directory should be created");
        let recording_path = dated_dir.join("session-1.jsonl");
        let payload = [
            RecordedTelemetry {
                session_id: "session-1".to_string(),
                event_type: "occupancy".to_string(),
                recorded_at_ms: 100,
                event: TelemetryEvent::Occupancy(OccupancySnapshot {
                    generated_at_ms: 100,
                    window_seconds: 60,
                    bins: Vec::new(),
                }),
            },
            RecordedTelemetry {
                session_id: "session-1".to_string(),
                event_type: "alert".to_string(),
                recorded_at_ms: 150,
                event: TelemetryEvent::Alert(AlertEvent {
                    id: "alert-1".to_string(),
                    alert_type: "burst_activity".to_string(),
                    severity: AlertSeverity::High,
                    message: "alert".to_string(),
                    detected_at_ms: 150,
                    source_sequence: Some(1),
                    frequency_start_hz: Some(10),
                    frequency_end_hz: Some(20),
                    power: Some(-20.0),
                }),
            },
        ]
        .into_iter()
        .map(|event| serde_json::to_string(&event).expect("recorded event should serialize"))
        .collect::<Vec<_>>()
        .join("\n");
        fs::write(&recording_path, format!("{payload}\n"))
            .expect("recording file should be written");

        let mut config = Config::from_env().expect("config should load");
        config.recordings_dir = recordings_dir;
        let config = std::sync::Arc::new(config);
        let event_bus = EventBus::new(64);
        let (telemetry_tx, _) = tokio::sync::broadcast::channel(64);
        let (control_tx, _control_rx) = tokio::sync::mpsc::channel(4);
        let app_state = ServiceState::new(config, event_bus, telemetry_tx, control_tx, 123);

        let response = build_router(app_state)
            .oneshot(
                Request::builder()
                    .uri("/api/recordings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request should succeed");
        let summaries: Vec<RecordingFileSummary> = read_json(response).await;

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].session_id, "session-1");
        assert_eq!(summaries[0].started_at_ms, Some(100));
        assert_eq!(summaries[0].ended_at_ms, Some(150));
        assert_eq!(summaries[0].event_count, Some(2));
        assert_eq!(summaries[0].alert_count, Some(1));
        assert_eq!(summaries[0].anomaly_count, Some(0));
        assert_eq!(summaries[0].igor_count, Some(0));
    }

    #[tokio::test]
    async fn status_and_health_endpoints_stay_consistent_across_live_degraded_and_playback() {
        let app_state = test_state().await;
        let hub = app_state.telemetry_hub();
        let mut live_status = app_state.snapshots().await.status;
        let live_recording = RecordingStatus {
            active: true,
            session_id: Some("rec-1".to_string()),
            file_path: Some("recordings/rec-1.jsonl".to_string()),
            started_at_ms: Some(10),
            event_count: 4,
        };
        let idle_playback = PlaybackStatus::default();
        live_status.current_mode = CaptureMode::Live;
        live_status.last_sweep_sequence = Some(7);
        live_status.last_sweep_at_ms = Some(77);
        live_status.current_recording = live_recording.clone();
        live_status.current_playback = idle_playback.clone();

        hub.publish(TelemetryEvent::Health(HealthStatus::online(
            "hackrf_sweep.exe",
            "Live capture active.",
        )))
        .await;
        hub.publish(TelemetryEvent::RecordingStatus(live_recording.clone()))
            .await;
        hub.publish(TelemetryEvent::PlaybackStatus(idle_playback.clone()))
            .await;
        hub.publish(TelemetryEvent::Status(live_status.clone()))
            .await;

        let live_health_response = build_router(app_state.clone())
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("live health request should succeed");
        let live_status_response = build_router(app_state.clone())
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("live status request should succeed");
        let live_health: HealthStatus = read_json(live_health_response).await;
        let live_status_snapshot: crate::models::SystemStatus =
            read_json(live_status_response).await;

        assert_eq!(live_health.state, HealthState::Online);
        assert_eq!(live_status_snapshot.current_mode, CaptureMode::Live);
        assert_eq!(live_status_snapshot.last_sweep_sequence, Some(7));
        assert_eq!(live_status_snapshot.current_recording, live_recording);

        let playback_status = PlaybackStatus {
            active: true,
            file_path: Some("recordings/playback.jsonl".to_string()),
            speed: 2.0,
            started_at_ms: Some(100),
            emitted_events: 3,
        };
        let mut playback_mode_status = live_status_snapshot.clone();
        playback_mode_status.current_mode = CaptureMode::Playback;
        playback_mode_status.current_playback = playback_status.clone();

        hub.publish(TelemetryEvent::Health(HealthStatus::degraded(
            "hackrf_sweep.exe",
            "Live capture paused while playback is active.",
            None,
        )))
        .await;
        hub.publish(TelemetryEvent::PlaybackStatus(playback_status.clone()))
            .await;
        hub.publish(TelemetryEvent::Status(playback_mode_status.clone()))
            .await;

        let playback_health_response = build_router(app_state.clone())
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("playback health request should succeed");
        let playback_status_response = build_router(app_state.clone())
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("playback status request should succeed");
        let playback_health: HealthStatus = read_json(playback_health_response).await;
        let playback_status_snapshot: crate::models::SystemStatus =
            read_json(playback_status_response).await;

        assert_eq!(playback_health.state, HealthState::Degraded);
        assert!(playback_health.message.contains("playback"));
        assert_eq!(playback_status_snapshot.current_mode, CaptureMode::Playback);
        assert_eq!(playback_status_snapshot.current_playback, playback_status);

        let mut degraded_live_status = playback_status_snapshot.clone();
        degraded_live_status.current_mode = CaptureMode::Live;
        degraded_live_status.current_playback.active = false;
        hub.publish(TelemetryEvent::Health(HealthStatus::degraded(
            "hackrf_sweep.exe",
            "Sweep capture exited with code Some(1); restarting.",
            Some("simulated capture exit".to_string()),
        )))
        .await;
        hub.publish(TelemetryEvent::PlaybackStatus(
            degraded_live_status.current_playback.clone(),
        ))
        .await;
        hub.publish(TelemetryEvent::Status(degraded_live_status.clone()))
            .await;

        let degraded_health_response = build_router(app_state.clone())
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("degraded health request should succeed");
        let degraded_status_response = build_router(app_state)
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("degraded status request should succeed");
        let degraded_health: HealthStatus = read_json(degraded_health_response).await;
        let degraded_status_snapshot: crate::models::SystemStatus =
            read_json(degraded_status_response).await;

        assert_eq!(degraded_health.state, HealthState::Degraded);
        assert!(degraded_health.message.contains("restarting"));
        assert_eq!(degraded_status_snapshot.current_mode, CaptureMode::Live);
        assert!(!degraded_status_snapshot.current_playback.active);
        assert_eq!(
            degraded_status_snapshot.current_recording.session_id,
            Some("rec-1".to_string())
        );
    }

    #[tokio::test]
    async fn websocket_bootstrap_emits_snapshot_sequence_in_order() {
        let app_state = test_state().await;
        let hub = app_state.telemetry_hub();
        let mut status = app_state.snapshots().await.status;
        let recording_status = RecordingStatus {
            active: true,
            session_id: Some("rec-7".to_string()),
            file_path: Some("recordings/rec-7.jsonl".to_string()),
            started_at_ms: Some(70),
            event_count: 9,
        };
        let playback_status = PlaybackStatus::default();
        let occupancy = OccupancySnapshot {
            generated_at_ms: 90,
            window_seconds: 300,
            bins: Vec::new(),
        };
        status.current_mode = CaptureMode::Live;
        status.last_sweep_sequence = Some(42);
        status.last_sweep_at_ms = Some(99);
        status.current_recording = recording_status.clone();
        status.current_playback = playback_status.clone();

        hub.publish(TelemetryEvent::Health(HealthStatus::online(
            "hackrf_sweep.exe",
            "Live capture active.",
        )))
        .await;
        hub.publish(TelemetryEvent::Status(status)).await;
        hub.publish(TelemetryEvent::RecordingStatus(recording_status.clone()))
            .await;
        hub.publish(TelemetryEvent::PlaybackStatus(playback_status.clone()))
            .await;
        hub.publish(TelemetryEvent::Occupancy(occupancy.clone()))
            .await;
        hub.publish(TelemetryEvent::Alert(AlertEvent {
            id: "a-1".to_string(),
            alert_type: "burst_activity".to_string(),
            severity: AlertSeverity::High,
            message: "first".to_string(),
            detected_at_ms: 1,
            source_sequence: Some(1),
            frequency_start_hz: Some(10),
            frequency_end_hz: Some(20),
            power: Some(-20.0),
        }))
        .await;
        hub.publish(TelemetryEvent::Alert(AlertEvent {
            id: "a-2".to_string(),
            alert_type: "power_spike".to_string(),
            severity: AlertSeverity::Critical,
            message: "second".to_string(),
            detected_at_ms: 2,
            source_sequence: Some(2),
            frequency_start_hz: Some(20),
            frequency_end_hz: Some(30),
            power: Some(-10.0),
        }))
        .await;

        let (url, server) = spawn_test_server(app_state).await;
        let (mut socket, _) = connect_async(url)
            .await
            .expect("websocket client should connect");
        let mut received = Vec::new();

        for _ in 0..7 {
            let message = timeout(Duration::from_secs(5), socket.next())
                .await
                .expect("websocket message should arrive")
                .expect("websocket stream should remain open")
                .expect("websocket message should be valid");
            let text = message.into_text().expect("bootstrap frame should be text");
            let event = serde_json::from_str::<TelemetryEvent>(&text)
                .expect("bootstrap payload should deserialize");
            received.push(event);
        }

        server.abort();
        let _ = server.await;

        assert!(matches!(received[0], TelemetryEvent::Health(_)));
        assert!(matches!(received[1], TelemetryEvent::Status(_)));
        assert!(matches!(received[2], TelemetryEvent::RecordingStatus(_)));
        assert!(matches!(received[3], TelemetryEvent::PlaybackStatus(_)));
        assert!(matches!(received[4], TelemetryEvent::Occupancy(_)));
        assert!(matches!(received[5], TelemetryEvent::Alert(_)));
        assert!(matches!(received[6], TelemetryEvent::Alert(_)));

        match &received[1] {
            TelemetryEvent::Status(status) => {
                assert_eq!(status.last_sweep_sequence, Some(42));
            }
            _ => panic!("second bootstrap event should be status"),
        }

        match &received[2] {
            TelemetryEvent::RecordingStatus(status) => {
                assert_eq!(status.session_id.as_deref(), Some("rec-7"));
            }
            _ => panic!("third bootstrap event should be recording status"),
        }

        match &received[5] {
            TelemetryEvent::Alert(alert) => assert_eq!(alert.id, "a-1"),
            _ => panic!("sixth bootstrap event should be the first alert"),
        }

        match &received[6] {
            TelemetryEvent::Alert(alert) => assert_eq!(alert.id, "a-2"),
            _ => panic!("seventh bootstrap event should be the second alert"),
        }
    }

    #[tokio::test]
    async fn websocket_bootstrap_includes_igor_assessments_after_alerts() {
        let app_state = test_state().await;
        let hub = app_state.telemetry_hub();

        hub.publish(TelemetryEvent::Health(HealthStatus::online(
            "hackrf_sweep.exe",
            "Live capture active.",
        )))
        .await;
        hub.publish(TelemetryEvent::Status(app_state.snapshots().await.status))
            .await;
        hub.publish(TelemetryEvent::RecordingStatus(RecordingStatus::default()))
            .await;
        hub.publish(TelemetryEvent::PlaybackStatus(PlaybackStatus::default()))
            .await;
        hub.publish(TelemetryEvent::Occupancy(OccupancySnapshot::default()))
            .await;
        hub.publish(TelemetryEvent::Alert(AlertEvent {
            id: "a-1".to_string(),
            alert_type: "burst_activity".to_string(),
            severity: AlertSeverity::High,
            message: "first".to_string(),
            detected_at_ms: 1,
            source_sequence: Some(1),
            frequency_start_hz: Some(10),
            frequency_end_hz: Some(20),
            power: Some(-20.0),
        }))
        .await;
        hub.publish(TelemetryEvent::IgorAssessment(IgorAssessment {
            id: "igor-1".to_string(),
            generated_at_ms: 2,
            source_sequence: 2,
            finding_kind: IgorFindingKind::CoordinatedEmitter,
            severity: AlertSeverity::Critical,
            risk_score: 90,
            frequency_start_hz: 20,
            frequency_end_hz: 30,
            evidence_count: 5,
            distinct_anomaly_types: vec![
                crate::models::AnomalyType::RepeatedPulses,
                crate::models::AnomalyType::PowerSpike,
            ],
            max_power: -10.0,
            message: "igor".to_string(),
        }))
        .await;

        let (url, server) = spawn_test_server(app_state).await;
        let (mut socket, _) = connect_async(url)
            .await
            .expect("websocket client should connect");
        let mut received = Vec::new();

        for _ in 0..7 {
            let message = timeout(Duration::from_secs(5), socket.next())
                .await
                .expect("websocket message should arrive")
                .expect("websocket stream should remain open")
                .expect("websocket message should be valid");
            let text = message.into_text().expect("bootstrap frame should be text");
            let event = serde_json::from_str::<TelemetryEvent>(&text)
                .expect("bootstrap payload should deserialize");
            received.push(event);
        }

        server.abort();
        let _ = server.await;

        assert!(matches!(received[5], TelemetryEvent::Alert(_)));
        assert!(matches!(received[6], TelemetryEvent::IgorAssessment(_)));
    }
}
