#!/usr/bin/env bash
# Downloads the official libVLC and an FFmpeg build into vendor/<os>/ so the
# installers can include them (see src-tauri/tauri.runtime.<os>.json).
# Usage: bash scripts/fetch-runtime.sh windows|macos-arm64|macos-x64
#   VLC_VERSION (default 3.0.21) picks the VideoLAN release.
#   On Windows, LOCAL_VLC=<dir> copies an installed VLC instead of downloading.
set -euo pipefail
VLC_VERSION="${VLC_VERSION:-3.0.21}"
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

case "${1:-}" in
  windows)
    out="$root/vendor/windows"
    rm -rf "$out"; mkdir -p "$out/vlc" "$out/ffmpeg"
    if [ -n "${LOCAL_VLC:-}" ]; then
      src="$LOCAL_VLC"
    else
      curl -fL --retry 3 -o "$work/vlc.zip" "https://download.videolan.org/pub/videolan/vlc/$VLC_VERSION/win64/vlc-$VLC_VERSION-win64.zip"
      unzip -q "$work/vlc.zip" -d "$work"
      src="$work/vlc-$VLC_VERSION"
    fi
    cp "$src/libvlc.dll" "$src/libvlccore.dll" "$out/vlc/"
    cp -R "$src/plugins" "$out/vlc/plugins"
    # Plugin cache: without it libVLC scans every plugin on each start.
    "$src/vlc-cache-gen.exe" "$out/vlc/plugins" || echo "warning: plugin cache not generated"
    # FFmpeg (ffprobe for inspection/library, ffmpeg for Lab/Convert): shared build, DLLs alongside.
    curl -fL --retry 3 -o "$work/ff.zip" "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl-shared.zip"
    unzip -q "$work/ff.zip" -d "$work/ff"
    cp "$work"/ff/*/bin/ffmpeg.exe "$work"/ff/*/bin/ffprobe.exe "$work"/ff/*/bin/*.dll "$out/ffmpeg/"
    ;;
  macos-arm64|macos-x64)
    arch=$([ "$1" = macos-arm64 ] && echo arm64 || echo intel64)
    out="$root/vendor/macos"
    rm -rf "$out"; mkdir -p "$out/vlc" "$out/ffmpeg"
    curl -fL --retry 3 -o "$work/vlc.dmg" "https://download.videolan.org/pub/videolan/vlc/$VLC_VERSION/macosx/vlc-$VLC_VERSION-$arch.dmg"
    hdiutil attach -nobrowse -readonly -mountpoint "$work/mnt" "$work/vlc.dmg" >/dev/null
    cp -R "$work/mnt/VLC.app/Contents/MacOS/lib" "$out/vlc/lib"
    cp -R "$work/mnt/VLC.app/Contents/MacOS/plugins" "$out/vlc/plugins"
    hdiutil detach "$work/mnt" >/dev/null
    # Static FFmpeg for this architecture (ffmpeg.martin-riedl.de builds, arm64 and amd64).
    # Optional: without it, playback works and inspection/library/Lab/Convert report FFmpeg missing.
    ffarch=$([ "$1" = macos-arm64 ] && echo arm64 || echo amd64)
    for tool in ffmpeg ffprobe; do
      if curl -fL --retry 3 -o "$work/$tool.zip" "https://ffmpeg.martin-riedl.de/redirect/latest/macos/$ffarch/release/$tool.zip"; then
        unzip -q -o "$work/$tool.zip" -d "$out/ffmpeg" && chmod +x "$out/ffmpeg/$tool"
      else
        echo "warning: $tool not bundled"
      fi
    done
    # libvlc.dylib's dependencies, for the CI log.
    otool -L "$out/vlc/lib/libvlc.dylib" || true
    ;;
  *)
    echo "usage: $0 windows|macos-arm64|macos-x64" >&2; exit 2 ;;
esac
du -sh "$out"/* 2>/dev/null
