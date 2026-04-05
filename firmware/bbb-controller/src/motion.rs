use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

use crate::config::Settings;
use crate::logging;

// TMC2209 register addresses (UART access).
// See TMC2209 datasheet Rev 1.09, Table 5.
#[allow(dead_code)]
const REG_GCONF: u8 = 0x00;
#[allow(dead_code)]
const REG_GSTAT: u8 = 0x01;
#[allow(dead_code)]
const REG_IHOLD_IRUN: u8 = 0x10;
#[allow(dead_code)]
const REG_VACTUAL: u8 = 0x22;
#[allow(dead_code)]
const REG_SGTHRS: u8 = 0x40;
#[allow(dead_code)]
const REG_SG_RESULT: u8 = 0x41;
#[allow(dead_code)]
const REG_CHOPCONF: u8 = 0x6c;
#[allow(dead_code)]
const REG_DRVSTATUS: u8 = 0x6f;

// TMC2209 single-wire UART sync nibble (always 0x05).
const SYNC_NIBBLE: u8 = 0x05;

/// Interface to the TMC2209 stepper driver over Linux UART.
pub struct Motion {
    file: std::fs::File,
    pub position_steps: i32,
    pub homed: bool,
}

impl Motion {
    /// Open the UART device and configure it for TMC2209 communication.
    pub fn open(settings: &Settings) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_SYNC)
            .open(&settings.uart_device)
            .map_err(|e| format!("open {}: {e}", settings.uart_device.display()))?;

        configure_uart(file.as_raw_fd(), settings.uart_baud)?;

        logging::info(format!(
            "TMC2209 UART opened at {} baud={}",
            settings.uart_device.display(),
            settings.uart_baud,
        ));

        Ok(Self {
            file,
            position_steps: 0,
            homed: false,
        })
    }

    /// Write GCONF and CHOPCONF defaults; read DRVSTATUS to verify connectivity.
    ///
    /// TODO:
    ///   1. Write GCONF = 0x000001C0  (UART mode, StealthChop, internal Rsense).
    ///   2. Write CHOPCONF = 0x10000053 (256 µstep, TOFF=3).
    ///   3. Write IHOLD_IRUN = 0x00061F0A (hold=10, run=31, delay=6).
    ///   4. Read DRVSTATUS and check that the driver is not in fault.
    pub fn init(&mut self) -> Result<(), String> {
        logging::warn("TMC2209 init not yet implemented");
        Ok(())
    }

    /// Perform a sensorless homing sequence using StallGuard.
    ///
    /// TODO:
    ///   1. Set SGTHRS to a calibrated stall threshold.
    ///   2. Drive VACTUAL at a slow creep velocity toward the limit.
    ///   3. Poll SG_RESULT; stop and zero the counter when stall is detected.
    ///   4. Back off a fixed number of steps to clear the endstop.
    pub fn home(&mut self) -> Result<(), String> {
        logging::warn("TMC2209 homing not yet implemented");
        self.position_steps = 0;
        self.homed = true;
        Ok(())
    }

    /// Move by a signed number of full steps.  Requires a prior successful home.
    ///
    /// TODO: translate `steps` into a VACTUAL velocity profile (ramp up, cruise,
    ///       ramp down) and wait for completion before returning.
    pub fn move_steps(&mut self, steps: i32) -> Result<(), String> {
        if !self.homed {
            return Err("motion controller not homed; call home() first".to_string());
        }
        logging::warn(format!("TMC2209 move {steps} steps not yet implemented"));
        self.position_steps += steps;
        Ok(())
    }

    // ── low-level UART helpers ────────────────────────────────────────────────

    /// Write a TMC2209 register over single-wire UART.
    /// Packet layout: SYNC | RESERVED | SLAVE_ADDR | REG|0x80 | DATA(4) | CRC
    #[allow(dead_code)]
    fn write_register(&mut self, slave_addr: u8, reg: u8, value: u32) -> Result<(), String> {
        let mut packet = [0u8; 8];
        packet[0] = SYNC_NIBBLE;
        packet[1] = 0x00; // reserved
        packet[2] = slave_addr;
        packet[3] = reg | 0x80; // write flag
        packet[4] = ((value >> 24) & 0xff) as u8;
        packet[5] = ((value >> 16) & 0xff) as u8;
        packet[6] = ((value >> 8) & 0xff) as u8;
        packet[7] = (value & 0xff) as u8;
        // CRC appended after the 8 data bytes.
        let crc = tmc_crc(&packet);
        let buf = [&packet[..], &[crc]].concat();
        self.file
            .write_all(&buf)
            .map_err(|e| format!("UART write reg 0x{reg:02x}: {e}"))
    }

    /// Read a TMC2209 register (send a 4-byte read request, receive 8 bytes).
    #[allow(dead_code)]
    fn read_register(&mut self, slave_addr: u8, reg: u8) -> Result<u32, String> {
        // Send read request
        let req = [SYNC_NIBBLE, 0x00, slave_addr, reg];
        let crc = tmc_crc(&req);
        self.file
            .write_all(&[&req[..], &[crc]].concat())
            .map_err(|e| format!("UART read-request reg 0x{reg:02x}: {e}"))?;

        // Receive reply (8-byte response frame)
        let mut reply = [0u8; 8];
        self.file
            .read_exact(&mut reply)
            .map_err(|e| format!("UART read-reply reg 0x{reg:02x}: {e}"))?;

        let value = ((reply[3] as u32) << 24)
            | ((reply[4] as u32) << 16)
            | ((reply[5] as u32) << 8)
            | (reply[6] as u32);
        Ok(value)
    }
}

// ── CRC-8 for TMC2209 UART frames ────────────────────────────────────────────

fn tmc_crc(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &byte in data {
        let mut b = byte;
        for _ in 0..8 {
            if (crc ^ b) & 0x01 != 0 {
                crc = (crc >> 1) ^ 0x07;
            } else {
                crc >>= 1;
            }
            b >>= 1;
        }
    }
    crc
}

// ── termios configuration ─────────────────────────────────────────────────────

fn configure_uart(fd: i32, baud: u32) -> Result<(), String> {
    let baud_flag =
        baud_to_flag(baud).ok_or_else(|| format!("unsupported baud rate {baud}"))?;

    let mut tty: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut tty) } < 0 {
        return Err(format!("tcgetattr: {}", std::io::Error::last_os_error()));
    }

    unsafe {
        libc::cfsetispeed(&mut tty, baud_flag);
        libc::cfsetospeed(&mut tty, baud_flag);
    }

    // 8N1, no flow control, raw mode
    tty.c_cflag &= !(libc::PARENB | libc::CSTOPB | libc::CSIZE);
    tty.c_cflag |= libc::CS8 | libc::CREAD | libc::CLOCAL;
    tty.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ECHOE | libc::ISIG);
    tty.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);
    tty.c_oflag &= !libc::OPOST;
    // Block reads until at least 1 byte arrives.
    tty.c_cc[libc::VMIN] = 1;
    tty.c_cc[libc::VTIME] = 0;

    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &tty) } < 0 {
        return Err(format!("tcsetattr: {}", std::io::Error::last_os_error()));
    }

    Ok(())
}

fn baud_to_flag(baud: u32) -> Option<libc::speed_t> {
    match baud {
        9600 => Some(libc::B9600),
        19200 => Some(libc::B19200),
        38400 => Some(libc::B38400),
        57600 => Some(libc::B57600),
        115200 => Some(libc::B115200),
        230400 => Some(libc::B230400),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::tmc_crc;

    #[test]
    fn crc_known_vector() {
        // Verify against the example in the TMC2209 datasheet (write to GCONF).
        // Packet bytes before CRC: 05 00 00 80 00 00 01 C0
        let data = [0x05u8, 0x00, 0x00, 0x80, 0x00, 0x00, 0x01, 0xC0];
        // Expected CRC for this packet is 0x4A.
        assert_eq!(tmc_crc(&data), 0x4A);
    }
}
