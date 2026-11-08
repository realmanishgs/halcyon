#!/usr/bin/env bash
# Generates the stress-test corpus: every container/codec this FFmpeg build can
# encode, hi-res video/audio, Dolby/DTS, IAMF (Eclipsa), subtitles, playlists and
# damaged/odd files. All media is synthetic (lavfi testsrc2/sine) — license-free.
#
# Output (gitignored):
#   test-media/formats/   one file per format + EXPECT.tsv (name<TAB>expect<TAB>note)
#   test-media/robust/    damaged / odd-name / long-path / 3-hour files
#   test-media/playlists/ M3U, M3U8, PLS, XSPF pointing at formats/
#   test-media/corpus/    put your own real files here (Dolby Atmos, Dolby Vision,
#                         Eclipsa exports, ...) — the stress test plays them too.
#
# expect = play          must open, decode every stream and seek
#          partial       plays, but not fully (note says what is missing)
#          unsupported   documented engine limitation; must fail cleanly (no hang/crash)
#
# Usage: bash scripts/make-stress-media.sh
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)/test-media"
mkdir -p "$ROOT/formats" "$ROOT/robust" "$ROOT/playlists" "$ROOT/corpus"
cd "$ROOT/formats"
MAN=EXPECT.tsv
: > "$MAN"
FF="ffmpeg -hide_banner -loglevel error -y -nostdin"
D=6
V="-f lavfi -i testsrc2=size=1280x720:rate=30:duration=$D"
VSD="-f lavfi -i testsrc2=size=720x576:rate=25:duration=$D"
A="-f lavfi -i sine=frequency=440:duration=$D:sample_rate=48000"
ok=0; bad=0

# gen <file> <expect> <note> -- <ffmpeg args...>
gen() {
  local out="$1" expect="$2" note="$3"; shift 4
  if [ -s "$out" ]; then  # resumable: keep what an earlier run produced
    printf '%s	%s	%s
' "$out" "$expect" "$note" >> "$MAN"; ok=$((ok+1)); return
  fi
  if timeout 300 $FF "$@" "$out" 2> ".err" && [ -s "$out" ]; then
    printf '%s\t%s\t%s\n' "$out" "$expect" "$note" >> "$MAN"; ok=$((ok+1)); printf '  + %s\n' "$out"
  else
    rm -f "$out"; bad=$((bad+1)); printf '  ! %s (not generated: %s)\n' "$out" "$(head -c 160 .err | tr '\n' ' ')"
  fi
}
surround() { echo "pan=$1|$(seq -s'|' 0 $(($2-1)) | sed 's/\([0-9]*\)/c\1=c0/g')"; }

echo "== video containers / codecs"
gen mp4_h264_aac.mp4        play "MP4 H.264 High + AAC-LC" -- $V $A -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest
gen mp4_hevc_aac.mp4        play "MP4 HEVC Main + AAC" -- $V $A -c:v libx265 -x265-params log-level=error -tag:v hvc1 -c:a aac -shortest
gen mp4_av1_opus.mp4        play "MP4 AV1 + Opus" -- $V $A -c:v libsvtav1 -preset 12 -c:a libopus -shortest
gen m4v_h264_aac.m4v        play "M4V H.264 + AAC" -- $V $A -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest
gen mkv_h264_flac.mkv       play "MKV H.264 + FLAC" -- $V $A -c:v libx264 -pix_fmt yuv420p -c:a flac -shortest
gen mkv_ffv1_flac.mkv       play "MKV FFV1 lossless + FLAC" -- $V $A -c:v ffv1 -c:a flac -shortest
gen mkv_vvc_opus.mkv        unsupported "H.266/VVC: no VVC decoder in libVLC 3.0" -- $V $A -c:v libvvenc -preset faster -c:a libopus -shortest
gen webm_vp8_vorbis.webm    play "WebM VP8 + Vorbis" -- $V $A -c:v libvpx -b:v 1M -deadline realtime -cpu-used 8 -c:a libvorbis -shortest
gen webm_vp9_opus.webm      play "WebM VP9 + Opus" -- $V $A -c:v libvpx-vp9 -b:v 1M -deadline realtime -cpu-used 8 -c:a libopus -shortest
gen webm_av1_opus.webm      play "WebM AV1 + Opus" -- $V $A -c:v libsvtav1 -preset 12 -c:a libopus -shortest
gen avi_mpeg4_mp3.avi       play "AVI MPEG-4 Part 2 (Xvid) + MP3" -- $V $A -c:v libxvid -q:v 4 -c:a libmp3lame -shortest
gen avi_divx3_mp3.avi       play "AVI MS-MPEG4v3 (DivX 3) + MP3" -- $V $A -c:v msmpeg4v3 -q:v 4 -c:a libmp3lame -shortest
gen avi_mjpeg_pcm.avi       play "AVI Motion JPEG + PCM" -- $V $A -c:v mjpeg -q:v 5 -pix_fmt yuvj420p -c:a pcm_s16le -shortest
gen mov_prores_pcm24.mov    play "MOV ProRes 422 HQ + PCM 24-bit" -- $V $A -c:v prores_ks -profile:v 3 -c:a pcm_s24le -shortest
gen mov_dnxhr_pcm.mov       play "MOV DNxHR LB + PCM" -- $V $A -c:v dnxhd -profile:v dnxhr_lb -pix_fmt yuv422p -c:a pcm_s16le -shortest
gen mov_h264_alac.mov       play "MOV H.264 + ALAC" -- $V $A -c:v libx264 -pix_fmt yuv420p -c:a alac -shortest
gen flv_h264_aac.flv        play "FLV H.264 + AAC" -- $V $A -c:v libx264 -pix_fmt yuv420p -c:a aac -ar 44100 -shortest
gen wmv_wmv2_wmav2.wmv      play "WMV (ASF) WMV2 + WMA v2" -- $V $A -c:v wmv2 -b:v 2M -c:a wmav2 -shortest
gen asf_wmv1_wmav2.asf      play "ASF WMV1 + WMA v2" -- $V $A -c:v wmv1 -b:v 2M -c:a wmav2 -shortest
gen mpg_mpeg1_mp2.mpg       play "MPEG-PS MPEG-1 + MP2" -- $V $A -c:v mpeg1video -b:v 3M -c:a mp2 -shortest -f mpeg
gen vob_mpeg2_ac3.vob       play "VOB (DVD PS) MPEG-2 + AC-3" -- $VSD $A -c:v mpeg2video -b:v 5M -c:a ac3 -shortest -f vob
gen ts_hevc_eac3.ts         play "MPEG-TS HEVC + E-AC-3" -- $V $A -c:v libx265 -x265-params log-level=error -c:a eac3 -shortest
gen m2ts_h264_ac3.m2ts      play "M2TS (Blu-ray TS) H.264 + AC-3" -- $V $A -c:v libx264 -pix_fmt yuv420p -c:a ac3 -shortest -f mpegts -mpegts_m2ts_mode 1
gen 3gp_h263_amrnb.3gp      play "3GP H.263 + AMR-NB" -- -f lavfi -i testsrc2=size=352x288:rate=15:duration=$D -f lavfi -i sine=frequency=440:duration=$D:sample_rate=8000 -c:v h263 -c:a libopencore_amrnb -ac 1 -ar 8000 -b:a 12.2k -shortest
gen 3g2_mpeg4_aac.3g2       play "3G2 MPEG-4 + AAC" -- -f lavfi -i testsrc2=size=352x288:rate=15:duration=$D $A -c:v mpeg4 -c:a aac -shortest
gen ogv_theora_vorbis.ogv   play "Ogg Theora + Vorbis" -- $V $A -c:v libtheora -q:v 6 -c:a libvorbis -shortest
gen mxf_mpeg2_pcm.mxf       play "MXF (OP1a) MPEG-2 + PCM" -- $VSD $A -c:v mpeg2video -b:v 8M -pix_fmt yuv422p -c:a pcm_s16le -shortest
gen dv_pal.dv               play "DV PAL (dvvideo + PCM)" -- $VSD $A -c:v dvvideo -pix_fmt yuv420p -c:a pcm_s16le -ar 48000 -ac 2 -shortest
gen nut_h264_flac.nut       play "NUT H.264 + FLAC" -- $V $A -c:v libx264 -pix_fmt yuv420p -c:a flac -shortest
gen mkv_mpeg2_interlaced.mkv play "MPEG-2 interlaced (TFF) — deinterlace target" -- $V $A -vf "fps=50,interlace=scan=tff" -c:v mpeg2video -flags +ilme+ildct -field_order tt -b:v 6M -c:a mp2 -shortest

echo "== embedded subtitles & chapters"
printf '1\n00:00:00,500 --> 00:00:03,000\nFirst cue — ünïcødé ✓\n\n2\n00:00:03,200 --> 00:00:05,500\nSecond cue\n' > ../sub.srt
printf '[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, Bold, Alignment, MarginV\nStyle: Default,Arial,48,&H00FFFFFF,1,2,40\n\n[Events]\nFormat: Layer, Start, End, Style, Text\nDialogue: 0,0:00:00.50,0:00:05.50,Default,{\\b1}Styled ASS{\\b0} cue\n' > ../sub.ass
printf 'WEBVTT\n\n00:00.500 --> 00:05.500\nWebVTT cue\n' > ../sub.vtt
printf ';FFMETADATA1\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=2000\ntitle=Intro\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=2000\nEND=4000\ntitle=Middle\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=4000\nEND=6000\ntitle=End\n' > ../chapters.txt
gen mkv_h264_ass.mkv        play "MKV + embedded ASS subtitles" -- $V $A -i ../sub.ass -map 0:v -map 1:a -map 2:s -c:v libx264 -pix_fmt yuv420p -c:a aac -c:s ass -t $D
gen mp4_h264_movtext.mp4    play "MP4 + mov_text (tx3g) subtitles" -- $V $A -i ../sub.srt -map 0:v -map 1:a -map 2:s -c:v libx264 -pix_fmt yuv420p -c:a aac -c:s mov_text -t $D
gen webm_vp9_webvtt.webm    play "WebM + WebVTT subtitles" -- $V $A -i ../sub.vtt -map 0:v -map 1:a -map 2:s -c:v libvpx-vp9 -deadline realtime -cpu-used 8 -c:a libopus -c:s webvtt -t $D
gen mkv_chapters.mkv        play "MKV with 3 chapters" -- $V $A -i ../chapters.txt -map 0:v -map 1:a -map_metadata 2 -map_chapters 2 -c:v libx264 -pix_fmt yuv420p -c:a aac -t $D
cp ../sub.srt mp4_h264_aac.srt; cp ../sub.ass mp4_h264_aac.ass; cp ../sub.vtt mp4_h264_aac.vtt

echo "== hi-res video"
gen hires_4k60_h264.mp4     play "4K 60 fps H.264 High 5.1" -- -f lavfi -i testsrc2=size=3840x2160:rate=60:duration=3 -f lavfi -i sine=d=3:r=48000 -c:v libx264 -preset ultrafast -pix_fmt yuv420p -c:a aac -shortest
gen hires_4k_hevc10_hdr10.mkv play "4K HEVC Main10 HDR10 (PQ, BT.2020, mastering metadata)" -- -f lavfi -i testsrc2=size=3840x2160:rate=24:duration=3 -f lavfi -i sine=d=3:r=48000 -c:v libx265 -preset ultrafast -pix_fmt yuv420p10le -x265-params "log-level=error:hdr10=1:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:master-display=G(13250,34500)B(7500,3000)R(34000,16000)WP(15635,16450)L(10000000,1):max-cll=1000,400" -c:a eac3 -shortest
gen hires_4k_hlg_hevc.mkv   play "4K HEVC Main10 HLG" -- -f lavfi -i testsrc2=size=3840x2160:rate=25:duration=3 -f lavfi -i sine=d=3:r=48000 -c:v libx265 -preset ultrafast -pix_fmt yuv420p10le -x265-params "log-level=error:colorprim=bt2020:transfer=arib-std-b67:colormatrix=bt2020nc" -c:a aac -shortest
gen hires_4k_vp9_10bit.webm play "4K VP9 Profile 2 (10-bit)" -- -f lavfi -i testsrc2=size=3840x2160:rate=24:duration=3 -f lavfi -i sine=d=3:r=48000 -c:v libvpx-vp9 -pix_fmt yuv420p10le -profile:v 2 -deadline realtime -cpu-used 8 -b:v 8M -c:a libopus -shortest
gen hires_8k_av1.mkv        play "8K (7680x4320) AV1" -- -f lavfi -i testsrc2=size=7680x4320:rate=24:duration=2 -f lavfi -i sine=d=2:r=48000 -c:v libsvtav1 -preset 12 -svtav1-params lp=4 -c:a libopus -shortest
gen hires_1080p120_h264.mp4 play "1080p 120 fps H.264" -- -f lavfi -i testsrc2=size=1920x1080:rate=120:duration=3 -f lavfi -i sine=d=3:r=48000 -c:v libx264 -preset ultrafast -pix_fmt yuv420p -c:a aac -shortest

echo "== audio"
gen a_mp3.mp3               play "MP3 (LAME VBR)" -- $A -c:a libmp3lame -q:a 2
gen a_mp2.mp2               play "MPEG-1 Layer II" -- $A -c:a mp2
gen a_aac.aac               play "AAC-LC ADTS" -- $A -c:a aac
gen a_aac.m4a               play "M4A AAC-LC" -- $A -c:a aac
gen a_alac_24_96.m4a        play "M4A ALAC 24-bit/96 kHz (hi-res)" -- -f lavfi -i sine=d=$D:r=96000 -c:a alac -sample_fmt s32p -bits_per_raw_sample 24
gen a_flac_24_192.flac      play "FLAC 24-bit/192 kHz (hi-res)" -- -f lavfi -i sine=d=$D:r=192000 -c:a flac -sample_fmt s32 -bits_per_raw_sample 24
gen a_flac_16_44.flac       play "FLAC 16-bit/44.1 kHz" -- -f lavfi -i sine=d=$D:r=44100 -c:a flac
gen a_wav_s16.wav           play "WAV PCM 16-bit" -- $A -c:a pcm_s16le
gen a_wav_f32_384k.wav      play "WAV PCM 32-bit float/384 kHz (hi-res)" -- -f lavfi -i sine=d=$D:r=384000 -c:a pcm_f32le
gen a_w64_s24.w64           play "Sony Wave64 PCM 24-bit" -- $A -c:a pcm_s24le
gen a_aiff_s24.aiff         play "AIFF PCM 24-bit" -- $A -c:a pcm_s24be
gen a_caf_alac.caf          play "CAF ALAC" -- $A -c:a alac
gen a_opus.opus             play "Opus" -- $A -c:a libopus
gen a_vorbis.ogg            play "Ogg Vorbis" -- $A -c:a libvorbis
gen a_flac.oga              play "Ogg FLAC" -- $A -c:a flac
gen a_speex.spx             play "Ogg Speex" -- -f lavfi -i sine=d=$D:r=16000 -c:a libspeex
gen a_wma.wma               play "WMA v2" -- $A -c:a wmav2
gen a_wavpack.wv            play "WavPack" -- $A -c:a wavpack
gen a_tta.tta               play "True Audio (TTA)" -- $A -c:a tta
gen a_amrnb.amr             play "AMR-NB" -- -f lavfi -i sine=d=$D:r=8000 -c:a libopencore_amrnb -ac 1 -b:a 12.2k
gen a_au.au                 play "Sun AU PCM" -- $A -c:a pcm_s16be
gen a_mka_opus_51.mka       play "MKA Opus 5.1" -- $A -af "$(surround 5.1 6)" -c:a libopus -mapping_family 1
gen a_mka_flac_71.mka       play "MKA FLAC 7.1" -- $A -af "$(surround 7.1 8)" -c:a flac
gen a_aptx.aptx             unsupported "Raw aptX: no aptX demuxer/decoder in libVLC 3" -- $A -c:a aptx

echo "== Dolby & DTS"
gen dolby_ac3_51.ac3        play "Dolby Digital (AC-3) 5.1 448 kb/s, raw" -- $A -af "$(surround 5.1 6)" -c:a ac3 -b:a 448k
gen dolby_ac3_20.mkv        play "Dolby Digital (AC-3) 2.0 in MKV" -- $V $A -c:v libx264 -pix_fmt yuv420p -c:a ac3 -shortest
gen dolby_eac3_51.eac3      play "Dolby Digital Plus (E-AC-3) 5.1, raw" -- $A -af "$(surround 5.1 6)" -c:a eac3 -b:a 640k
gen dolby_eac3_51.mp4       play "Dolby Digital Plus (E-AC-3) 5.1 in MP4 (ec-3)" -- $V $A -af "$(surround 5.1 6)" -c:v libx264 -pix_fmt yuv420p -c:a eac3 -shortest
gen dolby_truehd_51.mkv     play "Dolby TrueHD 5.1 (lossless) in MKV" -- $V $A -af "$(surround 5.1 6)" -c:v libx264 -pix_fmt yuv420p -c:a truehd -strict -2 -shortest
gen dolby_truehd_51.thd     play "Dolby TrueHD 5.1, raw" -- $A -af "$(surround 5.1 6)" -c:a truehd -strict -2
gen dolby_mlp_20.mlp        play "Meridian Lossless Packing (MLP) 2.0" -- $A -c:a mlp -strict -2
gen dts_51.dts              play "DTS 5.1 (core), raw" -- $A -af "$(surround 5.1 6)" -c:a dca -strict -2
gen dts_51.mkv              play "DTS 5.1 in MKV" -- $V $A -af "$(surround 5.1 6)" -c:v libx264 -pix_fmt yuv420p -c:a dca -strict -2 -shortest

echo "== IAMF (Eclipsa Audio)"
S="sine=frequency=330:d=$D:r=48000"
MIX_ST="type=iamf_mix_presentation:id=2:stg=0:annotations=en-us=Main,submix=parameter_id=100:parameter_rate=48000|element=stg=0:parameter_id=100:annotations=en-us=Bed|layout=sound_system=stereo"
gen iamf_stereo_opus.iamf   partial "IAMF via libavformat: libVLC decodes ONE substream as plain audio; no mix presentation, demixing, ambisonics or binaural rendering (stereo Opus)" -- -f lavfi -i "$S" -ac 2 -c:a libopus -stream_group "type=iamf_audio_element:id=1:st=0,layer=ch_layout=stereo" -stream_group "$MIX_ST" -streamid 0:0
gen iamf_stereo_opus.mp4    unsupported "IAMF-in-MP4 (iamf sample entry): not decoded by libVLC 3" -- -f lavfi -i "$S" -ac 2 -c:a libopus -stream_group "type=iamf_audio_element:id=1:st=0,layer=ch_layout=stereo" -stream_group "$MIX_ST" -streamid 0:0
gen iamf_stereo_lpcm.iamf   partial "IAMF via libavformat: libVLC decodes ONE substream as plain audio; no mix presentation, demixing, ambisonics or binaural rendering (stereo LPCM 24-bit)" -- -f lavfi -i "$S" -ac 2 -c:a pcm_s24le -stream_group "type=iamf_audio_element:id=1:st=0,layer=ch_layout=stereo" -stream_group "$MIX_ST" -streamid 0:0
gen iamf_51_flac.iamf       partial "IAMF via libavformat: libVLC decodes ONE substream as plain audio; no mix presentation, demixing, ambisonics or binaural rendering (scalable stereo→5.1 FLAC, 4 substreams: only the first is heard)" -- -f lavfi -i "$S" -f lavfi -i "$S" -f lavfi -i "$S" -f lavfi -i "$S" -filter_complex "[0]pan=stereo|c0=c0|c1=c0[f];[1]pan=stereo|c0=c0|c1=c0[b];[2]anull[c];[3]anull[l]" -map "[f]" -map "[b]" -map "[c]" -map "[l]" -c:a flac -stream_group "type=iamf_audio_element:id=1:st=0:st=1:st=2:st=3,demixing=parameter_id=998,recon_gain=parameter_id=101,layer=ch_layout=stereo,layer=ch_layout=5.1(side)" -stream_group "type=iamf_mix_presentation:id=2:stg=0:annotations=en-us=Main,submix=parameter_id=100:parameter_rate=48000|element=stg=0:parameter_id=100:annotations=en-us=Bed|layout=sound_system=stereo|layout=sound_system=5.1(side)" -streamid 0:0 -streamid 1:1 -streamid 2:2 -streamid 3:3
gen iamf_foa_opus.iamf      partial "IAMF via libavformat: libVLC decodes ONE substream as plain audio; no mix presentation, demixing, ambisonics or binaural rendering (first-order ambisonics, 4 mono substreams: only W is heard)" -- -f lavfi -i "$S" -f lavfi -i "$S" -f lavfi -i "$S" -f lavfi -i "$S" -map 0 -map 1 -map 2 -map 3 -c:a libopus -stream_group "type=iamf_audio_element:id=1:st=0:st=1:st=2:st=3:audio_element_type=scene,layer=ch_layout=ambisonic 1:ambisonics_mode=mono" -stream_group "type=iamf_mix_presentation:id=2:stg=0:annotations=en-us=Main,submix=parameter_id=100:parameter_rate=48000|element=stg=0:parameter_id=100:annotations=en-us=FOA|layout=sound_system=stereo" -streamid 0:0 -streamid 1:1 -streamid 2:2 -streamid 3:3

echo "== MIDI"
python - <<'PY'
import struct
ev=b''
for i,n in enumerate([60,64,67,72,67,64,60]):
    ev+=bytes([0x00,0x90,n,100])+bytes([0x83,0x60,0x80,n,0])  # 480-tick notes
ev+=bytes([0x00,0xFF,0x2F,0x00])
trk=b'MTrk'+struct.pack('>I',len(ev))+ev
open('midi_scale.mid','wb').write(b'MThd'+struct.pack('>IHHH',6,0,1,480)+trk)
PY
printf 'midi_scale.mid\tunsupported\tMIDI: libVLC fluidsynth needs a SoundFont (none configured)\n' >> "$MAN"

rm -f .err
echo "== robustness"
cd "$ROOT/robust"
: > zero_bytes.mp4
head -c 65536 /dev/urandom > random_bytes.mkv
head -c 300000 ../formats/mp4_h264_aac.mp4 > truncated.mp4
cp ../formats/mkv_h264_flac.mkv corrupt_middle.mkv
python - <<'PY'
import os
p='corrupt_middle.mkv'; n=os.path.getsize(p); b=bytearray(open(p,'rb').read())
for off in range(n//3, n//3+60000, 997): b[off:off+64]=os.urandom(64)
open(p,'wb').write(b)
PY
cp ../formats/a_mp3.mp3 mp3_named_as.mp4
cp ../formats/mp4_h264_aac.mp4 "weird #1 100% [x] & y's {z}.mp4"
cp ../formats/mp4_h264_aac.mp4 "😀 émoji ünïcødé 日本語 ファイル.mp4"
LONG="$ROOT/robust/$(printf 'very_long_directory_name_%02d/' 1 2 3 4 5 6 7 8 9 10)"
mkdir -p "$LONG" && cp ../formats/a_opus.opus "$LONG/deep_file_beyond_260_characters.opus"
[ -s long_3h.opus ] || ffmpeg -hide_banner -loglevel error -y -nostdin -f lavfi -i "sine=frequency=220:duration=10800:sample_rate=16000" -c:a libopus -b:a 6k -application voip long_3h.opus && echo "  + long_3h.opus"

echo "== playlists"
cd "$ROOT/playlists"
WROOT="$(cygpath -w "$ROOT")"
printf '#EXTM3U\n#EXTINF:6,H.264\n../formats/mp4_h264_aac.mp4\n#EXTINF:6,FLAC\n../formats/a_flac_16_44.flac\n../formats/webm_vp9_opus.webm\n' > relative.m3u
printf '\xef\xbb\xbf#EXTM3U\n%s\n%s\n' "$WROOT\\formats\\a_opus.opus" "$WROOT\\robust\\😀 émoji ünïcødé 日本語 ファイル.mp4" > unicode.m3u8
printf '[playlist]\nFile1=../formats/a_mp3.mp3\nTitle1=MP3\nFile2=../formats/a_vorbis.ogg\nNumberOfEntries=2\nVersion=2\n' > list.pls
WROOT="$WROOT" python - <<'PY'
import os, urllib.parse
root = os.environ["WROOT"]
items = [os.path.join(root, "formats", "a_flac_24_192.flac"), os.path.join(root, "formats", "mkv_h264_ass.mkv")]
uri = lambda p: "file:///" + urllib.parse.quote(p.replace("\\", "/"), safe="/:")
loc = "".join(f"<track><location>{uri(p)}</location></track>" for p in items)
open("list.xspf", "w", encoding="utf-8").write(
    f'<?xml version="1.0" encoding="UTF-8"?><playlist version="1" xmlns="http://xspf.org/ns/0/"><trackList>{loc}</trackList></playlist>')
PY

printf '\nGenerated %d format files (%d skipped). Manifest: test-media/formats/EXPECT.tsv\n' "$ok" "$bad"
