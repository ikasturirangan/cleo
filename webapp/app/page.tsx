'use client'

import { useEffect, useState } from 'react'
import { Toaster, toast } from 'sonner'
import { ConnectionBar } from '@/components/connection-bar'
import { CameraView } from '@/components/camera-view'
import { MotionControls } from '@/components/motion-controls'
import { getState } from '@/lib/api'
import type { DeviceState } from '@/lib/types'

const POLL_MS = 800

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
        for (const e of data.errors) {
          if (!seenErrors.has(e)) {
            seenErrors.add(e)
            toast.error(e)
          }
        }
        for (const e of seenErrors) {
          if (!data.errors.includes(e)) seenErrors.delete(e)
        }
      } else {
        setConnected(false)
        setError('Cannot reach Pi')
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
    <div className="min-h-screen bg-slate-50">
      <Toaster position="top-right" richColors closeButton />
      <ConnectionBar connected={connected} error={error} />

      <main className="max-w-6xl mx-auto p-4 pt-5 grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Camera — spans 2 columns */}
        <div className="lg:col-span-2">
          <CameraView />
        </div>

        {/* Controls */}
        <div className="space-y-4">
          <MotionControls state={state} />
        </div>
      </main>
    </div>
  )
}
