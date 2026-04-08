//! Minimal HTTP API for motor control.
//!
//! Endpoints:
//!   GET  /state        → {"position_steps": N, "homed": bool}
//!   POST /home         → runs homing sequence
//!   POST /move?steps=N → moves N steps (positive = forward, negative = reverse)

use std::sync::{Arc, Mutex};

use tiny_http::{Method, Response, Server};

use crate::config::Settings;
use crate::logging;
use crate::motion::Motion;

pub fn serve(motion: Arc<Mutex<Motion>>, settings: Arc<Settings>) {
    let addr = format!("0.0.0.0:{}", 8080);
    let server = match Server::http(&addr) {
        Ok(s) => s,
        Err(e) => {
            logging::error(format!("API server failed to bind to {addr}: {e}"));
            return;
        }
    };
    logging::info(format!("HTTP API listening on {addr}"));

    for mut request in server.incoming_requests() {
        let method = request.method().clone();
        let url = request.url().to_string();
        let response = handle(&method, &url, &motion, &settings);
        let _ = request.respond(response);
    }
}

fn lock(motion: &Arc<Mutex<Motion>>) -> std::sync::MutexGuard<Motion> {
    // Recover from poisoned mutex (e.g. after a panic during homing).
    motion.lock().unwrap_or_else(|e| e.into_inner())
}

fn handle(
    method: &Method,
    url: &str,
    motion: &Arc<Mutex<Motion>>,
    settings: &Arc<Settings>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let path = url.split('?').next().unwrap_or("/");

    match (method, path) {
        (Method::Get, "/state") | (Method::Get, "/") => {
            let m = lock(motion);
            json(
                200,
                format!(
                    r#"{{"position_steps":{},"homed":{}}}"#,
                    m.position_steps, m.homed
                ),
            )
        }

        (Method::Post, "/home") => {
            let mut m = lock(motion);
            match m.home(settings) {
                Ok(()) => json(
                    200,
                    format!(
                        r#"{{"ok":true,"position_steps":{},"homed":true}}"#,
                        m.position_steps
                    ),
                ),
                Err(e) => json(500, format!(r#"{{"error":{:?}}}"#, e)),
            }
        }

        (Method::Post, "/move") => match parse_query_steps(url) {
            Some(steps) => {
                let mut m = lock(motion);
                match m.move_steps(steps) {
                    Ok(()) => json(
                        200,
                        format!(
                            r#"{{"ok":true,"position_steps":{}}}"#,
                            m.position_steps
                        ),
                    ),
                    Err(e) => json(500, format!(r#"{{"error":{:?}}}"#, e)),
                }
            }
            None => json(400, r#"{"error":"missing or invalid ?steps=N"}"#.to_string()),
        },

        _ => json(404, r#"{"error":"not found"}"#.to_string()),
    }
}

fn parse_query_steps(url: &str) -> Option<i32> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next() == Some("steps") {
            return kv.next()?.parse::<i32>().ok();
        }
    }
    None
}

fn json(code: u16, body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    let data = body.into_bytes();
    let len = data.len();
    Response::new(
        tiny_http::StatusCode(code),
        vec![
            tiny_http::Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json"[..],
            )
            .unwrap(),
            tiny_http::Header::from_bytes(
                &b"Access-Control-Allow-Origin"[..],
                &b"*"[..],
            )
            .unwrap(),
        ],
        std::io::Cursor::new(data),
        Some(len),
        None,
    )
}
