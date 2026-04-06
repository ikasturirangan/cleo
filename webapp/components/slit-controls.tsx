'use client'

import { useState } from 'react'
import { toast } from 'sonner'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import { Slider } from '@/components/ui/slider'
import { Input } from '@/components/ui/input'
import { sendCommand } from '@/lib/api'
import type { DeviceState, SlitConfig } from '@/lib/types'

interface Props {
  state: DeviceState | null
}

const DEFAULTS: SlitConfig = {
  width_um: 200,
  angle_deg: 0,
  brightness: 128,
  offset_x_px: 0,
  offset_y_px: 0,
}

export function SlitControls({ state }: Props) {
  const [pending, setPending] = useState(false)
  // Local copy — updated optimistically; overwritten by incoming state on next poll.
  const [local, setLocal] = useState<SlitConfig>(DEFAULTS)

  // Prefer polled state when available, fall back to local optimistic copy.
  const slit: SlitConfig = state?.slit ?? local

  async function apply(patch: Partial<SlitConfig>) {
    const next: SlitConfig = { ...slit, ...patch }
    setLocal(next)
    setPending(true)
    try {
      const res = await sendCommand({ type: 'set_slit', ...next })
      if (res.type === 'error') toast.error(`Slit error: ${res.message}`)
    } catch {
      toast.error('Failed to send slit command')
    } finally {
      setPending(false)
    }
  }

  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm font-medium">Slit Controls</CardTitle>
      </CardHeader>
      <CardContent className="space-y-5 text-sm">
        <SliderRow
          label="Width"
          unit="µm"
          value={slit.width_um}
          display={slit.width_um.toFixed(0)}
          min={10}
          max={500}
          step={5}
          disabled={pending}
          onChange={(v) => apply({ width_um: v })}
        />

        <SliderRow
          label="Angle"
          unit="°"
          value={slit.angle_deg}
          display={slit.angle_deg.toFixed(1)}
          min={-90}
          max={90}
          step={0.5}
          disabled={pending}
          onChange={(v) => apply({ angle_deg: v })}
        />

        <SliderRow
          label="Brightness"
          unit=""
          value={slit.brightness}
          display={String(slit.brightness)}
          min={0}
          max={255}
          step={1}
          disabled={pending}
          onChange={(v) => apply({ brightness: v })}
        />

        <div className="grid grid-cols-2 gap-3">
          <OffsetInput
            label="X offset (px)"
            value={slit.offset_x_px}
            disabled={pending}
            onChange={(v) => apply({ offset_x_px: v })}
          />
          <OffsetInput
            label="Y offset (px)"
            value={slit.offset_y_px}
            disabled={pending}
            onChange={(v) => apply({ offset_y_px: v })}
          />
        </div>
      </CardContent>
    </Card>
  )
}

function SliderRow({
  label,
  unit,
  value,
  display,
  min,
  max,
  step,
  disabled,
  onChange,
}: {
  label: string
  unit: string
  value: number
  display: string
  min: number
  max: number
  step: number
  disabled: boolean
  onChange: (v: number) => void
}) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between">
        <Label className="text-xs text-muted-foreground">{label}</Label>
        <span className="text-xs tabular-nums font-mono">
          {display}
          {unit}
        </span>
      </div>
      <Slider
        value={[value]}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        onValueChange={([v]) => onChange(v)}
        className="w-full"
      />
    </div>
  )
}

function OffsetInput({
  label,
  value,
  disabled,
  onChange,
}: {
  label: string
  value: number
  disabled: boolean
  onChange: (v: number) => void
}) {
  return (
    <div className="space-y-1">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <Input
        type="number"
        value={value}
        min={-200}
        max={200}
        disabled={disabled}
        className="h-8 text-sm font-mono"
        onChange={(e) => {
          const n = Number(e.target.value)
          if (!Number.isNaN(n)) onChange(n)
        }}
      />
    </div>
  )
}
