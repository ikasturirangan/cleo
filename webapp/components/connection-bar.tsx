'use client'

interface Props {
  connected: boolean
  error: string | null
}

export function ConnectionBar({ connected, error }: Props) {
  return (
    <header className="border-b bg-white px-6 py-3 flex items-center justify-between">
      <div className="flex items-center gap-3">
        <span className="font-bold text-base tracking-tight text-slate-800">SlitCam</span>
        <span className="text-slate-400 text-sm">Control Panel</span>
      </div>
      <div className="flex items-center gap-2">
        <span
          className={`inline-flex items-center gap-1.5 text-xs font-medium px-2.5 py-1 rounded-full ${
            connected
              ? 'bg-green-50 text-green-700 border border-green-200'
              : 'bg-red-50 text-red-700 border border-red-200'
          }`}
        >
          <span
            className={`w-1.5 h-1.5 rounded-full ${connected ? 'bg-green-500' : 'bg-red-500'}`}
          />
          {connected ? 'Connected' : error ?? 'Disconnected'}
        </span>
      </div>
    </header>
  )
}
