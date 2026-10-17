#!/usr/bin/env bash
# cda-design-shot.sh — capture viewport PNGs via Playwright (no auto-install).
set -euo pipefail

SKIP_MSG='SKIP_VISUAL: Playwright not available'

usage() {
  echo "Usage: $(basename "$0") <URL> <OUT_DIR>" >&2
  exit 1
}

[[ $# -eq 2 ]] || usage
URL="$1"
OUT_DIR="$2"
mkdir -p "$OUT_DIR"

if ! command -v npx >/dev/null 2>&1; then
  echo "$SKIP_MSG" >&2
  exit 2
fi

if ! npx --no-install playwright --version >/dev/null 2>&1; then
  echo "$SKIP_MSG" >&2
  exit 2
fi

WIDTHS=(375 768 1440)
for w in "${WIDTHS[@]}"; do
  OUT="$OUT_DIR/w${w}.png"
  node -e "
const { chromium } = require('playwright');
(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: ${w}, height: 900 } });
  await page.goto(process.argv[1], { waitUntil: 'networkidle' });
  await page.screenshot({ path: process.argv[2], fullPage: true });
  await browser.close();
})().catch(e => { console.error(e.message); process.exit(1); });
" "$URL" "$OUT" || {
    echo "cda-design-shot: capture failed (width ${w})" >&2
    exit 3
  }
done
