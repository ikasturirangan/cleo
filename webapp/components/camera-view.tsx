import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { VideoOff, Video } from 'lucide-react'
import type { DeviceState } from '@/lib/types'

interface Props {
  state: DeviceState | null
}

export function CameraView({ state }: Props) {
  const connected = state?.camera.connected ?? false
  const hasResolution =
    connected &&
    state?.camera.capture_width !== undefined &&
    state.camera.capture_width > 0

  return (
    <Card className="overflow-hidden">
      <CardHeader className="pb-2">
        <CardTitle className="text-sm font-medium flex items-center justify-between">
          <span className="flex items-center gap-2">
            {connected ? (
              <Video className="h-4 w-4 text-green-500" />
            ) : (
              <VideoOff className="h-4 w-4 text-muted-foreground" />
            )}
            Camera Feed
          </span>
          {hasResolution && (
            <Badge variant="outline" className="font-mono text-xs">
              {state!.camera.capture_width}×{state!.camera.capture_height}
            </Badge>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent className="p-0">
        {/* Placeholder until V4L2 capture and MJPEG streaming are implemented */}
        <div className="aspect-video bg-muted flex flex-col items-center justify-center gap-3 text-muted-foreground select-none">
          <VideoOff className="h-14 w-14 opacity-25" />
          <div className="text-center space-y-1">
            <p className="text-sm">
              {connected
                ? 'Live stream coming in Phase 4'
                : 'Camera not connected'}
            </p>
            {state?.camera.device_path && (
              <p className="text-xs font-mono opacity-50">
                {state.camera.device_path}
              </p>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
