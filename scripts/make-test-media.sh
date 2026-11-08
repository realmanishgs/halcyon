#!/usr/bin/env bash
# Generates small, license-free synthetic test media (FFmpeg lavfi sources).
# Output: test-media/ (gitignored). Usage: bash scripts/make-test-media.sh
set -euo pipefail
OUT="$(dirname "$0")/../test-media"; mkdir -p "$OUT"; cd "$OUT"
F="ffmpeg -hide_banner -loglevel error -y"
V="-f lavfi -i testsrc2=size=1280x720:rate=30:duration=20"
A1="-f lavfi -i sine=frequency=440:duration=20:sample_rate=48000"
A2="-f lavfi -i sine=frequency=880:duration=20:sample_rate=48000"
printf '1\n00:00:01,000 --> 00:00:05,000\nHalcyon subtitle test — line one\n\n2\n00:00:06,000 --> 00:00:10,000\nÜnïcödé ✓ second cue\n' > subs.srt
$F $V $A1 -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest h264_aac.mp4
$F $V $A1 $A2 -i subs.srt -map 0:v -map 1:a -map 2:a -map 3:s -c:v libx264 -pix_fmt yuv420p -c:a aac -c:s srt \
   -metadata:s:a:0 language=eng -metadata:s:a:0 title="440 Hz" -metadata:s:a:1 language=jpn -metadata:s:a:1 title="880 Hz" \
   -metadata:s:s:0 language=eng -shortest multi_track.mkv
$F $V $A1 -c:v libx265 -x265-params log-level=error -pix_fmt yuv420p10le -c:a eac3 -shortest hevc10_eac3.mkv
$F $V $A1 -c:v libvpx-vp9 -b:v 1M -deadline realtime -cpu-used 8 -c:a libopus -shortest vp9_opus.webm
$F $V $A1 -c:v libsvtav1 -preset 12 -c:a libopus -shortest av1_opus.mkv
$F $V $A1 -c:v mpeg2video -c:a mp2 -shortest mpeg2.ts
$F $A1 -metadata title="Sine Study" -metadata artist="Halcyon Test" -metadata album="Fixtures" -metadata genre="Test" -metadata date=2026 -c:a flac tagged.flac
$F $A1 -c:a libmp3lame -q:a 4 tone.mp3
$F -f lavfi -i "sine=frequency=220:duration=10:sample_rate=48000" -af "pan=5.1|c0=c0|c1=c0|c2=c0|c3=c0|c4=c0|c5=c0" -c:a pcm_s24le surround51.wav
cp h264_aac.mp4 "ünïcødé 日本語 ファイル.mp4"
head -c 200000 h264_aac.mp4 > truncated.mp4
head -c 65536 /dev/urandom > garbage.mkv
ls -la
