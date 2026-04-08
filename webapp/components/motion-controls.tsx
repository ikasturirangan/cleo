'use client'

import { useState } from 'react'
import { toast } from 'sonner'
import { moveMotor, homeMotor } from '@/lib/api'
import type { DeviceState } from '@/lib/types'

const STEPS = [1, 5, 10, 50, 100, 500] as const

interface Props {
  state: DeviceState | null
}

export function MotionControls({ state }: Props) {
  const [pending, setPending] = useState(false)
  const position = state?.motion.position_steps ?? 0
  const homed = state?.motion.homed ?? false

  async function move(steps: number) {
    setPending(true)
    try {
      const res = await moveMotor(steps)
      if (!res.ok) toast.error(res.error ?? 'Move failed')
    } catch {
      toast.error('Move command failed')
    } finally {
      setPending(false)
    }
  }

  async function home() {
    setPending(true)
    try {
      const res = await homeMotor()
      if (!res.ok) toast.error(res.error ?? 'Homing failed')
      else toast.success('Homed')
    } catch {
      toast.error('Home command failed')
    } finally {
      setPending(false)
    }
  }

  return (
    <div className="bg-white border rounded-lg p-4 space-y-4">
      {/* Header + position */}
      <div className="flex items-center justify-between">
        <span className="text-sm font-semibold text-slate-700">Focus Control</span>
        <div className="text-right">
          <span className="font-mono text-sm font-bold text-slate-800">
            {position} <span className="font-normal text-slate-400">steps</span>
          </span>
          {homed && (
            <span className="ml-2 text-xs text-green-600 font-medium">Homed</span>
          )}
        </div>
      </div>

      {/* Forward buttons */}
      <div>
        <p className="text-xs text-slate-400 mb-1.5">Forward (+)</p>
        <div className="grid grid-cols-6 gap-1">
          {STEPS.map((s) => (
            <button
              key={`f${s}`}
              onClick={() => move(s)}
              disabled={pending}
              className="py-2 text-xs font-medium rounded border border-slate-200 bg-slate-50 hover:bg-blue-50 hover:border-blue-300 hover:text-blue-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              +{s}
            </button>
          ))}
        </div>
      </div>

      {/* Backward buttons */}
      <div>
        <p className="text-xs text-slate-400 mb-1.5">Backward (−)</p>
        <div className="grid grid-cols-6 gap-1">
          {STEPS.map((s) => (
            <button
              key={`b${s}`}
              onClick={() => move(-s)}
              disabled={pending}
              className="py-2 text-xs font-medium rounded border border-slate-200 bg-slate-50 hover:bg-amber-50 hover:border-amber-300 hover:text-amber-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              −{s}
            </button>
          ))}
        </div>
      </div>

      {/* Home */}
      <button
        onClick={home}
        disabled={pending}
        className="w-full py-2.5 text-sm font-semibold rounded border border-slate-300 bg-white hover:bg-slate-50 disabled:opacity-40 disabled:cursor-not-allowed transition-colors text-slate-700"
      >
        {pending ? 'Working…' : 'Home Motor'}
      </button>
    </div>
  )
}
