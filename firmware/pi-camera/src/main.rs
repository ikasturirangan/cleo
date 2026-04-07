mod config;
mod gadget;
mod logging;
mod motion;

use crate::config::Settings;
use crate::motion::Motion;
use std::env;
use std::process::{Command, ExitCode};
use std::sync::{Arc, Mutex};

fn main() -> ExitCode {
    let command = match env::args().nth(1) {
        Some(command) => command,
        None => {
            print_help();
            return ExitCode::from(2);
        }
    };

    let settings = match Settings::from_env() {
        Ok(settings) => settings,
        Err(err) => {
            logging::error(err);
            return ExitCode::from(2);
        }
    };

    let result = match command.as_str() {
        "run" => run(&settings),
        "cleanup" => gadget::cleanup(&settings),
        "preflight" => gadget::preflight(&settings),
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
        Err(err) => {
            logging::error(err);
            ExitCode::from(1)
        }
    }
}

fn run(settings: &Settings) -> Result<(), String> {
    gadget::preflight(settings)?;

    // ── TMC2209 step/dir GPIO ─────────────────────────────────────────────────
    // Non-fatal — camera still works if GPIO export fails (e.g. pins in use).
    let motion: Option<Arc<Mutex<Motion>>> = match Motion::open(settings) {
        Ok(m) => {
            logging::info("TMC2209 step/dir GPIO ready");
            Some(Arc::new(Mutex::new(m)))
        }
        Err(e) => {
            logging::warn(format!("TMC2209 GPIO open failed: {e} — motor disabled"));
            None
        }
    };

    let udc_name = gadget::setup(settings)?;
    logging::info(format!("Configured USB gadget on UDC {udc_name}"));

    // Pass motion handle into uvc-gadget via env so future control integration
    // can forward commands; for now uvc-gadget runs in the foreground.
    let _ = motion; // keep alive until uvc-gadget exits

    let status = Command::new(&settings.uvc_gadget_bin)
        .arg("-c")
        .arg(&settings.camera_id)
        .arg("-r")
        .arg(&settings.uvc_resolution)
        .arg("-f")
        .arg(settings.uvc_framerate.to_string())
        .arg(settings.uvc_function_name())
        .status()
        .map_err(|err| {
            format!(
                "failed to launch {}: {err}",
                settings.uvc_gadget_bin.display()
            )
        })?;

    let cleanup_result = gadget::cleanup(settings);

    if let Err(err) = cleanup_result {
        logging::warn(format!("cleanup after run failed: {err}"));
    }

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "uvc-gadget exited with status {status}; inspect journalctl for details"
        ))
    }
}

fn print_help() {
    println!(
        concat!(
            "slitcam-pi-camera {}\n\n",
            "Usage:\n",
            "  slitcam-pi-camera preflight\n",
            "  slitcam-pi-camera run\n",
            "  slitcam-pi-camera cleanup\n",
            "  slitcam-pi-camera print-env\n",
        ),
        env!("CARGO_PKG_VERSION")
    );
}
