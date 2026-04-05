mod api;
mod camera;
mod config;
mod dlp;
mod logging;
mod motion;

use crate::camera::Camera;
use crate::config::Settings;
use crate::dlp::Dlp;
use crate::motion::Motion;
use slitcam_shared::DeviceState;
use std::env;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn main() -> ExitCode {
    let command = match env::args().nth(1) {
        Some(c) => c,
        None => {
            print_help();
            return ExitCode::from(2);
        }
    };

    let settings = match Settings::from_env() {
        Ok(s) => s,
        Err(e) => {
            logging::error(e);
            return ExitCode::from(2);
        }
    };

    let result = match command.as_str() {
        "run" => run(&settings),
        "preflight" => preflight(&settings),
        "print-env" => {
            println!("{}", settings.env_template());
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        _ => Err(format!("unknown command '{command}'")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            logging::error(e);
            ExitCode::from(1)
        }
    }
}

// ── commands ──────────────────────────────────────────────────────────────────

fn preflight(settings: &Settings) -> Result<(), String> {
    logging::info("Running preflight checks");

    // I2C bus accessible?
    if !settings.i2c_bus.exists() {
        return Err(format!(
            "I2C bus {} does not exist; check BeagleBone cape/overlay config",
            settings.i2c_bus.display()
        ));
    }

    // UART device accessible?
    if !settings.uart_device.exists() {
        return Err(format!(
            "UART device {} does not exist; check BeagleBone cape/overlay config",
            settings.uart_device.display()
        ));
    }

    // Look for the Pi camera (non-fatal at preflight — it may not be connected yet).
    match Camera::find(settings)? {
        Some(path) => logging::info(format!("Pi camera found at {}", path.display())),
        None => logging::warn(
            "Pi camera not visible on USB yet; ensure the Pi is running and connected",
        ),
    }

    logging::info("Preflight OK");
    Ok(())
}

fn run(settings: &Settings) -> Result<(), String> {
    preflight(settings)?;

    // ── shared state ──────────────────────────────────────────────────────────
    let state: Arc<Mutex<DeviceState>> = Arc::new(Mutex::new(DeviceState::default()));

    // ── camera ────────────────────────────────────────────────────────────────
    let camera_path = Camera::wait_for_device(settings)?;
    let camera = Camera::open(camera_path.clone())?;
    {
        let mut s = state.lock().unwrap();
        s.camera.connected = true;
        s.camera.device_path = camera_path.to_string_lossy().into_owned();
        s.camera.capture_width = 1280;
        s.camera.capture_height = 720;
    }

    camera.start_capture()?;

    // ── DLP2000 ───────────────────────────────────────────────────────────────
    let mut dlp = Dlp::open(settings)?;
    dlp.init()?;
    {
        state.lock().unwrap().dlp_ready = true;
    }

    // ── TMC2209 ───────────────────────────────────────────────────────────────
    let mut motion = Motion::open(settings)?;
    motion.init()?;

    // ── API server ────────────────────────────────────────────────────────────
    {
        let api_state = Arc::clone(&state);
        let api_settings = settings.clone();
        std::thread::spawn(move || {
            if let Err(e) = api::serve(&api_settings, api_state) {
                logging::error(format!("API server exited: {e}"));
            }
        });
    }

    logging::info(format!(
        "SlitCam BBB controller active — API on http://{}",
        settings.api_bind
    ));

    // ── main loop ─────────────────────────────────────────────────────────────
    // Keep the service alive and mirror hardware state into the shared snapshot.
    // Long-term: replace with an async event loop or select! over device fds.
    loop {
        {
            let mut s = state.lock().unwrap();
            s.motion.position_steps = motion.position_steps;
            s.motion.homed = motion.homed;
        }
        // Silence unused-variable warnings for hardware handles kept alive here.
        let _ = &dlp;
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn print_help() {
    println!(
        concat!(
            "slitcam-bbb-controller {}\n\n",
            "Usage:\n",
            "  slitcam-bbb-controller run\n",
            "  slitcam-bbb-controller preflight\n",
            "  slitcam-bbb-controller print-env\n",
        ),
        env!("CARGO_PKG_VERSION")
    );
}
