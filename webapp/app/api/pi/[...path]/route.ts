import { type NextRequest, NextResponse } from 'next/server'

// Pi is always on the USB CDC NCM interface at a fixed IP.
const PI_API_URL = process.env.PI_API_URL ?? 'http://192.168.7.1:8080'

async function proxy(req: NextRequest, path: string[]): Promise<NextResponse> {
  const url = `${PI_API_URL}/${path.join('/')}`

  const init: RequestInit = {
    method: req.method,
    headers: { 'Content-Type': 'application/json' },
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
      { type: 'error', message: `Cannot reach Pi at ${PI_API_URL}: ${err}` },
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
