import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import type { DeviceState } from '@/lib/types'

interface Props {
  state: DeviceState | null
}

export function DeviceStatus({ state }: Props) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm font-medium">Device Status</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2 text-sm">
        <Row label="Camera">
          <StatusBadge
            ok={state?.camera.connected ?? false}
            label={
              state?.camera.connected
                ? (state.camera.device_path || 'Connected')
                : 'Disconnected'
            }
          />
        </Row>

        <Row label="Projector">
          <StatusBadge
            ok={state?.dlp_ready ?? false}
            label={state?.dlp_ready ? 'Ready' : 'Not ready'}
          />
        </Row>

        <Row label="Motor">
          <StatusBadge
            ok={state?.motion.homed ?? false}
            label={state?.motion.homed ? 'Homed' : 'Not homed'}
          />
        </Row>

        {state?.motion.homed && (
          <Row label="Position">
            <span className="font-mono text-xs tabular-nums">
              {state.motion.position_steps} steps
            </span>
          </Row>
        )}

        {state?.camera.capture_width && state.camera.capture_width > 0 ? (
          <Row label="Resolution">
            <span className="font-mono text-xs tabular-nums">
              {state.camera.capture_width}×{state.camera.capture_height}
            </span>
          </Row>
        ) : null}

        {state?.errors && state.errors.length > 0 && (
          <>
            <Separator />
            <div className="space-y-1 pt-1">
              {state.errors.map((e, i) => (
                <p key={i} className="text-destructive text-xs leading-tight">
                  {e}
                </p>
              ))}
            </div>
          </>
        )}
      </CardContent>
    </Card>
  )
}

function Row({
  label,
  children,
}: {
  label: string
  children: React.ReactNode
}) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="text-muted-foreground shrink-0">{label}</span>
      {children}
    </div>
  )
}

function StatusBadge({ ok, label }: { ok: boolean; label: string }) {
  return (
    <Badge
      variant={ok ? 'default' : 'secondary'}
      className={ok ? 'bg-green-600 hover:bg-green-700 text-white truncate max-w-[160px]' : 'truncate max-w-[160px]'}
      title={label}
    >
      {label}
    </Badge>
  )
}
