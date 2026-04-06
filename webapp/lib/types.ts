// TypeScript mirrors of firmware/shared/src/lib.rs
// Keep in sync with the Rust types — serde renames snake_case, tag = "type".

export interface SlitConfig {
  width_um: number
  angle_deg: number
  brightness: number
  offset_x_px: number
  offset_y_px: number
}

export interface MotionState {
  position_steps: number
  homed: boolean
}

export interface CameraInfo {
  connected: boolean
  device_path: string
  capture_width: number
  capture_height: number
}

export interface DeviceState {
  camera: CameraInfo
  slit: SlitConfig
  motion: MotionState
  dlp_ready: boolean
  errors: string[]
}

// ── Control commands ──────────────────────────────────────────────────────────
// Matches #[serde(tag = "type", rename_all = "snake_case")] ControlCommand.
// SetSlit(SlitConfig) inlines the struct fields alongside the tag.

export type ControlCommand =
  | ({ type: 'set_slit' } & SlitConfig)
  | { type: 'move_focus'; steps: number }
  | { type: 'home_focus' }
  | { type: 'set_capture_format'; width: number; height: number }
  | { type: 'get_state' }
  | { type: 'ping' }

// ── Responses ────────────────────────────────────────────────────────────────
// Matches #[serde(tag = "type", rename_all = "snake_case")] CommandResponse.
// State(DeviceState) inlines the DeviceState fields.

export type CommandResponse =
  | { type: 'ok' }
  | ({ type: 'state' } & DeviceState)
  | { type: 'error'; message: string }
