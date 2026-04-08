'use client'

import { useState } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { VideoOff, Video } from 'lucide-react'

const STREAM_URL = 'http://192.168.7.1:8080/stream.mjpg'

export function CameraView() {
  const [connected, setConnected] = useState(false)
  const [error, setError] = useState(false)

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
          {connected && (
            <Badge variant="outline" className="font-mono text-xs">
              Live
            </Badge>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent className="p-0 relative">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img
          src={STREAM_URL}
          alt="Camera feed"
          className={`w-full aspect-video bg-black object-contain ${connected ? '' : 'hidden'}`}
          onLoad={() => { setConnected(true); setError(false) }}
          onError={() => { setConnected(false); setError(true) }}
        />
        {!connected && (
          <div className="aspect-video bg-muted flex flex-col items-center justify-center gap-3 text-muted-foreground select-none">
            <VideoOff className="h-14 w-14 opacity-25" />
            <div className="text-center space-y-1 px-4">
              <p className="text-sm">
                {error ? 'Cannot reach camera — is the Pi connected?' : 'Connecting to camera…'}
              </p>
              <p className="text-xs font-mono opacity-50">{STREAM_URL}</p>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
