//! libVLC 3.x adapter, loaded dynamically at runtime.
//!
//! Signatures are transcribed from the VLC 3.0.x headers
//! (include/vlc/libvlc*.h, branch `3.0.x`). Loading at runtime means we link
//! against no VLC import library and the user's VLC (LGPL libvlc + its
//! plugin set) stays a separately replaceable component.

use super::*;
use libloading::Library;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::path::{Path, PathBuf};

type Inst = c_void;
type Media = c_void;
type Player = c_void;

#[repr(C)]
struct TrackDescription {
    id: c_int,
    name: *mut c_char,
    next: *mut TrackDescription,
}

// libvlc_meta_t values used here.
const META_TITLE: c_int = 0;
const META_ARTIST: c_int = 1;
const META_GENRE: c_int = 2;
const META_ALBUM: c_int = 4;
const META_DATE: c_int = 8;
const META_ARTWORK_URL: c_int = 15;

// libvlc_state_t values used here.
const STATE_PLAYING: c_int = 3;
const STATE_STOPPED: c_int = 5;
const STATE_ENDED: c_int = 6;
const STATE_ERROR: c_int = 7;

const SLAVE_SUBTITLE: c_int = 0; // libvlc_media_slave_type_subtitle

/// `libvlc_media_stats_t` (libvlc_media.h, VLC 3.0.x).
#[repr(C)]
#[derive(Default)]
struct RawStats {
    read_bytes: c_int,
    input_bitrate: f32,
    demux_read_bytes: c_int,
    demux_bitrate: f32,
    demux_corrupted: c_int,
    demux_discontinuity: c_int,
    decoded_video: c_int,
    decoded_audio: c_int,
    displayed_pictures: c_int,
    lost_pictures: c_int,
    played_abuffers: c_int,
    lost_abuffers: c_int,
    sent_packets: c_int,
    sent_bytes: c_int,
    send_bitrate: f32,
}

/// Percent-encoded `file:///` URI for a local path (what add_slave expects).
pub fn file_uri(path: &str) -> String {
    let p = plain_path(path).replace('\\', "/");
    let mut out = String::from(if p.starts_with("//") { "file:" } else { "file:///" });
    for b in p.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

macro_rules! vlc_api {
    ($($name:ident : fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        #[allow(non_snake_case)]
        struct Api { $($name: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)* }
        impl Api {
            unsafe fn load(lib: &Library) -> Result<Self, String> {
                Ok(Self { $($name: *lib
                    .get(concat!(stringify!($name), "\0").as_bytes())
                    .map_err(|e| format!("libVLC symbol {} missing: {e}", stringify!($name)))?,)* })
            }
        }
    };
}

vlc_api! {
    libvlc_new: fn(c_int, *const *const c_char) -> *mut Inst;
    libvlc_release: fn(*mut Inst);
    libvlc_get_version: fn() -> *const c_char;
    libvlc_errmsg: fn() -> *const c_char;
    libvlc_free: fn(*mut c_void);
    libvlc_media_new_path: fn(*mut Inst, *const c_char) -> *mut Media;
    libvlc_media_release: fn(*mut Media);
    libvlc_media_get_meta: fn(*mut Media, c_int) -> *mut c_char;
    libvlc_media_player_new: fn(*mut Inst) -> *mut Player;
    libvlc_media_player_release: fn(*mut Player);
    libvlc_media_player_set_media: fn(*mut Player, *mut Media);
    libvlc_media_player_get_media: fn(*mut Player) -> *mut Media;
    libvlc_media_player_set_hwnd: fn(*mut Player, *mut c_void);
    libvlc_media_player_play: fn(*mut Player) -> c_int;
    libvlc_media_player_set_pause: fn(*mut Player, c_int);
    libvlc_media_player_stop: fn(*mut Player);
    libvlc_media_player_get_length: fn(*mut Player) -> i64;
    libvlc_media_player_get_time: fn(*mut Player) -> i64;
    libvlc_media_player_set_time: fn(*mut Player, i64);
    libvlc_media_player_get_rate: fn(*mut Player) -> f32;
    libvlc_media_player_set_rate: fn(*mut Player, f32) -> c_int;
    libvlc_media_player_get_state: fn(*mut Player) -> c_int;
    libvlc_media_player_next_frame: fn(*mut Player);
    libvlc_audio_get_volume: fn(*mut Player) -> c_int;
    libvlc_audio_set_volume: fn(*mut Player, c_int) -> c_int;
    libvlc_audio_get_track_description: fn(*mut Player) -> *mut TrackDescription;
    libvlc_audio_get_track: fn(*mut Player) -> c_int;
    libvlc_audio_set_track: fn(*mut Player, c_int) -> c_int;
    libvlc_video_get_spu_description: fn(*mut Player) -> *mut TrackDescription;
    libvlc_video_get_spu: fn(*mut Player) -> c_int;
    libvlc_video_set_spu: fn(*mut Player, c_int) -> c_int;
    libvlc_video_set_mouse_input: fn(*mut Player, c_uint);
    libvlc_video_set_key_input: fn(*mut Player, c_uint);
    libvlc_track_description_list_release: fn(*mut TrackDescription);
    libvlc_media_new_location: fn(*mut Inst, *const c_char) -> *mut Media;
    libvlc_media_get_stats: fn(*mut Media, *mut RawStats) -> c_int;
    libvlc_media_player_has_vout: fn(*mut Player) -> c_uint;
    libvlc_media_player_is_seekable: fn(*mut Player) -> c_int;
    libvlc_media_player_get_chapter: fn(*mut Player) -> c_int;
    libvlc_media_player_get_chapter_count: fn(*mut Player) -> c_int;
    libvlc_media_player_set_chapter: fn(*mut Player, c_int);
    libvlc_media_player_add_slave: fn(*mut Player, c_int, *const c_char, bool) -> c_int;
    libvlc_audio_get_mute: fn(*mut Player) -> c_int;
    libvlc_audio_set_mute: fn(*mut Player, c_int);
    libvlc_audio_get_delay: fn(*mut Player) -> i64;
    libvlc_audio_set_delay: fn(*mut Player, i64) -> c_int;
    libvlc_video_get_spu_delay: fn(*mut Player) -> i64;
    libvlc_video_set_spu_delay: fn(*mut Player, i64) -> c_int;
    libvlc_video_set_aspect_ratio: fn(*mut Player, *const c_char);
    libvlc_video_set_deinterlace: fn(*mut Player, *const c_char);
    libvlc_video_take_snapshot: fn(*mut Player, c_uint, *const c_char, c_uint, c_uint) -> c_int;
    libvlc_video_get_size: fn(*mut Player, c_uint, *mut c_uint, *mut c_uint) -> c_int;
}

pub struct VlcEngine {
    api: Api,
    inst: *mut Inst,
    player: *mut Player,
    location: PathBuf,
    _lib: Library, // keep loaded for the lifetime of the function pointers
}

// libVLC objects are thread-safe; access is additionally serialised by a Mutex.
unsafe impl Send for VlcEngine {}

#[cfg(windows)]
const LIB_NAME: &str = "libvlc.dll";
#[cfg(target_os = "macos")]
const LIB_NAME: &str = "libvlc.dylib";
#[cfg(all(unix, not(target_os = "macos")))]
const LIB_NAME: &str = "libvlc.so.5";

/// Directories searched for libVLC, in priority order: override, a copy
/// bundled next to the executable, then the platform's standard VLC install.
fn candidate_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(d) = std::env::var("HALCYON_LIBVLC_DIR") {
        v.push(PathBuf::from(d));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            v.push(d.join("vlc"));
        }
    }
    #[cfg(windows)]
    for var in ["ProgramFiles", "ProgramW6432"] {
        if let Ok(pf) = std::env::var(var) {
            v.push(Path::new(&pf).join("VideoLAN").join("VLC"));
        }
    }
    #[cfg(target_os = "macos")]
    v.push(PathBuf::from("/Applications/VLC.app/Contents/MacOS/lib"));
    #[cfg(all(unix, not(target_os = "macos")))]
    for d in ["/usr/lib/x86_64-linux-gnu", "/usr/lib64", "/usr/lib", "/usr/local/lib", "/usr/lib/aarch64-linux-gnu"] {
        v.push(PathBuf::from(d));
    }
    v
}

#[cfg(windows)]
unsafe fn open_library(dir: &Path) -> Result<Library, String> {
    // Altered search path makes Windows resolve libvlccore.dll next to libvlc.dll.
    libloading::os::windows::Library::load_with_flags(dir.join(LIB_NAME), libloading::os::windows::LOAD_WITH_ALTERED_SEARCH_PATH)
        .map(Into::into)
        .map_err(|e| format!("Failed to load {LIB_NAME}: {e}"))
}

#[cfg(not(windows))]
unsafe fn open_library(dir: &Path) -> Result<Library, String> {
    // VLC.app keeps its plugins outside the lib folder; tell libvlccore where.
    #[cfg(target_os = "macos")]
    if std::env::var_os("VLC_PLUGIN_PATH").is_none() {
        std::env::set_var("VLC_PLUGIN_PATH", dir.join("../plugins"));
    }
    Library::new(dir.join(LIB_NAME)).map_err(|e| format!("Failed to load {LIB_NAME}: {e}"))
}

impl VlcEngine {
    pub fn load() -> Result<Self, String> {
        Self::load_with_args(&[])
    }

    /// `extra` is appended to the libVLC command line (used by headless tests).
    pub fn load_with_args(extra: &[&str]) -> Result<Self, String> {
        let dir = candidate_dirs()
            .into_iter()
            .find(|d| d.join(LIB_NAME).is_file())
            .ok_or("libVLC not found. Install VLC 3.x (64-bit) or set HALCYON_LIBVLC_DIR.")?;
        unsafe {
            let lib = open_library(&dir)?;
            let api = Api::load(&lib)?;
            let args: Vec<CString> = ["--no-video-title-show", "--no-snapshot-preview", "--no-metadata-network-access"]
                .iter()
                .chain(extra.iter())
                .map(|s| CString::new(*s).unwrap())
                .collect();
            let argv: Vec<*const c_char> = args.iter().map(|a| a.as_ptr()).collect();
            let inst = (api.libvlc_new)(argv.len() as c_int, argv.as_ptr());
            if inst.is_null() {
                return Err(format!("libvlc_new failed: {}", errmsg(&api)));
            }
            let player = (api.libvlc_media_player_new)(inst);
            if player.is_null() {
                (api.libvlc_release)(inst);
                return Err("libvlc_media_player_new failed".into());
            }
            // Let the host UI handle input; the video surface is display-only.
            (api.libvlc_video_set_mouse_input)(player, 0);
            (api.libvlc_video_set_key_input)(player, 0);
            Ok(Self { api, inst, player, location: dir, _lib: lib })
        }
    }

    fn descriptions(&self, list: *mut TrackDescription) -> Vec<TrackEntry> {
        let mut out = Vec::new();
        let mut p = list;
        unsafe {
            while !p.is_null() {
                out.push(TrackEntry { id: (*p).id, name: cstr((*p).name).unwrap_or_default() });
                p = (*p).next;
            }
            if !list.is_null() {
                (self.api.libvlc_track_description_list_release)(list);
            }
        }
        out
    }

    fn raw_state(&self) -> c_int {
        unsafe { (self.api.libvlc_media_player_get_state)(self.player) }
    }

    fn has_media(&self) -> bool {
        unsafe {
            let m = (self.api.libvlc_media_player_get_media)(self.player);
            if m.is_null() {
                return false;
            }
            (self.api.libvlc_media_release)(m);
            true
        }
    }

    fn meta_field(&self, media: *mut Media, field: c_int) -> Option<String> {
        unsafe {
            let p = (self.api.libvlc_media_get_meta)(media, field);
            let s = cstr(p);
            if !p.is_null() {
                (self.api.libvlc_free)(p as *mut c_void);
            }
            s.filter(|s| !s.is_empty())
        }
    }
}

unsafe fn cstr(p: *const c_char) -> Option<String> {
    (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned())
}

fn errmsg(api: &Api) -> String {
    unsafe { cstr((api.libvlc_errmsg)()).unwrap_or_else(|| "unknown error".into()) }
}

impl Drop for VlcEngine {
    fn drop(&mut self) {
        unsafe {
            (self.api.libvlc_media_player_stop)(self.player);
            (self.api.libvlc_media_player_release)(self.player);
            (self.api.libvlc_release)(self.inst);
        }
    }
}

impl MediaEngine for VlcEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo {
            name: "libVLC".into(),
            version: unsafe { cstr((self.api.libvlc_get_version)()).unwrap_or_default() },
            location: self.location.display().to_string(),
        }
    }

    fn open(&mut self, source: &str, surface: Option<isize>) -> Result<(), String> {
        let url = is_url(source);
        let c = CString::new(if url { source.to_string() } else { engine_path(source) })
            .map_err(|_| "Path contains NUL")?;
        unsafe {
            (self.api.libvlc_media_player_stop)(self.player);
            let media = if url {
                (self.api.libvlc_media_new_location)(self.inst, c.as_ptr())
            } else {
                (self.api.libvlc_media_new_path)(self.inst, c.as_ptr())
            };
            if media.is_null() {
                return Err(format!("Could not open media: {}", errmsg(&self.api)));
            }
            (self.api.libvlc_media_player_set_media)(self.player, media);
            (self.api.libvlc_media_release)(media); // player holds its own reference
            #[cfg(windows)]
            if let Some(h) = surface {
                (self.api.libvlc_media_player_set_hwnd)(self.player, h as *mut c_void);
            }
            #[cfg(not(windows))]
            let _ = surface; // TODO(macOS/Linux): embed via set_nsobject / set_xwindow
        }
        self.play()
    }

    fn play(&mut self) -> Result<(), String> {
        if !self.has_media() {
            return Err("Nothing to play".into());
        }
        // libVLC 3 ignores play() after the end of the media or an error: reset first.
        if matches!(self.raw_state(), STATE_ENDED | STATE_ERROR | STATE_STOPPED) {
            unsafe { (self.api.libvlc_media_player_stop)(self.player) };
        }
        match unsafe { (self.api.libvlc_media_player_play)(self.player) } {
            0 => Ok(()),
            _ => Err(format!("Playback failed: {}", errmsg(&self.api))),
        }
    }

    fn set_paused(&mut self, paused: bool) {
        unsafe { (self.api.libvlc_media_player_set_pause)(self.player, paused as c_int) }
    }

    fn stop(&mut self) {
        unsafe { (self.api.libvlc_media_player_stop)(self.player) }
    }

    fn seek_ms(&mut self, ms: i64) {
        // Seeking needs a running input; after the end (or a stop) restart first.
        if matches!(self.raw_state(), STATE_ENDED | STATE_STOPPED) && self.play().is_ok() {
            let t = std::time::Instant::now();
            while self.raw_state() != STATE_PLAYING && t.elapsed().as_millis() < 1500 {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        unsafe { (self.api.libvlc_media_player_set_time)(self.player, ms.max(0)) }
    }

    fn set_muted(&mut self, muted: bool) {
        unsafe { (self.api.libvlc_audio_set_mute)(self.player, muted as c_int) }
    }

    fn add_subtitle_file(&mut self, path: &str) -> Result<(), String> {
        let uri = CString::new(file_uri(path)).map_err(|_| "Path contains NUL")?;
        match unsafe { (self.api.libvlc_media_player_add_slave)(self.player, SLAVE_SUBTITLE, uri.as_ptr(), true) } {
            0 => Ok(()),
            _ => Err("Could not load subtitle file".into()),
        }
    }

    fn set_audio_delay_ms(&mut self, ms: i64) -> Result<(), String> {
        match unsafe { (self.api.libvlc_audio_set_delay)(self.player, ms * 1000) } {
            0 => Ok(()),
            _ => Err("Audio delay not available".into()),
        }
    }

    fn set_subtitle_delay_ms(&mut self, ms: i64) -> Result<(), String> {
        match unsafe { (self.api.libvlc_video_set_spu_delay)(self.player, ms * 1000) } {
            0 => Ok(()),
            _ => Err("Subtitle delay not available".into()),
        }
    }

    fn set_aspect_ratio(&mut self, aspect: Option<&str>) {
        let c = aspect.and_then(|a| CString::new(a).ok());
        let p = c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
        unsafe { (self.api.libvlc_video_set_aspect_ratio)(self.player, p) }
    }

    fn set_deinterlace(&mut self, mode: Option<&str>) {
        let c = mode.and_then(|a| CString::new(a).ok());
        let p = c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
        unsafe { (self.api.libvlc_video_set_deinterlace)(self.player, p) }
    }

    fn set_chapter(&mut self, chapter: i32) {
        unsafe { (self.api.libvlc_media_player_set_chapter)(self.player, chapter) }
    }

    fn snapshot(&mut self, path: &str) -> Result<(), String> {
        let c = CString::new(plain_path(path)).map_err(|_| "Path contains NUL")?;
        match unsafe { (self.api.libvlc_video_take_snapshot)(self.player, 0, c.as_ptr(), 0, 0) } {
            0 => Ok(()),
            _ => Err("No video to capture".into()),
        }
    }

    fn video_size(&self) -> Option<(u32, u32)> {
        let (mut w, mut h) = (0, 0);
        let ok = unsafe { (self.api.libvlc_video_get_size)(self.player, 0, &mut w, &mut h) } == 0;
        (ok && w > 0 && h > 0).then_some((w, h))
    }

    fn stats(&self) -> Option<MediaStats> {
        unsafe {
            let m = (self.api.libvlc_media_player_get_media)(self.player);
            if m.is_null() {
                return None;
            }
            let mut r = RawStats::default();
            let ok = (self.api.libvlc_media_get_stats)(m, &mut r) != 0; // true on success
            (self.api.libvlc_media_release)(m);
            ok.then(|| MediaStats {
                read_bytes: r.read_bytes as i64,
                input_bitrate_kbps: r.input_bitrate * 8000.0,
                demux_corrupted: r.demux_corrupted as i64,
                demux_discontinuity: r.demux_discontinuity as i64,
                decoded_video: r.decoded_video as i64,
                decoded_audio: r.decoded_audio as i64,
                displayed_pictures: r.displayed_pictures as i64,
                lost_pictures: r.lost_pictures as i64,
                played_abuffers: r.played_abuffers as i64,
                lost_abuffers: r.lost_abuffers as i64,
            })
        }
    }

    fn set_volume(&mut self, volume: i32) {
        unsafe { (self.api.libvlc_audio_set_volume)(self.player, volume.clamp(0, 200)) };
    }

    fn set_rate(&mut self, rate: f32) -> Result<(), String> {
        match unsafe { (self.api.libvlc_media_player_set_rate)(self.player, rate) } {
            0 => Ok(()),
            _ => Err("This media does not support the requested rate".into()),
        }
    }

    fn next_frame(&mut self) {
        unsafe { (self.api.libvlc_media_player_next_frame)(self.player) }
    }

    fn status(&self) -> PlaybackStatus {
        unsafe {
            let state = match (self.api.libvlc_media_player_get_state)(self.player) {
                1 => PlaybackState::Opening,
                2 => PlaybackState::Buffering,
                3 => PlaybackState::Playing,
                4 => PlaybackState::Paused,
                5 => PlaybackState::Stopped,
                6 => PlaybackState::Ended,
                7 => PlaybackState::Error,
                _ => PlaybackState::Idle,
            };
            PlaybackStatus {
                state,
                time_ms: (self.api.libvlc_media_player_get_time)(self.player).max(0),
                length_ms: (self.api.libvlc_media_player_get_length)(self.player).max(0),
                volume: (self.api.libvlc_audio_get_volume)(self.player),
                muted: (self.api.libvlc_audio_get_mute)(self.player) == 1,
                rate: (self.api.libvlc_media_player_get_rate)(self.player),
                seekable: (self.api.libvlc_media_player_is_seekable)(self.player) != 0,
                has_video: (self.api.libvlc_media_player_has_vout)(self.player) > 0,
                chapter: (self.api.libvlc_media_player_get_chapter)(self.player),
                chapter_count: (self.api.libvlc_media_player_get_chapter_count)(self.player),
                audio_delay_ms: (self.api.libvlc_audio_get_delay)(self.player) / 1000,
                subtitle_delay_ms: (self.api.libvlc_video_get_spu_delay)(self.player) / 1000,
            }
        }
    }

    fn tracks(&self) -> TrackSelection {
        unsafe {
            TrackSelection {
                audio: self.descriptions((self.api.libvlc_audio_get_track_description)(self.player)),
                subtitle: self.descriptions((self.api.libvlc_video_get_spu_description)(self.player)),
                current_audio: (self.api.libvlc_audio_get_track)(self.player),
                current_subtitle: (self.api.libvlc_video_get_spu)(self.player),
            }
        }
    }

    fn set_audio_track(&mut self, id: i32) -> Result<(), String> {
        match unsafe { (self.api.libvlc_audio_set_track)(self.player, id) } {
            0 => Ok(()),
            _ => Err("Audio track not available".into()),
        }
    }

    fn set_subtitle_track(&mut self, id: i32) -> Result<(), String> {
        match unsafe { (self.api.libvlc_video_set_spu)(self.player, id) } {
            0 => Ok(()),
            _ => Err("Subtitle track not available".into()),
        }
    }

    fn meta(&self) -> EngineMeta {
        unsafe {
            let m = (self.api.libvlc_media_player_get_media)(self.player);
            if m.is_null() {
                return EngineMeta::default();
            }
            let out = EngineMeta {
                title: self.meta_field(m, META_TITLE),
                artist: self.meta_field(m, META_ARTIST),
                album: self.meta_field(m, META_ALBUM),
                genre: self.meta_field(m, META_GENRE),
                date: self.meta_field(m, META_DATE),
                artwork_url: self.meta_field(m, META_ARTWORK_URL),
            };
            (self.api.libvlc_media_release)(m); // get_media retains
            out
        }
    }
}
