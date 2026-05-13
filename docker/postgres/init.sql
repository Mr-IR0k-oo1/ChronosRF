-- Initialize telemetry tables for ChronosRF

CREATE TABLE IF NOT EXISTS telemetry_events (
    id BIGSERIAL PRIMARY KEY,
    session_id TEXT,
    event_type TEXT,
    recorded_at_ms BIGINT,
    payload JSONB,
    inserted_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE IF NOT EXISTS ingestion_offsets (
    file_path TEXT PRIMARY KEY,
    byte_offset BIGINT NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_telemetry_recorded_at ON telemetry_events(recorded_at_ms);
