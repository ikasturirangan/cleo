'use client'

import { useEffect, useRef, useState } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { VideoOff, Video } from 'lucide-react'

export function CameraView() {
  const videoRef = useRef<HTMLVideoElement>(null)
  const [streaming, setStreaming] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [label, setLabel] = useState<string>('')

  useEffect(() => {
    let stream: MediaStream | null = null

    async function start() {
      try {
        // Enumerate devices to find the SlitCam Pi Camera by name.
        const devices = await navigator.mediaDevices.enumerateDevices()
        const cam = devices.find(
          (d) =>
            d.kind === 'videoinput' &&
            d.label.toLowerCase().includes('slitcam'),
        )

        const videoConstraints: MediaTrackConstraints = {
          width: { ideal: 640 },
          height: { ideal: 480 },
          frameRate: { ideal: 30 },
          ...(cam ? { deviceId: { exact: cam.deviceId } } : {}),
        }
        const constraints: MediaStreamConstraints = { video: videoConstraints }

        stream = await navigator.mediaDevices.getUserMedia(constraints)

        if (videoRef.current) {
          videoRef.current.srcObject = stream
          setLabel(stream.getVideoTracks()[0]?.label ?? 'Camera')
          setStreaming(true)
          setError(null)
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
        setStreaming(false)
      }
    }

    start()

    return () => {
      stream?.getTracks().forEach((t) => t.stop())
    }
  }, [])

  return (
    <Card className="overflow-hidden">
      <CardHeader className="pb-2">
        <CardTitle className="text-sm font-medium flex items-center justify-between">
          <span className="flex items-center gap-2">
            {streaming ? (
              <Video className="h-4 w-4 text-green-500" />
            ) : (
              <VideoOff className="h-4 w-4 text-muted-foreground" />
            )}
            Camera Feed
          </span>
          {streaming && (
            <Badge variant="outline" className="font-mono text-xs">
              Live
            </Badge>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent className="p-0 relative">
        {/* Video element is always mounted so the ref is valid when getUserMedia resolves */}
        <video
          ref={videoRef}
          autoPlay
          playsInline
          muted
          className={`w-full aspect-video bg-black object-contain ${streaming ? '' : 'hidden'}`}
        />
        {!streaming && (
          <div className="aspect-video bg-muted flex flex-col items-center justify-center gap-3 text-muted-foreground select-none">
            <VideoOff className="h-14 w-14 opacity-25" />
            <div className="text-center space-y-1 px-4">
              <p className="text-sm">{error ?? 'Connecting to camera…'}</p>
              {label && (
                <p className="text-xs font-mono opacity-50">{label}</p>
              )}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
