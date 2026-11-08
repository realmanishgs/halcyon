#!/usr/bin/env bash
# Verifies hardware decoding on THIS machine with the same libVLC/plugins Halcyon
# loads: plays hi-res fixtures through vlc.exe with D3D11VA requested and reads
# VLC's own debug log to see which decoder was actually used.
# (A VLC window appears for ~3 s per file.)
# Usage: bash scripts/hw-decode-check.sh   (after make-stress-media.sh)
set -uo pipefail
VLC="${HALCYON_LIBVLC_DIR:-/c/Program Files/VideoLAN/VLC}/vlc.exe"
ROOT="$(cd "$(dirname "$0")/.." && pwd)/test-media"
OUT="$ROOT/HW_DECODE.md"
LOG="$(mktemp -d)"
{
  echo "# Hardware decoding check"
  echo
  echo "Machine GPU(s): $(powershell -NoProfile -c "(Get-CimInstance Win32_VideoController).Name -join ', '" | tr -d '\r')"
  echo "Engine: $("$VLC" --version 2>/dev/null | head -1 | tr -d '\r')"
  echo
  echo "| File | Decoder input (from VLC log) | Result (D3D11VA requested) |"
  echo "|---|---|---|"
} > "$OUT"
for f in hires_4k_hevc10_hdr10.mkv hires_4k60_h264.mp4 hires_4k_vp9_10bit.webm hires_8k_av1.mkv mp4_h264_aac.mp4; do
  src="$ROOT/formats/$f"; [ -f "$src" ] || continue
  log="$LOG/$f.log"
  "$VLC" --intf dummy --play-and-exit --run-time 3 --no-audio --avcodec-hw d3d11va --vout direct3d11 \
    --file-logging --logfile "$(cygpath -w "$log")" --log-verbose 2 "$(cygpath -w "$src")" >/dev/null 2>&1
  hw=$(grep -a -o -E "Using D3D11VA \(.*\) for hardware decoding|Using DXVA2 \(.*\) for hardware decoding" "$log" | head -1)
  input=$(grep -a -o -E "'[^']*' as input" "$log" | head -1)
  if [ -n "$hw" ]; then used="HARDWARE — $hw"; else used="software (no D3D11VA/DXVA2 decoder created)"; fi
  echo "| \`$f\` | ${input:-?} | $used |" >> "$OUT"
done
echo >> "$OUT"
echo "Raw logs: $LOG" >> "$OUT"
cat "$OUT"
