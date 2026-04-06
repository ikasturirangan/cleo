'use client'

import { useState } from 'react'
import { toast } from 'sonner'
import { ChevronDown, ChevronUp, Home } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import { sendCommand } from '@/lib/api'
import type { DeviceState } from '@/lib/types'

const STEP_SIZES = [1, 10, 100] as const

interface Props {
  state: DeviceState | null
}

export function MotionControls({ state }: Props) {
  const [pending, setPending] = useState(false)
  const [stepSize, setStepSize] = useState<number>(10)

  const homed = state?.motion.homed ?? false
  const position = state?.motion.position_steps ?? 0

  async function move(steps: number) {
    setPending(true)
    try {
      const res = await sendCommand({ type: 'move_focus', steps })
      if (res.type === 'error') toast.error(`Motion error: ${res.message}`)
    } catch {
      toast.error('Failed to send move command')
    } finally {
      setPending(false)
    }
  }

  async function home() {
    setPending(true)
    try {
      const res = await sendCommand({ type: 'home_focus' })
      if (res.type === 'error') toast.error(`Homing error: ${res.message}`)
      else toast.success('Homing complete')
    } catch {
      toast.error('Failed to send home command')
    } finally {
      setPending(false)
    }
  }

  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm font-medium flex items-center justify-between">
          Focus Control
          <Badge
            className={
              homed
                ? 'bg-green-600 hover:bg-green-700 text-white font-mono tabular-nums'
                : 'font-mono'
            }
            variant={homed ? 'default' : 'secondary'}
          >
            {homed ? `${position} steps` : 'Not homed'}
          </Badge>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        {/* Step size selector */}
        <div className="flex gap-1">
          {STEP_SIZES.map((s) => (
            <Button
              key={s}
              variant={stepSize === s ? 'default' : 'outline'}
              size="sm"
              className="flex-1 text-xs h-7"
              onClick={() => setStepSize(s)}
            >
              {s}
            </Button>
          ))}
        </div>

        {/* Move buttons */}
        <div className="flex gap-2">
          <Button
            variant="outline"
            className="flex-1"
            disabled={pending || !homed}
            onClick={() => move(stepSize)}
          >
            <ChevronUp className="h-4 w-4" />
            +{stepSize}
          </Button>
          <Button
            variant="outline"
            className="flex-1"
            disabled={pending || !homed}
            onClick={() => move(-stepSize)}
          >
            <ChevronDown className="h-4 w-4" />
            -{stepSize}
          </Button>
        </div>

        <Separator />

        <Button
          variant="secondary"
          className="w-full"
          disabled={pending}
          onClick={home}
        >
          <Home className="h-4 w-4 mr-1.5" />
          Home
        </Button>

        {!homed && (
          <p className="text-xs text-muted-foreground text-center">
            Home the motor before moving
          </p>
        )}
      </CardContent>
    </Card>
  )
}
