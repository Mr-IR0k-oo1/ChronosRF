Local Postgres + ingestor for ChronosRF

This repository includes a docker-compose stack that starts:

- Postgres (chronosrf-db)
- An ingestor service (chronosrf-ingestor) that watches recordings/*.jsonl and inserts new lines into Postgres as JSONB

Quick start:

1. Start the stack:

   docker compose up -d

2. Verify Postgres is ready:

   docker compose logs chronosrf-db

3. Check inserted rows (on host):

   psql -h localhost -U chronos -d chronos -c "SELECT id, session_id, event_type, recorded_at_ms, inserted_at FROM telemetry_events ORDER BY inserted_at DESC LIMIT 10;"

Notes:

- The ingestor watches the repository's recordings/ directory (mounted read-only into the container). It keeps per-file offsets in the ingestion_offsets table so it can resume where it left off.
- The telemetry payload is stored verbatim in the payload JSONB column; event-specific metadata is stored in session_id, event_type, and recorded_at_ms when available.
- To change DB credentials, edit docker-compose.yml environment variables.
