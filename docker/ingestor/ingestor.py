#!/usr/bin/env python3
"""
Simple ingestor: watches recording JSONL files and inserts appended lines into Postgres as JSONB.
It stores per-file offsets in ingestion_offsets to avoid duplicate inserts.
"""
import os
import time
import json
import logging
from pathlib import Path
import psycopg2
from psycopg2.extras import Json

logging.basicConfig(level=logging.INFO, format="[%(asctime)s] %(levelname)s: %(message)s")

DB_HOST = os.environ.get("DB_HOST", "db")
DB_PORT = int(os.environ.get("DB_PORT", "5432"))
DB_NAME = os.environ.get("DB_NAME", "chronos")
DB_USER = os.environ.get("DB_USER", "chronos")
DB_PASSWORD = os.environ.get("DB_PASSWORD", "chronos")
RECORDINGS_DIR = os.environ.get("RECORDINGS_DIR", "/data/recordings")
POLL_INTERVAL = float(os.environ.get("POLL_INTERVAL", "1"))


def connect_db():
    while True:
        try:
            conn = psycopg2.connect(host=DB_HOST, port=DB_PORT, dbname=DB_NAME, user=DB_USER, password=DB_PASSWORD)
            conn.autocommit = True
            logging.info("Connected to Postgres at %s:%s/%s", DB_HOST, DB_PORT, DB_NAME)
            return conn
        except Exception:
            logging.exception("Postgres connect failed, retrying in 5s")
            time.sleep(5)


def ensure_offsets_table(conn):
    cur = conn.cursor()
    cur.execute("""
        CREATE TABLE IF NOT EXISTS ingestion_offsets (
            file_path TEXT PRIMARY KEY,
            byte_offset BIGINT NOT NULL,
            updated_at TIMESTAMPTZ DEFAULT now()
        );
    """)
    cur.close()


def process_file(conn, path: Path):
    file_key = str(path)
    try:
        c = conn.cursor()
        c.execute("SELECT byte_offset FROM ingestion_offsets WHERE file_path = %s", (file_key,))
        row = c.fetchone()
        byte_offset = int(row[0]) if row else 0
        size = path.stat().st_size
        if byte_offset > size:
            # file rotated/truncated -> restart from 0
            byte_offset = 0
        if size == 0 or byte_offset == size:
            c.close()
            return

        with open(path, "r", encoding="utf-8") as fh:
            fh.seek(byte_offset)
            lines = fh.readlines()
            new_byte_offset = fh.tell()

        inserted = 0
        for line in lines:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except Exception:
                logging.exception("Failed to parse JSON line in %s; skipping", file_key)
                continue

            session_id = obj.get("session_id") if isinstance(obj, dict) else None
            event_type = obj.get("event_type") if isinstance(obj, dict) else None
            recorded_at_ms = obj.get("recorded_at_ms") if isinstance(obj, dict) else None

            try:
                cur2 = conn.cursor()
                cur2.execute(
                    "INSERT INTO telemetry_events (session_id, event_type, recorded_at_ms, payload) VALUES (%s,%s,%s,%s)",
                    (session_id, event_type, recorded_at_ms, Json(obj)),
                )
                cur2.close()
                inserted += 1
            except Exception:
                logging.exception("Failed to insert telemetry from %s", file_key)

        # update byte_offset
        if row:
            c.execute("UPDATE ingestion_offsets SET byte_offset = %s, updated_at = now() WHERE file_path = %s", (new_byte_offset, file_key))
        else:
            c.execute("INSERT INTO ingestion_offsets (file_path, byte_offset) VALUES (%s, %s)", (file_key, new_byte_offset))
        c.close()
        if inserted:
            logging.info("Processed %d new lines from %s", inserted, file_key)
    except Exception:
        logging.exception("Error processing file %s", file_key)


def scan_and_process(conn):
    root = Path(RECORDINGS_DIR)
    if not root.exists():
        logging.warning("Recordings dir %s does not exist; sleeping", RECORDINGS_DIR)
        return

    # traverse dated directories and process .jsonl files
    for sub in sorted(root.iterdir()):
        if not sub.is_dir():
            continue
        for f in sorted(sub.iterdir()):
            if f.is_file() and f.suffix == ".jsonl":
                process_file(conn, f)


def main():
    conn = connect_db()
    ensure_offsets_table(conn)

    # ensure telemetry_events table exists (init.sql in Postgres also creates it, but double-check)
    cur = conn.cursor()
    cur.execute("""
        CREATE TABLE IF NOT EXISTS telemetry_events (
            id BIGSERIAL PRIMARY KEY,
            session_id TEXT,
            event_type TEXT,
            recorded_at_ms BIGINT,
            payload JSONB,
            inserted_at TIMESTAMPTZ DEFAULT now()
        );
    """)
    cur.close()

    logging.info("Starting ingestion loop (poll interval %ss)", POLL_INTERVAL)
    try:
        while True:
            scan_and_process(conn)
            time.sleep(POLL_INTERVAL)
    except KeyboardInterrupt:
        logging.info("Shutting down ingestor")
    finally:
        conn.close()


if __name__ == "__main__":
    main()
