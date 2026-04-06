import type { CommandResponse, ControlCommand, DeviceState } from '@/lib/types'

// All calls go through the Next.js API proxy to avoid CORS.
// The proxy forwards to BBB_API_URL (server env var).
const BASE = '/api/bbb'

export async function getState(): Promise<DeviceState | null> {
  try {
    const res = await fetch(`${BASE}/state`, { cache: 'no-store' })
    if (!res.ok) return null
    const body = (await res.json()) as CommandResponse
    if (body.type === 'state') {
      const { type: _type, ...state } = body
      return state as DeviceState
    }
    return null
  } catch {
    return null
  }
}

export async function sendCommand(cmd: ControlCommand): Promise<CommandResponse> {
  const res = await fetch(`${BASE}/command`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(cmd),
  })
  return res.json() as Promise<CommandResponse>
}

export async function ping(): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/health`, { cache: 'no-store' })
    if (!res.ok) return false
    const body = (await res.json()) as CommandResponse
    return body.type === 'ok'
  } catch {
    return false
  }
}
