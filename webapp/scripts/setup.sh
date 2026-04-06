#!/usr/bin/env bash
# One-time setup for the SlitCam web application.
# Run from the webapp/ directory: bash scripts/setup.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/.."

# ── Node version check ────────────────────────────────────────────────────────

node_major=$(node -e "process.stdout.write(String(process.versions.node.split('.')[0]))" 2>/dev/null || echo "0")
if [[ "${node_major}" -lt 18 ]]; then
  echo "Node.js 18 or later is required (found ${node_major})" >&2
  exit 1
fi

# ── Install npm dependencies ──────────────────────────────────────────────────

echo "Installing npm dependencies..."
npm install

# ── Add shadcn components ─────────────────────────────────────────────────────
# --yes skips confirmation prompts; --overwrite updates existing files.

echo "Adding shadcn/ui components..."
npx shadcn@latest add --yes --overwrite \
  badge \
  button \
  card \
  input \
  label \
  separator \
  slider \
  sonner

# ── .env.local ────────────────────────────────────────────────────────────────

if [[ ! -f .env.local ]]; then
  cp .env.local.example .env.local
  echo ""
  echo "Created .env.local from .env.local.example"
  echo "Edit BBB_API_URL to point to your BeagleBone's IP or hostname."
fi

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
echo "Setup complete. Run the dev server with:"
echo "  npm run dev"
echo ""
echo "Then open http://localhost:3000"
