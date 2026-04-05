use crate::config::Settings;
use crate::logging;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

struct FrameSpec {
    width: u16,
    height: u16,
    format_dir: &'static str,
    name: &'static str,
    intervals: &'static [&'static str],
}

const FRAME_SPECS: &[FrameSpec] = &[
    FrameSpec {
        width: 640,
        height: 480,
        format_dir: "uncompressed",
        name: "u",
        intervals: &[
            "333333", "416667", "500000", "666666", "1000000", "1333333", "2000000",
        ],
    },
    FrameSpec {
        width: 1280,
        height: 720,
        format_dir: "uncompressed",
        name: "u",
        intervals: &["1000000", "1333333", "2000000"],
    },
    FrameSpec {
        width: 1920,
        height: 1080,
        format_dir: "uncompressed",
        name: "u",
        intervals: &["2000000"],
    },
    FrameSpec {
        width: 640,
        height: 480,
        format_dir: "mjpeg",
        name: "m",
        intervals: &[
            "333333", "416667", "500000", "666666", "1000000", "1333333", "2000000",
        ],
    },
    FrameSpec {
        width: 1280,
        height: 720,
        format_dir: "mjpeg",
        name: "m",
        intervals: &[
            "333333", "416667", "500000", "666666", "1000000", "1333333", "2000000",
        ],
    },
    FrameSpec {
        width: 1920,
        height: 1080,
        format_dir: "mjpeg",
        name: "m",
        intervals: &[
            "333333", "416667", "500000", "666666", "1000000", "1333333", "2000000",
        ],
    },
];

pub fn preflight(settings: &Settings) -> Result<(), String> {
    if !settings.configfs_root.exists() {
        return Err(format!(
            "configfs root does not exist at {}",
            settings.configfs_root.display()
        ));
    }

    if !settings.uvc_gadget_bin.exists() {
        return Err(format!(
            "uvc-gadget binary is missing at {}",
            settings.uvc_gadget_bin.display()
        ));
    }

    match select_udc_once(settings)? {
        Some(udc) => logging::info(format!("Found USB device controller: {udc}")),
        None => logging::warn(
            "No USB device controller is visible yet; the runtime will wait for the host connection",
        ),
    }

    let camera_output = run_command_capture("rpicam-hello", &["--list-cameras"])?;
    logging::info("Camera inventory:");
    eprintln!("{camera_output}");

    Ok(())
}

pub fn setup(settings: &Settings) -> Result<String, String> {
    ensure_root()?;
    run_command("modprobe", &["libcomposite"])?;
    cleanup(settings)?;

    let udc_name = wait_for_udc(settings)?;
    let gadget_dir = settings.gadget_dir();
    let strings_dir = gadget_dir.join("strings/0x409");
    let config_dir = settings.config_dir();
    let config_strings_dir = config_dir.join("strings/0x409");
    let function_dir = settings.function_dir();

    fs::create_dir_all(&strings_dir).map_err(io_error("create gadget strings directory"))?;
    fs::create_dir_all(&config_strings_dir)
        .map_err(io_error("create gadget config strings directory"))?;
    fs::create_dir_all(function_dir.join("control/header/h"))
        .map_err(io_error("create UVC control header directory"))?;
    fs::create_dir_all(function_dir.join("control/class/fs"))
        .map_err(io_error("create UVC control class fs directory"))?;
    fs::create_dir_all(function_dir.join("control/class/ss"))
        .map_err(io_error("create UVC control class ss directory"))?;
    fs::create_dir_all(function_dir.join("streaming/header/h"))
        .map_err(io_error("create UVC streaming header directory"))?;
    fs::create_dir_all(function_dir.join("streaming/class/fs"))
        .map_err(io_error("create UVC streaming class fs directory"))?;
    fs::create_dir_all(function_dir.join("streaming/class/hs"))
        .map_err(io_error("create UVC streaming class hs directory"))?;
    fs::create_dir_all(function_dir.join("streaming/class/ss"))
        .map_err(io_error("create UVC streaming class ss directory"))?;

    write_text(gadget_dir.join("idVendor"), &settings.usb_vendor_id_hex())?;
    write_text(gadget_dir.join("idProduct"), &settings.usb_product_id_hex())?;
    write_text(strings_dir.join("serialnumber"), &settings.usb_serial)?;
    write_text(strings_dir.join("manufacturer"), &settings.usb_manufacturer)?;
    write_text(strings_dir.join("product"), &settings.usb_product)?;
    write_text(
        config_strings_dir.join("configuration"),
        &settings.usb_configuration,
    )?;
    write_text(
        config_dir.join("MaxPower"),
        &settings.max_power_ma.to_string(),
    )?;

    create_uvc_function(settings)?;
    write_text(gadget_dir.join("UDC"), &udc_name)?;

    Ok(udc_name)
}

pub fn cleanup(settings: &Settings) -> Result<(), String> {
    ensure_root()?;

    let gadget_dir = settings.gadget_dir();
    if !gadget_dir.exists() {
        return Ok(());
    }

    let udc_file = gadget_dir.join("UDC");
    if udc_file.exists() {
        fs::write(&udc_file, "").map_err(io_error("unbind USB device controller"))?;
    }

    fs::remove_dir_all(&gadget_dir).map_err(io_error("remove gadget directory"))?;
    Ok(())
}

fn create_uvc_function(settings: &Settings) -> Result<(), String> {
    let function_dir = settings.function_dir();
    let streaming_header_dir = function_dir.join("streaming/header/h");
    let streaming_class_fs = function_dir.join("streaming/class/fs/h");
    let streaming_class_hs = function_dir.join("streaming/class/hs/h");
    let streaming_class_ss = function_dir.join("streaming/class/ss/h");
    let control_header_dir = function_dir.join("control/header/h");
    let control_class_fs = function_dir.join("control/class/fs/h");
    let control_class_ss = function_dir.join("control/class/ss/h");

    for spec in FRAME_SPECS {
        let frame_dir = function_dir.join(format!(
            "streaming/{}/{}/{}p",
            spec.format_dir, spec.name, spec.height
        ));
        fs::create_dir_all(&frame_dir).map_err(io_error("create frame directory"))?;
        write_text(frame_dir.join("wWidth"), &spec.width.to_string())?;
        write_text(frame_dir.join("wHeight"), &spec.height.to_string())?;
        write_text(
            frame_dir.join("dwMaxVideoFrameBufferSize"),
            &(u32::from(spec.width) * u32::from(spec.height) * 2).to_string(),
        )?;
        write_text(
            frame_dir.join("dwFrameInterval"),
            &spec.intervals.join("\n"),
        )?;
    }

    ensure_symlink(
        &function_dir.join("streaming/uncompressed/u"),
        &streaming_header_dir.join("u"),
    )?;
    ensure_symlink(
        &function_dir.join("streaming/mjpeg/m"),
        &streaming_header_dir.join("m"),
    )?;
    ensure_symlink(&streaming_header_dir, &streaming_class_fs)?;
    ensure_symlink(&streaming_header_dir, &streaming_class_hs)?;
    ensure_symlink(&streaming_header_dir, &streaming_class_ss)?;
    ensure_symlink(&control_header_dir, &control_class_fs)?;
    ensure_symlink(&control_header_dir, &control_class_ss)?;

    write_text(function_dir.join("streaming_maxpacket"), "2048")?;
    ensure_symlink(
        &settings.function_dir(),
        &settings.config_dir().join(settings.uvc_function_name()),
    )?;

    Ok(())
}

fn ensure_root() -> Result<(), String> {
    if run_command_capture("id", &["-u"])? != "0" {
        return Err("this command must be run as root".to_string());
    }
    Ok(())
}

fn wait_for_udc(settings: &Settings) -> Result<String, String> {
    let deadline = Instant::now() + settings.udc_wait_timeout;

    loop {
        if let Some(name) = select_udc_once(settings)? {
            return Ok(name);
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "no USB device controller found under /sys/class/udc after waiting {} seconds; connect the USB host to the Pi Zero 2 W data port",
                settings.udc_wait_timeout.as_secs()
            ));
        }

        thread::sleep(Duration::from_secs(1));
    }
}

fn select_udc_once(settings: &Settings) -> Result<Option<String>, String> {
    let udc_root = Path::new("/sys/class/udc");
    let entries = fs::read_dir(udc_root).map_err(io_error("read /sys/class/udc"))?;

    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(io_error("iterate UDC entries"))?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy().trim().to_string();
        if !name.is_empty() {
            names.push(name);
        }
    }

    if names.is_empty() {
        return Ok(None);
    }

    names.sort();

    if let Some(preferred) = &settings.preferred_udc {
        if names.iter().any(|name| name == preferred) {
            return Ok(Some(preferred.clone()));
        }
        return Err(format!(
            "preferred UDC '{preferred}' was not found, available controllers: {}",
            names.join(", ")
        ));
    }

    Ok(Some(names.remove(0)))
}

fn run_command(program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|err| format!("failed to execute {program}: {err}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with status {status}"))
    }
}

fn run_command_capture(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to execute {program}: {err}"))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            Ok(String::from_utf8_lossy(&output.stderr).trim().to_string())
        } else {
            Ok(stdout)
        }
    } else {
        Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn write_text(path: PathBuf, value: &str) -> Result<(), String> {
    fs::write(&path, value).map_err(io_error(format!("write {}", path.display())))
}

fn ensure_symlink(target: &Path, link: &Path) -> Result<(), String> {
    match fs::symlink_metadata(link) {
        Ok(_) => fs::remove_file(link).map_err(io_error(format!("remove {}", link.display())))?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("failed to inspect {}: {err}", link.display())),
    }

    symlink(target, link).map_err(|err| {
        format!(
            "failed to create symlink {} -> {}: {err}",
            link.display(),
            target.display()
        )
    })
}

fn io_error(context: impl Into<String>) -> impl FnOnce(std::io::Error) -> String {
    let context = context.into();
    move |err| format!("{context}: {err}")
}

#[cfg(test)]
mod tests {
    use super::FRAME_SPECS;

    #[test]
    fn includes_full_hd_mjpeg_mode() {
        assert!(FRAME_SPECS.iter().any(|spec| {
            spec.width == 1920 && spec.height == 1080 && spec.format_dir == "mjpeg"
        }));
    }
}
