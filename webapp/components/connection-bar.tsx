'use client'

import { Badge } from '@/components/ui/badge'
import { Wifi, WifiOff } from 'lucide-react'

interface Props {
  connected: boolean
  error: string | null
}

export function ConnectionBar({ connected, error }: Props) {
  return (
    <header className="sticky top-0 z-10 border-b border-border bg-card px-4 py-3 flex items-center justify-between">
      <div className="flex items-center gap-3">
        <span className="font-semibold text-lg tracking-tight">SlitCam</span>
        <span className="text-muted-foreground text-sm hidden sm:block">
          Control Panel
        </span>
      </div>
      <div className="flex items-center gap-2">
        {connected ? (
          <Badge className="gap-1.5 bg-green-600 hover:bg-green-700 text-white">
            <Wifi className="h-3 w-3" />
            Connected
          </Badge>
        ) : (
          <Badge variant="destructive" className="gap-1.5">
            <WifiOff className="h-3 w-3" />
            {error ?? 'Disconnected'}
          </Badge>
        )}
      </div>
    </header>
  )
}
