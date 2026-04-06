'use client'

import { useEffect, useState } from 'react'
import { Toaster, toast } from 'sonner'
import { ConnectionBar } from '@/components/connection-bar'
import { DeviceStatus } from '@/components/device-status'
import { CameraView } from '@/components/camera-view'
import { SlitControls } from '@/components/slit-controls'
import { MotionControls } from '@/components/motion-controls'
import { getState } from '@/lib/api'
import type { DeviceState } from '@/lib/types'

const POLL_MS = 1000

export default function Home() {
  const [state, setState] = useState<DeviceState | null>(null)
  const [connected, setConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let mounted = true
    const seenErrors = new Set<string>()

    async function poll() {
      if (!mounted) return

      const data = await getState()
      if (!mounted) return

      if (data) {
        setState(data)
        setConnected(true)
        setError(null)

        // Surface new device-reported errors as toasts (deduplicated).
        for (const e of data.errors) {
          if (!seenErrors.has(e)) {
            seenErrors.add(e)
            toast.error(e)
          }
        }
        // Clear seen errors that are no longer active.
        for (const e of seenErrors) {
          if (!data.errors.includes(e)) seenErrors.delete(e)
        }
      } else {
        setConnected(false)
        setError('Cannot reach device')
      }
    }

    void poll()
    const id = setInterval(() => void poll(), POLL_MS)
    return () => {
      mounted = false
      clearInterval(id)
    }
  }, [])

  return (
    <div className="min-h-screen bg-background text-foreground">
      <Toaster position="top-right" richColors closeButton />
      <ConnectionBar connected={connected} error={error} />

      <main className="container mx-auto p-4 grid grid-cols-1 lg:grid-cols-3 gap-4 pt-6">
        {/* Left: camera view (spans 2 columns on large screens) */}
        <div className="lg:col-span-2">
          <CameraView state={state} />
        </div>

        {/* Right: control stack */}
        <div className="space-y-4">
          <DeviceStatus state={state} />
          <SlitControls state={state} />
          <MotionControls state={state} />
        </div>
      </main>
    </div>
  )
}
