import type { DeviceState, MotionState } from '@/lib/types'

// All calls go through the Next.js API proxy to avoid CORS.
const BASE = '/api/pi'

// ── Pi motor API ──────────────────────────────────────────────────────────────

export async function getMotorState(): Promise<MotionState | null> {
  try {
    const res = await fetch(`${BASE}/state`, { cache: 'no-store' })
    if (!res.ok) return null
    return res.json() as Promise<MotionState>
  } catch {
    return null
  }
}

export async function moveMotor(steps: number): Promise<{ ok: boolean; error?: string; position_steps?: number }> {
  try {
    const res = await fetch(`${BASE}/move?steps=${steps}`, { method: 'POST', cache: 'no-store' })
    return res.json()
  } catch (err) {
    return { ok: false, error: String(err) }
  }
}

export async function homeMotor(): Promise<{ ok: boolean; error?: string }> {
  try {
    const res = await fetch(`${BASE}/home`, { method: 'POST', cache: 'no-store' })
    return res.json()
  } catch (err) {
    return { ok: false, error: String(err) }
  }
}

// ── Legacy device state (used by connection bar / device status) ──────────────
// Polls the Pi motor state and constructs a minimal DeviceState so existing
// components keep working while BBB integration is pending.

export async function getState(): Promise<DeviceState | null> {
  const motion = await getMotorState()
  if (!motion) return null
  return {
    camera: { connected: true, device_path: '', capture_width: 0, capture_height: 0 },
    slit: { width_um: 0, angle_deg: 0, brightness: 0, offset_x_px: 0, offset_y_px: 0 },
    motion,
    dlp_ready: false,
    errors: [],
  }
}
