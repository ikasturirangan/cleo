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

    if !settings.i2c_bus.exists() {
        return Err(format!(
            "I2C bus {} does not exist; check BeagleBone cape/overlay config",
            settings.i2c_bus.display()
        ));
    }

    if !settings.uart_device.exists() {
        return Err(format!(
            "UART device {} does not exist; check BeagleBone cape/overlay config",
            settings.uart_device.display()
        ));
    }

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
    // Honour an explicit SLITCAM_VIDEO_DEVICE override before falling back to
    // USB discovery.  This lets operators pin /dev/videoN on systems where
    // device numbering is unstable.
    let camera_path = if env::var_os("SLITCAM_VIDEO_DEVICE").is_some()
        && settings.video_device.exists()
    {
        logging::info(format!(
            "Using pinned camera device {}",
            settings.video_device.display()
        ));
        settings.video_device.clone()
    } else {
        Camera::wait_for_device(settings)?
    };

    let camera = Camera::open(camera_path.clone())?;

    // Record that the device node is present and opened; capture_width/height
    // remain 0 until start_capture() negotiates the format with the driver.
    {
        let mut s = state.lock().unwrap();
        s.camera.connected = true;
        s.camera.device_path = camera_path.to_string_lossy().into_owned();
    }

    camera.start_capture()?;

    // ── DLP2000 ───────────────────────────────────────────────────────────────
    let dlp = Arc::new(Mutex::new(Dlp::open(settings)?));
    dlp.lock().unwrap().init()?;
    // dlp_ready stays false until init() exchanges real I2C traffic with the
    // DLPC2607 and confirms the device ID.

    // ── TMC2209 ───────────────────────────────────────────────────────────────
    let motion = Arc::new(Mutex::new(Motion::open(settings)?));
    motion.lock().unwrap().init()?;

    // ── API server ────────────────────────────────────────────────────────────
    // The API thread owns shared handles so it can drive hardware directly and
    // then read back authoritative state (e.g. position_steps from Motion).
    {
        let api_state = Arc::clone(&state);
        let api_motion = Arc::clone(&motion);
        let api_dlp = Arc::clone(&dlp);
        let api_settings = settings.clone();
        std::thread::spawn(move || {
            if let Err(e) = api::serve(&api_settings, api_state, api_motion, api_dlp) {
                logging::error(format!("API server exited: {e}"));
            }
        });
    }

    logging::info(format!(
        "SlitCam BBB controller active — API on http://{}",
        settings.api_bind
    ));

    // Keep the service alive.  Hardware state is updated by the API thread
    // after each command; there is no background sync loop to clobber it.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
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
