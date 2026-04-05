use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use slitcam_shared::{CommandResponse, ControlCommand, DeviceState};

use crate::config::Settings;
use crate::logging;

/// Start the HTTP API server.  Blocks until the listener fails.
///
/// Spawn this in a background thread before entering the main service loop.
///
/// ## Endpoints
/// - `GET  /health`  → `{"type":"ok"}`
/// - `GET  /state`   → full `DeviceState` JSON
/// - `POST /command` → accepts `ControlCommand` JSON, returns `CommandResponse` JSON
pub fn serve(settings: &Settings, state: Arc<Mutex<DeviceState>>) -> Result<(), String> {
    let listener = TcpListener::bind(&settings.api_bind)
        .map_err(|e| format!("bind {}: {e}", settings.api_bind))?;
    logging::info(format!("API listening on http://{}", settings.api_bind));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, state) {
                        logging::warn(format!("API connection error: {e}"));
                    }
                });
            }
            Err(e) => logging::warn(format!("API accept error: {e}")),
        }
    }

    Ok(())
}

// ── connection handler ────────────────────────────────────────────────────────

fn handle_connection(
    stream: TcpStream,
    state: Arc<Mutex<DeviceState>>,
) -> Result<(), String> {
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|e| format!("clone stream: {e}"))?,
    );

    // Read request line
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| format!("read request line: {e}"))?;
    let request_line = request_line.trim_end();

    let mut parts = request_line.splitn(3, ' ');
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();

    // Read headers — look for Content-Length
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| format!("read header: {e}"))?;
        if line == "\r\n" || line.is_empty() {
            break;
        }
        let lower = line.to_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            if let Ok(n) = rest.trim().parse::<usize>() {
                content_length = n;
            }
        }
    }

    // Read body
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader
            .read_exact(&mut body)
            .map_err(|e| format!("read body: {e}"))?;
    }

    // Route
    let (status, body_json) = route(&method, &path, &body, state);

    // Write response — get a mutable handle to the original stream
    let mut stream = reader.into_inner();
    let response = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {body_json}",
        body_json.len(),
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|e| format!("write response: {e}"))?;

    Ok(())
}

// ── routing ───────────────────────────────────────────────────────────────────

fn route(
    method: &str,
    path: &str,
    body: &[u8],
    state: Arc<Mutex<DeviceState>>,
) -> (&'static str, String) {
    match (method, path) {
        ("GET", "/health") => (
            "200 OK",
            serde_json::to_string(&CommandResponse::Ok).unwrap_or_default(),
        ),

        ("GET", "/state") => {
            let snapshot = state.lock().unwrap().clone();
            match serde_json::to_string(&CommandResponse::State(snapshot)) {
                Ok(json) => ("200 OK", json),
                Err(e) => ("500 Internal Server Error", err_json(e)),
            }
        }

        ("POST", "/command") => match serde_json::from_slice::<ControlCommand>(body) {
            Ok(cmd) => {
                let response = execute_command(cmd, &state);
                match serde_json::to_string(&response) {
                    Ok(json) => ("200 OK", json),
                    Err(e) => ("500 Internal Server Error", err_json(e)),
                }
            }
            Err(e) => (
                "400 Bad Request",
                serde_json::to_string(&CommandResponse::error(format!(
                    "invalid command: {e}"
                )))
                .unwrap_or_default(),
            ),
        },

        _ => (
            "404 Not Found",
            serde_json::to_string(&CommandResponse::error("endpoint not found"))
                .unwrap_or_default(),
        ),
    }
}

// ── command dispatch ──────────────────────────────────────────────────────────

fn execute_command(
    cmd: ControlCommand,
    state: &Arc<Mutex<DeviceState>>,
) -> CommandResponse {
    match cmd {
        ControlCommand::Ping => CommandResponse::Ok,

        ControlCommand::GetState => {
            CommandResponse::State(state.lock().unwrap().clone())
        }

        ControlCommand::SetSlit(config) => {
            // TODO: forward to Dlp::set_slit via a command channel.
            state.lock().unwrap().slit = config;
            CommandResponse::Ok
        }

        ControlCommand::MoveFocus { steps } => {
            // TODO: forward to Motion::move_steps via a command channel.
            let mut s = state.lock().unwrap();
            if !s.motion.homed {
                return CommandResponse::error("motor not homed");
            }
            s.motion.position_steps += steps;
            CommandResponse::Ok
        }

        ControlCommand::HomeFocus => {
            // TODO: forward to Motion::home via a command channel.
            let mut s = state.lock().unwrap();
            s.motion.position_steps = 0;
            s.motion.homed = true;
            CommandResponse::Ok
        }

        ControlCommand::SetCaptureFormat { width, height } => {
            // TODO: forward to Camera via a command channel.
            let mut s = state.lock().unwrap();
            s.camera.capture_width = width;
            s.camera.capture_height = height;
            CommandResponse::Ok
        }
    }
}

fn err_json(e: impl std::fmt::Display) -> String {
    serde_json::to_string(&CommandResponse::error(e.to_string())).unwrap_or_default()
}
