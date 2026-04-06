import { type NextRequest, NextResponse } from 'next/server'

// Server-side env var — never exposed to the browser.
const BBB_API_URL = process.env.BBB_API_URL ?? 'http://slitcam-bbb.local:8080'

async function proxy(req: NextRequest, path: string[]): Promise<NextResponse> {
  const url = `${BBB_API_URL}/${path.join('/')}`

  const init: RequestInit = {
    method: req.method,
    headers: { 'Content-Type': 'application/json' },
    // next.js fetch cache must be disabled for live device polling
    cache: 'no-store',
  }

  if (req.method === 'POST') {
    init.body = await req.text()
  }

  try {
    const upstream = await fetch(url, init)
    const data: unknown = await upstream.json()
    return NextResponse.json(data, { status: upstream.status })
  } catch (err) {
    return NextResponse.json(
      { type: 'error', message: `Cannot reach BBB at ${BBB_API_URL}: ${err}` },
      { status: 503 },
    )
  }
}

export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ path: string[] }> },
) {
  return proxy(req, (await params).path)
}

export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ path: string[] }> },
) {
  return proxy(req, (await params).path)
}
