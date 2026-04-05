mod config;
mod gadget;
mod logging;

use crate::config::Settings;
use std::env;
use std::process::{Command, ExitCode};

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

    let udc_name = gadget::setup(settings)?;
    logging::info(format!("Configured USB gadget on UDC {udc_name}"));

    let status = Command::new(&settings.uvc_gadget_bin)
        .arg("-c")
        .arg(&settings.camera_id)
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
