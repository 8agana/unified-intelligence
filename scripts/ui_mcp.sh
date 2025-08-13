#!/usr/bin/env bash
set -euo pipefail

APP="unified-intelligence"
BIN="./target/release/${APP}"
PID_FILE="./${APP}.run.pid"
LOG_DIR="./logs"
LOG_FILE="${LOG_DIR}/${APP}.log"

export UI_TRANSPORT=${UI_TRANSPORT:-http}
export UI_HTTP_BIND=${UI_HTTP_BIND:-127.0.0.1:8787}
export UI_HTTP_PATH=${UI_HTTP_PATH:-/mcp}
# Optional: UI_BEARER_TOKEN can be provided via environment

ensure_built() {
  if [ ! -x "$BIN" ]; then
    echo "[ui_mcp] Building release binary..."
    cargo build --release
  fi
}

ensure_logs() {
  mkdir -p "$LOG_DIR"
  touch "$LOG_FILE"
}

is_running() {
  if [ -f "$PID_FILE" ]; then
    local pid
    pid=$(cat "$PID_FILE" || true)
    if [ -n "$pid" ] && ps -p "$pid" > /dev/null 2>&1; then
      return 0
    fi
  fi
  return 1
}

start() {
  if is_running; then
    echo "[ui_mcp] Already running (PID $(cat "$PID_FILE"))"
    exit 0
  fi
  ensure_built
  ensure_logs
  echo "[ui_mcp] Starting ${APP} on ${UI_HTTP_BIND}${UI_HTTP_PATH} (transport=${UI_TRANSPORT})"
  nohup "$BIN" >> "$LOG_FILE" 2>&1 &
  echo $! > "$PID_FILE"
  sleep 0.5
  if is_running; then
    echo "[ui_mcp] Started (PID $(cat "$PID_FILE"))"
  else
    echo "[ui_mcp] Failed to start; see ${LOG_FILE}" >&2
    exit 1
  fi
}

stop() {
  if ! is_running; then
    echo "[ui_mcp] Not running"
    rm -f "$PID_FILE"
    exit 0
  fi
  local pid
  pid=$(cat "$PID_FILE")
  echo "[ui_mcp] Stopping PID ${pid}"
  kill "$pid" || true
  # Wait up to 5s
  for _ in $(seq 1 10); do
    if ps -p "$pid" > /dev/null 2>&1; then
      sleep 0.5
    else
      break
    fi
  done
  if ps -p "$pid" > /dev/null 2>&1; then
    echo "[ui_mcp] Forcing stop"
    kill -9 "$pid" || true
  fi
  rm -f "$PID_FILE"
  echo "[ui_mcp] Stopped"
}

status() {
  if is_running; then
    echo "[ui_mcp] Running (PID $(cat "$PID_FILE"))"
    exit 0
  else
    echo "[ui_mcp] Not running"
    exit 1
  fi
}

case "${1:-start}" in
  start) start ;;
  stop) stop ;;
  restart) stop; start ;;
  status) status ;;
  *) echo "Usage: $0 {start|stop|restart|status}"; exit 2 ;;
esac

