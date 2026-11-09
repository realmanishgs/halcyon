//! Immersive Audio Lab analysis: ITU-R BS.1770-4 / EBU R128 loudness
//! (momentary, short-term, integrated, loudness range), sample and true peak,
//! clipping and silence detection, with JSON/CSV reports.
//!
//! Audio is decoded out of process by FFmpeg to 48 kHz float and analysed as
//! a stream, so hour-long files use constant memory. Results are checked
//! against FFmpeg's own `ebur128` filter in the tests.

use crate::probe;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};

const RATE: usize = 48_000;
const STEP: usize = RATE / 10; // 100 ms sub-block

/// One biquad, direct form I.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 2]) -> Self {
        Self { b, a, ..Default::default() }
    }
    #[inline]
    fn run(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1] - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// K-weighting for 48 kHz (BS.1770-4, Table 1 and 2).
fn k_filter() -> (Biquad, Biquad) {
    (
        Biquad::new([1.53512485958697, -2.69169618940638, 1.19839281085285], [-1.69065929318241, 0.73248077421585]),
        Biquad::new([1.0, -2.0, 1.0], [-1.99004745483398, 0.99007225036621]),
    )
}

/// Channel weights by FFmpeg channel order for a layout name. LFE is
/// excluded; surround channels get +1.5 dB (1.41) per BS.1770-4.
pub fn channel_weights(layout: Option<&str>, channels: usize) -> Vec<f64> {
    let names: Option<&[&str]> = match layout.unwrap_or("") {
        "mono" => Some(&["FC"]),
        "stereo" => Some(&["FL", "FR"]),
        "2.1" => Some(&["FL", "FR", "LFE"]),
        "3.0" => Some(&["FL", "FR", "FC"]),
        "3.1" => Some(&["FL", "FR", "FC", "LFE"]),
        "quad" => Some(&["FL", "FR", "BL", "BR"]),
        "quad(side)" => Some(&["FL", "FR", "SL", "SR"]),
        "4.0" => Some(&["FL", "FR", "FC", "BC"]),
        "4.1" => Some(&["FL", "FR", "FC", "LFE", "BC"]),
        "5.0" => Some(&["FL", "FR", "FC", "BL", "BR"]),
        "5.0(side)" => Some(&["FL", "FR", "FC", "SL", "SR"]),
        "5.1" => Some(&["FL", "FR", "FC", "LFE", "BL", "BR"]),
        "5.1(side)" => Some(&["FL", "FR", "FC", "LFE", "SL", "SR"]),
        "6.1" => Some(&["FL", "FR", "FC", "LFE", "BC", "SL", "SR"]),
        "7.1" => Some(&["FL", "FR", "FC", "LFE", "BL", "BR", "SL", "SR"]),
        "7.1(wide)" | "7.1(wide-side)" => Some(&["FL", "FR", "FC", "LFE", "BL", "BR", "FLC", "FRC"]),
        _ => None,
    };
    match names {
        Some(n) if n.len() == channels => n
            .iter()
            .map(|c| match *c {
                "LFE" => 0.0,
                "BL" | "BR" | "SL" | "SR" | "BC" => 1.41,
                _ => 1.0,
            })
            .collect(),
        // Unknown layout: assume the common SMPTE order, LFE fourth from 6 channels up.
        _ => (0..channels).map(|i| if channels >= 6 && i == 3 { 0.0 } else if channels >= 5 && i >= 4 { 1.41 } else { 1.0 }).collect(),
    }
}

/// 4× oversampling interpolator for true-peak (polyphase windowed sinc,
/// 4 phases × 24 taps, Kaiser β = 7).
struct TruePeak {
    phases: [[f64; 24]; 4],
    hist: Vec<[f64; 24]>,
    pos: usize,
    pub max: Vec<f64>,
}

fn bessel_i0(x: f64) -> f64 {
    let mut s = 1.0;
    let mut t = 1.0;
    for k in 1..40 {
        t *= (x / 2.0) / k as f64;
        s += t * t;
    }
    s
}

impl TruePeak {
    fn new(channels: usize) -> Self {
        let n = 96;
        let beta = 7.0;
        let mut h = [0.0f64; 96];
        for (i, v) in h.iter_mut().enumerate() {
            let m = i as f64 - (n as f64 - 1.0) / 2.0;
            let x = m / 4.0; // cutoff at the original Nyquist
            let sinc = if x.abs() < 1e-12 { 1.0 } else { (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x) };
            let r = 2.0 * i as f64 / (n as f64 - 1.0) - 1.0;
            *v = sinc * bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / bessel_i0(beta);
        }
        let mut phases = [[0.0; 24]; 4];
        for (p, ph) in phases.iter_mut().enumerate() {
            for (k, c) in ph.iter_mut().enumerate() {
                *c = h[k * 4 + p];
            }
        }
        // Each phase passes DC at unity.
        for ph in phases.iter_mut() {
            let s: f64 = ph.iter().sum();
            ph.iter_mut().for_each(|c| *c /= s);
        }
        Self { phases, hist: vec![[0.0; 24]; channels], pos: 0, max: vec![0.0; channels] }
    }

    #[inline]
    fn push(&mut self, ch: usize, x: f64) {
        let h = &mut self.hist[ch];
        h[self.pos] = x;
        for ph in &self.phases {
            let mut acc = 0.0;
            for (k, c) in ph.iter().enumerate() {
                acc += c * h[(self.pos + 24 - k) % 24];
            }
            let a = acc.abs();
            if a > self.max[ch] {
                self.max[ch] = a;
            }
        }
    }

    fn advance(&mut self) {
        self.pos = (self.pos + 1) % 24;
    }
}

fn lufs(power: f64) -> f64 {
    if power <= 0.0 {
        f64::NEG_INFINITY
    } else {
        -0.691 + 10.0 * power.log10()
    }
}

fn db(x: f64) -> f64 {
    if x <= 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * x.log10()
    }
}

/// JSON cannot carry -inf: report silence as -200.
fn finite(v: f64) -> f64 {
    if v.is_finite() {
        (v * 100.0).round() / 100.0
    } else {
        -200.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipEvent {
    pub time_ms: i64,
    pub channel: usize,
    pub samples: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    pub path: String,
    pub stream: Option<usize>,
    pub channels: usize,
    pub layout: Option<String>,
    pub duration_ms: i64,
    pub integrated_lufs: f64,
    pub loudness_range_lu: f64,
    pub max_momentary_lufs: f64,
    pub max_short_term_lufs: f64,
    pub sample_peak_dbfs: f64,
    pub true_peak_dbtp: f64,
    pub channel_true_peak_dbtp: Vec<f64>,
    pub channel_rms_dbfs: Vec<f64>,
    pub channel_dc_offset: Vec<f64>,
    pub clip_events: Vec<ClipEvent>,
    pub clipped_samples: u64,
    /// (start ms, end ms) of stretches below −60 dBFS lasting at least 2 s.
    pub silences: Vec<(i64, i64)>,
    /// One entry per 100 ms: (momentary LUFS, short-term LUFS, sample peak dBFS).
    pub timeline: Vec<(f64, f64, f64)>,
}

struct Meter {
    ch: usize,
    w: Vec<f64>,
    filt: Vec<(Biquad, Biquad)>,
    tp: TruePeak,
    sub_power: Vec<f64>, // weighted mean-square of each 100 ms sub-block
    acc: Vec<f64>,
    acc_n: usize,
    peak_block: f64,
    timeline: Vec<(f64, f64, f64)>,
    sample_peak: f64,
    sumsq: Vec<f64>,
    sum: Vec<f64>,
    frames: u64,
    clip_run: Vec<usize>,
    clips: Vec<ClipEvent>,
    clipped: u64,
}

impl Meter {
    fn new(ch: usize, w: Vec<f64>) -> Self {
        Self {
            ch,
            w,
            filt: (0..ch).map(|_| k_filter()).collect(),
            tp: TruePeak::new(ch),
            sub_power: Vec::new(),
            acc: vec![0.0; ch],
            acc_n: 0,
            peak_block: 0.0,
            timeline: Vec::new(),
            sample_peak: 0.0,
            sumsq: vec![0.0; ch],
            sum: vec![0.0; ch],
            frames: 0,
            clip_run: vec![0; ch],
            clips: Vec::new(),
            clipped: 0,
        }
    }

    fn frame(&mut self, f: &[f32]) {
        for c in 0..self.ch {
            let x = f[c] as f64;
            let a = x.abs();
            if a > self.sample_peak {
                self.sample_peak = a;
            }
            if a > self.peak_block {
                self.peak_block = a;
            }
            self.sumsq[c] += x * x;
            self.sum[c] += x;
            // Clipping: 3+ consecutive samples at digital full scale.
            if a >= 0.9999 {
                self.clip_run[c] += 1;
            } else {
                self.end_clip(c);
            }
            let (s1, s2) = &mut self.filt[c];
            let y = s2.run(s1.run(x));
            self.acc[c] += y * y;
            self.tp.push(c, x);
        }
        self.tp.advance();
        self.frames += 1;
        self.acc_n += 1;
        if self.acc_n == STEP {
            self.close_block();
        }
    }

    fn end_clip(&mut self, c: usize) {
        let n = self.clip_run[c];
        if n >= 3 {
            self.clipped += n as u64;
            if self.clips.len() < 200 {
                let t = (self.frames as i64 - n as i64) * 1000 / RATE as i64;
                self.clips.push(ClipEvent { time_ms: t, channel: c, samples: n });
            }
        }
        self.clip_run[c] = 0;
    }

    fn close_block(&mut self) {
        let p: f64 = (0..self.ch).map(|c| self.w[c] * self.acc[c] / STEP as f64).sum();
        self.sub_power.push(p);
        self.acc.iter_mut().for_each(|a| *a = 0.0);
        self.acc_n = 0;
        let n = self.sub_power.len();
        let mean = |k: usize| self.sub_power[n.saturating_sub(k)..].iter().sum::<f64>() / k as f64;
        let m = if n >= 4 { lufs(mean(4)) } else { f64::NEG_INFINITY };
        let s = if n >= 30 { lufs(mean(30)) } else { f64::NEG_INFINITY };
        self.timeline.push((m, s, db(self.peak_block)));
        self.peak_block = 0.0;
    }

    fn finish(mut self, path: &str, stream: Option<usize>, layout: Option<String>) -> Analysis {
        for c in 0..self.ch {
            self.end_clip(c);
        }
        // Gating blocks: 400 ms (4 sub-blocks), 75 % overlap.
        let sp = &self.sub_power;
        let blocks: Vec<f64> = (3..sp.len()).map(|i| sp[i - 3..=i].iter().sum::<f64>() / 4.0).collect();
        let abs: Vec<f64> = blocks.iter().copied().filter(|p| lufs(*p) > -70.0).collect();
        let integrated = if abs.is_empty() {
            f64::NEG_INFINITY
        } else {
            let rel = lufs(abs.iter().sum::<f64>() / abs.len() as f64) - 10.0;
            let g: Vec<f64> = abs.into_iter().filter(|p| lufs(*p) > rel).collect();
            if g.is_empty() { f64::NEG_INFINITY } else { lufs(g.iter().sum::<f64>() / g.len() as f64) }
        };
        // Loudness range (EBU Tech 3342): 3 s short-term values every 100 ms.
        let st: Vec<f64> = (29..sp.len()).map(|i| sp[i - 29..=i].iter().sum::<f64>() / 30.0).collect();
        let st_abs: Vec<f64> = st.iter().copied().filter(|p| lufs(*p) > -70.0).collect();
        let lra = if st_abs.is_empty() {
            0.0
        } else {
            let rel = lufs(st_abs.iter().sum::<f64>() / st_abs.len() as f64) - 20.0;
            let mut v: Vec<f64> = st_abs.into_iter().map(lufs).filter(|l| *l > rel).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if v.len() < 2 {
                0.0
            } else {
                let pct = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
                pct(0.95) - pct(0.10)
            }
        };
        // Silence: 100 ms blocks with peak below −60 dBFS, runs of 2 s or more.
        let mut silences = Vec::new();
        let mut start: Option<usize> = None;
        for (i, t) in self.timeline.iter().chain(std::iter::once(&(0.0, 0.0, 0.0))).enumerate() {
            let quiet = t.2 < -60.0 && i < self.timeline.len();
            match (quiet, start) {
                (true, None) => start = Some(i),
                (false, Some(s)) => {
                    if i - s >= 20 {
                        silences.push((s as i64 * 100, i as i64 * 100));
                    }
                    start = None;
                }
                _ => {}
            }
        }
        let n = self.frames.max(1) as f64;
        let max_m = self.timeline.iter().map(|t| t.0).fold(f64::NEG_INFINITY, f64::max);
        let max_s = self.timeline.iter().map(|t| t.1).fold(f64::NEG_INFINITY, f64::max);
        let tp = self.tp.max.iter().copied().fold(0.0, f64::max).max(self.sample_peak);
        Analysis {
            path: path.to_string(),
            stream,
            channels: self.ch,
            layout,
            duration_ms: (self.frames * 1000 / RATE as u64) as i64,
            integrated_lufs: finite(integrated),
            loudness_range_lu: finite(lra),
            max_momentary_lufs: finite(max_m),
            max_short_term_lufs: finite(max_s),
            sample_peak_dbfs: finite(db(self.sample_peak)),
            true_peak_dbtp: finite(db(tp)),
            channel_true_peak_dbtp: self.tp.max.iter().map(|m| finite(db(*m))).collect(),
            channel_rms_dbfs: self.sumsq.iter().map(|s| finite(10.0 * (s / n).log10())).collect(),
            channel_dc_offset: self.sum.iter().map(|s| (s / n * 1e6).round() / 1e6).collect(),
            clip_events: self.clips,
            clipped_samples: self.clipped,
            silences,
            timeline: self.timeline.iter().map(|t| (finite(t.0), finite(t.1), finite(t.2))).collect(),
        }
    }
}

/// Analyses audio stream `stream` (index among audio streams; None = first).
/// `progress` receives 0..1; setting `cancel` stops early with an error.
pub fn analyze(path: &str, stream: Option<usize>, cancel: &AtomicBool, progress: impl Fn(f32)) -> Result<Analysis, String> {
    let info = probe::probe(path)?;
    let audio: Vec<&probe::Stream> = info.streams.iter().filter(|s| s.codec_type.as_deref() == Some("audio")).collect();
    let s = audio.get(stream.unwrap_or(0)).ok_or("No such audio stream")?;
    let channels = s.channels.unwrap_or(2).max(1) as usize;
    let layout = s.channel_layout.clone();
    let duration = info.format.duration.as_deref().and_then(|d| d.parse::<f64>().ok()).unwrap_or(0.0);
    let ff = probe::ffmpeg_path().ok_or("ffmpeg not found (install FFmpeg or set HALCYON_FFMPEG)")?;
    let mut child = probe::quiet(ff)
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(crate::engine::plain_path(path))
        .args(["-map", &format!("0:a:{}", stream.unwrap_or(0)), "-vn", "-sn", "-dn", "-f", "f32le", "-acodec", "pcm_f32le", "-ar", "48000"])
        .args(["-ac", &channels.to_string(), "pipe:1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("ffmpeg failed to start: {e}"))?;
    let mut out = child.stdout.take().unwrap();
    let mut m = Meter::new(channels, channel_weights(layout.as_deref(), channels));
    let frame_bytes = 4 * channels;
    let mut buf = vec![0u8; frame_bytes * 4800];
    let mut carry = Vec::<u8>::new();
    let mut frame = vec![0f32; channels];
    let mut last = std::time::Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            return Err("Cancelled".into());
        }
        let n = out.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        carry.extend_from_slice(&buf[..n]);
        let whole = carry.len() / frame_bytes * frame_bytes;
        for chunk in carry[..whole].chunks_exact(frame_bytes) {
            for (c, v) in frame.iter_mut().enumerate() {
                *v = f32::from_le_bytes(chunk[c * 4..c * 4 + 4].try_into().unwrap());
            }
            m.frame(&frame);
        }
        carry.drain(..whole);
        if duration > 0.0 && last.elapsed().as_millis() > 200 {
            progress((m.frames as f64 / RATE as f64 / duration).min(1.0) as f32);
            last = std::time::Instant::now();
        }
    }
    let _ = child.wait();
    if m.frames == 0 {
        return Err("No audio could be decoded".into());
    }
    progress(1.0);
    Ok(m.finish(path, stream, layout))
}

pub fn report_json(a: &Analysis) -> String {
    serde_json::to_string_pretty(a).unwrap_or_default()
}

pub fn report_csv(a: &Analysis) -> String {
    let mut s = String::from("time_s,momentary_lufs,short_term_lufs,sample_peak_dbfs\n");
    for (i, t) in a.timeline.iter().enumerate() {
        s.push_str(&format!("{:.1},{},{},{}\n", i as f64 / 10.0, t.0, t.1, t.2));
    }
    s
}

/// Decodes one audio stream to a 24-bit WAV (used to audition IAMF substreams).
pub fn extract_stream(path: &str, stream: usize, out: &std::path::Path) -> Result<(), String> {
    let ff = probe::ffmpeg_path().ok_or("ffmpeg not found")?;
    let o = probe::quiet(ff)
        .args(["-v", "error", "-nostdin", "-y", "-i"])
        .arg(crate::engine::plain_path(path))
        .args(["-map", &format!("0:a:{stream}"), "-c:a", "pcm_s24le"])
        .arg(out)
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ff() -> Option<PathBuf> {
        probe::ffmpeg_path()
    }

    fn gen(name: &str, args: &[&str]) -> Option<String> {
        let ff = ff()?;
        let dir = std::env::temp_dir().join("halcyon-lab-tests");
        std::fs::create_dir_all(&dir).ok()?;
        let out = dir.join(name);
        let ok = probe::quiet(ff).args(["-v", "error", "-y", "-nostdin"]).args(args).arg(&out).status().ok()?.success();
        ok.then(|| out.to_string_lossy().into_owned())
    }

    /// FFmpeg's ebur128 summary: (integrated LUFS, LRA LU, true peak dBFS).
    fn ffmpeg_ebur128(path: &str) -> (f64, f64, f64) {
        let o = probe::quiet(ff().unwrap())
            .args(["-nostdin", "-hide_banner", "-i", path, "-map", "0:a:0", "-af", "ebur128=peak=true", "-f", "null", "-"])
            .output()
            .unwrap();
        let t = String::from_utf8_lossy(&o.stderr);
        let summary = &t[t.rfind("Summary:").expect("ebur128 summary")..];
        let val = |key: &str| -> f64 {
            let i = summary.find(key).unwrap() + key.len();
            summary[i..].split_whitespace().next().unwrap().parse().unwrap()
        };
        let tp_section = &summary[summary.find("True peak:").unwrap()..];
        let i = tp_section.find("Peak:").unwrap() + 5;
        let tp: f64 = tp_section[i..].split_whitespace().next().unwrap().parse().unwrap();
        (val("I:"), val("LRA:"), tp)
    }

    fn run(path: &str) -> Analysis {
        analyze(path, None, &AtomicBool::new(false), |_| {}).unwrap()
    }

    #[test]
    fn ebu_reference_sine_measures_minus_23() {
        // EBU Tech 3341 case 1: stereo 1 kHz sine at −23 dBFS → −23.0 LUFS (±0.1).
        let Some(p) = gen("sine-23.wav", &["-f", "lavfi", "-i", "aevalsrc=0.0707946*sin(2*PI*1000*t)|0.0707946*sin(2*PI*1000*t):s=48000:d=20", "-c:a", "pcm_f32le"]) else {
            eprintln!("skipped: ffmpeg not found");
            return;
        };
        let a = run(&p);
        assert!((a.integrated_lufs + 23.0).abs() <= 0.1, "{}", a.integrated_lufs);
        assert!(a.loudness_range_lu.abs() <= 0.1);
        assert!((a.sample_peak_dbfs + 23.0).abs() < 0.05);
        assert!(a.clip_events.is_empty() && a.silences.is_empty());
    }

    #[test]
    fn matches_ffmpeg_ebur128_on_dynamic_surround_material() {
        // 5.1 noise + tones with level changes over 40 s.
        let Some(p) = gen(
            "dyn51.wav",
            &[
                "-f", "lavfi", "-i", "anoisesrc=d=40:c=pink:r=48000:a=0.3",
                "-f", "lavfi", "-i", "sine=f=440:d=40:r=48000",
                "-filter_complex",
                "[0][1]amix=inputs=2,volume='if(lt(t,10),0.2,if(lt(t,25),1,0.05))':eval=frame,pan=5.1|FL=c0|FR=c0|FC=0.5*c0|LFE=c0|BL=0.7*c0|BR=0.3*c0",
                "-c:a", "pcm_f32le",
            ],
        ) else {
            eprintln!("skipped: ffmpeg not found");
            return;
        };
        let a = run(&p);
        let (i, lra, tp) = ffmpeg_ebur128(&p);
        assert_eq!(a.channels, 6);
        assert!((a.integrated_lufs - i).abs() <= 0.2, "I ours {} ffmpeg {i}", a.integrated_lufs);
        assert!((a.loudness_range_lu - lra).abs() <= 0.6, "LRA ours {} ffmpeg {lra}", a.loudness_range_lu);
        assert!((a.true_peak_dbtp - tp).abs() <= 0.3, "TP ours {} ffmpeg {tp}", a.true_peak_dbtp);
    }

    #[test]
    fn inter_sample_peaks_exceed_sample_peak() {
        // fs/4 sine at 45° phase: samples sit at ±0.707 of the true amplitude.
        let Some(p) = gen("isp.wav", &["-f", "lavfi", "-i", "aevalsrc=0.9*sin(2*PI*12000*t+PI/4):s=48000:d=5", "-c:a", "pcm_f32le"]) else {
            return;
        };
        let a = run(&p);
        assert!((a.sample_peak_dbfs - db(0.9 * 0.70710678)).abs() < 0.05, "{}", a.sample_peak_dbfs);
        assert!((a.true_peak_dbtp - db(0.9)).abs() < 0.2, "{}", a.true_peak_dbtp);
    }

    #[test]
    fn finds_clipping_and_silence() {
        let Some(p) = gen(
            "clip-silence.wav",
            &[
                "-f", "lavfi", "-i", "aevalsrc=if(lt(t\\,2)\\,1.6*sin(2*PI*200*t)\\,if(lt(t\\,6)\\,0\\,0.3*sin(2*PI*200*t))):s=48000:d=8",
                "-c:a", "pcm_s16le",
            ],
        ) else {
            return;
        };
        let a = run(&p);
        assert!(a.clipped_samples > 1000 && !a.clip_events.is_empty(), "{}", a.clipped_samples);
        assert!(a.clip_events[0].time_ms < 2000);
        assert_eq!(a.silences.len(), 1, "{:?}", a.silences);
        let (s, e) = a.silences[0];
        assert!((1900..=2100).contains(&s) && (5900..=6100).contains(&e), "{s}-{e}");
        assert!(report_csv(&a).lines().count() > 70);
    }
}
