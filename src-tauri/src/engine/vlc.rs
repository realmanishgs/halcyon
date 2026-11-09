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

// libvlc_video_adjust_option_t
const ADJUST_ENABLE: c_uint = 0;
const ADJUST_CONTRAST: c_uint = 1;
const ADJUST_BRIGHTNESS: c_uint = 2;
const ADJUST_HUE: c_uint = 3;
const ADJUST_SATURATION: c_uint = 4;
const ADJUST_GAMMA: c_uint = 5;

// libvlc_event_e (libvlc_events.h, 3.0.x)
const EV_MEDIA_CHANGED: c_int = 0x100;
const EV_OPENING: c_int = 0x102;
const EV_PLAYING: c_int = 0x104;
const EV_PAUSED: c_int = 0x105;
const EV_STOPPED: c_int = 0x106;
const EV_END_REACHED: c_int = 0x109;
const EV_ERROR: c_int = 0x10A;
const EV_LENGTH_CHANGED: c_int = 0x111;
const EV_VOUT: c_int = 0x112;
const EV_ES_ADDED: c_int = 0x114;
const EV_ES_DELETED: c_int = 0x115;
const PLAYER_EVENTS: [c_int; 11] = [
    EV_MEDIA_CHANGED, EV_OPENING, EV_PLAYING, EV_PAUSED, EV_STOPPED, EV_END_REACHED, EV_ERROR, EV_LENGTH_CHANGED,
    EV_VOUT, EV_ES_ADDED, EV_ES_DELETED,
];

// libvlc_media_parse_flag_t / libvlc_media_parsed_status_t / discoverer category
const PARSE_NETWORK: c_int = 0x01;
const PARSED_DONE: c_int = 4;
const DISCOVERER_LAN: c_int = 1;

#[repr(C)]
struct RawEvent {
    kind: c_int,
    obj: *mut c_void,
}

#[repr(C)]
struct RawAudioDevice {
    next: *mut RawAudioDevice,
    device: *mut c_char,
    description: *mut c_char,
}

#[repr(C)]
struct RawDiscoverer {
    name: *mut c_char,
    longname: *mut c_char,
    category: c_int,
}

type Sink = EventSink;

unsafe extern "C" fn on_event(ev: *const RawEvent, data: *mut c_void) {
    // Runs on a libVLC thread: never call back into libVLC from here.
    let sink = &*(data as *const Sink);
    let e = match (*ev).kind {
        EV_MEDIA_CHANGED => EngineEvent::Media,
        EV_OPENING => EngineEvent::Opening,
        EV_PLAYING => EngineEvent::Playing,
        EV_PAUSED => EngineEvent::Paused,
        EV_STOPPED => EngineEvent::Stopped,
        EV_END_REACHED => EngineEvent::Ended,
        EV_ERROR => EngineEvent::Error,
        EV_LENGTH_CHANGED => EngineEvent::Length,
        EV_VOUT => EngineEvent::Vout,
        EV_ES_ADDED | EV_ES_DELETED => EngineEvent::Tracks,
        _ => return,
    };
    sink(e);
}

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
    // UNC "//server/share" → "file://server/share"; "C:/x" → "file:///C:/x"; "/home/x" → "file:///home/x".
    let mut out = String::from(if p.starts_with('/') { "file:" } else { "file:///" });
    if p.starts_with('/') && !p.starts_with("//") {
        out.push_str("//");
    }
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
    libvlc_media_player_event_manager: fn(*mut Player) -> *mut c_void;
    libvlc_media_add_option: fn(*mut Media, *const c_char);
    libvlc_event_attach: fn(*mut c_void, c_int, unsafe extern "C" fn(*const RawEvent, *mut c_void), *mut c_void) -> c_int;
    libvlc_video_set_adjust_int: fn(*mut Player, c_uint, c_int);
    libvlc_video_set_adjust_float: fn(*mut Player, c_uint, f32);
    libvlc_video_set_crop_geometry: fn(*mut Player, *const c_char);
    libvlc_audio_equalizer_get_preset_count: fn() -> c_uint;
    libvlc_audio_equalizer_get_preset_name: fn(c_uint) -> *const c_char;
    libvlc_audio_equalizer_get_band_count: fn() -> c_uint;
    libvlc_audio_equalizer_get_band_frequency: fn(c_uint) -> f32;
    libvlc_audio_equalizer_new: fn() -> *mut c_void;
    libvlc_audio_equalizer_new_from_preset: fn(c_uint) -> *mut c_void;
    libvlc_audio_equalizer_release: fn(*mut c_void);
    libvlc_audio_equalizer_set_preamp: fn(*mut c_void, f32) -> c_int;
    libvlc_audio_equalizer_get_preamp: fn(*mut c_void) -> f32;
    libvlc_audio_equalizer_set_amp_at_index: fn(*mut c_void, f32, c_uint) -> c_int;
    libvlc_audio_equalizer_get_amp_at_index: fn(*mut c_void, c_uint) -> f32;
    libvlc_media_player_set_equalizer: fn(*mut Player, *mut c_void) -> c_int;
    libvlc_audio_output_device_enum: fn(*mut Player) -> *mut RawAudioDevice;
    libvlc_audio_output_device_list_release: fn(*mut RawAudioDevice);
    libvlc_audio_output_device_set: fn(*mut Player, *const c_char, *const c_char);
    libvlc_media_parse_with_options: fn(*mut Media, c_int, c_int) -> c_int;
    libvlc_media_get_parsed_status: fn(*mut Media) -> c_int;
    libvlc_media_subitems: fn(*mut Media) -> *mut c_void;
    libvlc_media_list_lock: fn(*mut c_void);
    libvlc_media_list_unlock: fn(*mut c_void);
    libvlc_media_list_count: fn(*mut c_void) -> c_int;
    libvlc_media_list_item_at_index: fn(*mut c_void, c_int) -> *mut Media;
    libvlc_media_list_release: fn(*mut c_void);
    libvlc_media_get_mrl: fn(*mut Media) -> *mut c_char;
    libvlc_media_get_type: fn(*mut Media) -> c_int;
    libvlc_media_discoverer_list_get: fn(*mut Inst, c_int, *mut *mut *mut RawDiscoverer) -> usize;
    libvlc_media_discoverer_list_release: fn(*mut *mut RawDiscoverer, usize);
    libvlc_media_discoverer_new: fn(*mut Inst, *const c_char) -> *mut c_void;
    libvlc_media_discoverer_start: fn(*mut c_void) -> c_int;
    libvlc_media_discoverer_stop: fn(*mut c_void);
    libvlc_media_discoverer_release: fn(*mut c_void);
    libvlc_media_discoverer_media_list: fn(*mut c_void) -> *mut c_void;
}

pub struct VlcEngine {
    api: Api,
    inst: *mut Inst,
    player: *mut Player,
    location: PathBuf,
    /// Options passed by the host (e.g. dummy outputs in tests), kept across reconfiguration.
    base_args: Vec<String>,
    config_args: Vec<String>,
    media_options: Vec<String>,
    source: Option<String>,
    surface: Option<isize>,
    equalizer: Option<Equalizer>,
    adjust: Option<VideoAdjust>,
    device: String,
    /// Boxed so its address stays stable for libVLC's callback user data.
    sink: Option<Box<Sink>>,
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
            let base_args: Vec<String> = extra.iter().map(|s| s.to_string()).collect();
            let config_args = EngineConfig::default().args();
            let (inst, player) = create(&api, &base_args, &config_args)?;
            Ok(Self {
                api,
                inst,
                player,
                location: dir,
                base_args,
                config_args,
                media_options: EngineConfig::default().media_options(),
                source: None,
                surface: None,
                equalizer: None,
                adjust: None,
                device: String::new(),
                sink: None,
                _lib: lib,
            })
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

    fn attach_events(&self) {
        if let Some(b) = &self.sink {
            unsafe {
                let em = (self.api.libvlc_media_player_event_manager)(self.player);
                let data = &**b as *const Sink as *mut c_void;
                for ev in PLAYER_EVENTS {
                    (self.api.libvlc_event_attach)(em, ev, on_event, data);
                }
            }
        }
    }

    fn media_items(&self, list: *mut c_void) -> Vec<BrowseItem> {
        let mut out = Vec::new();
        if list.is_null() {
            return out;
        }
        unsafe {
            (self.api.libvlc_media_list_lock)(list);
            for i in 0..(self.api.libvlc_media_list_count)(list) {
                let m = (self.api.libvlc_media_list_item_at_index)(list, i);
                if m.is_null() {
                    continue;
                }
                let mrl_p = (self.api.libvlc_media_get_mrl)(m);
                let url = cstr(mrl_p).unwrap_or_default();
                if !mrl_p.is_null() {
                    (self.api.libvlc_free)(mrl_p as *mut c_void);
                }
                let name = self
                    .meta_field(m, META_TITLE)
                    .unwrap_or_else(|| url.rsplit('/').next().unwrap_or("").to_string());
                let kind = match (self.api.libvlc_media_get_type)(m) {
                    1 => "file",
                    2 => "directory",
                    3 => "disc",
                    4 => "stream",
                    5 => "playlist",
                    _ => "unknown",
                };
                (self.api.libvlc_media_release)(m);
                out.push(BrowseItem { name, url, kind: kind.into() });
            }
            (self.api.libvlc_media_list_unlock)(list);
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

unsafe fn create(api: &Api, base: &[String], config: &[String]) -> Result<(*mut Inst, *mut Player), String> {
    let fixed = ["--no-video-title-show", "--no-snapshot-preview", "--no-metadata-network-access"];
    let args: Vec<CString> = fixed
        .iter()
        .map(|s| s.to_string())
        .chain(config.iter().cloned())
        .chain(base.iter().cloned())
        .map(|s| CString::new(s).unwrap())
        .collect();
    let argv: Vec<*const c_char> = args.iter().map(|a| a.as_ptr()).collect();
    let inst = (api.libvlc_new)(argv.len() as c_int, argv.as_ptr());
    if inst.is_null() {
        return Err(format!("libvlc_new failed: {}", errmsg(api)));
    }
    let player = (api.libvlc_media_player_new)(inst);
    if player.is_null() {
        (api.libvlc_release)(inst);
        return Err("libvlc_media_player_new failed".into());
    }
    // Let the host UI handle input; the video surface is display-only.
    (api.libvlc_video_set_mouse_input)(player, 0);
    (api.libvlc_video_set_key_input)(player, 0);
    Ok((inst, player))
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
            for o in &self.media_options {
                if let Ok(c) = CString::new(o.as_str()) {
                    (self.api.libvlc_media_add_option)(media, c.as_ptr());
                }
            }
            (self.api.libvlc_media_player_set_media)(self.player, media);
            (self.api.libvlc_media_release)(media); // player holds its own reference
            self.source = Some(source.to_string());
            self.surface = surface;
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

    fn set_event_sink(&mut self, sink: EventSink) {
        self.sink = Some(Box::new(sink));
        self.attach_events();
    }

    fn configure(&mut self, cfg: &EngineConfig) -> Result<bool, String> {
        let args = cfg.args();
        let media_options = cfg.media_options();
        if args == self.config_args && media_options == self.media_options {
            return Ok(false);
        }
        self.media_options = media_options;
        let st = self.status();
        let active = matches!(
            st.state,
            PlaybackState::Playing | PlaybackState::Paused | PlaybackState::Opening | PlaybackState::Buffering
        );
        // Create first: if the new options are rejected the current instance keeps playing.
        let (inst, player) = unsafe { create(&self.api, &self.base_args, &args)? };
        unsafe {
            (self.api.libvlc_media_player_stop)(self.player);
            (self.api.libvlc_media_player_release)(self.player);
            (self.api.libvlc_release)(self.inst);
        }
        self.inst = inst;
        self.player = player;
        self.config_args = args;
        self.attach_events();
        if !self.device.is_empty() {
            let d = self.device.clone();
            self.set_audio_device(&d);
        }
        let eq = self.equalizer.clone();
        self.set_equalizer(eq.as_ref())?;
        unsafe {
            (self.api.libvlc_audio_set_volume)(self.player, st.volume.max(0));
            (self.api.libvlc_audio_set_mute)(self.player, st.muted as c_int);
        }
        if active {
            if let Some(src) = self.source.clone() {
                self.open(&src, self.surface)?;
                let _ = self.set_rate(st.rate);
                let t = std::time::Instant::now();
                while self.raw_state() != STATE_PLAYING && t.elapsed().as_millis() < 3000 {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                if st.time_ms > 0 {
                    self.seek_ms(st.time_ms);
                }
                // The audio output is rebuilt for the new input: restore its level.
                self.set_volume(st.volume.max(0));
                self.set_muted(st.muted);
                if st.state == PlaybackState::Paused {
                    self.set_paused(true);
                }
            }
        } else if let Some(src) = self.source.clone() {
            // Keep the media loaded so play() works without reopening.
            let c = CString::new(if is_url(&src) { src.clone() } else { engine_path(&src) }).map_err(|_| "Path contains NUL")?;
            unsafe {
                let m = if is_url(&src) {
                    (self.api.libvlc_media_new_location)(self.inst, c.as_ptr())
                } else {
                    (self.api.libvlc_media_new_path)(self.inst, c.as_ptr())
                };
                if !m.is_null() {
                    for o in &self.media_options {
                        if let Ok(c) = CString::new(o.as_str()) {
                            (self.api.libvlc_media_add_option)(m, c.as_ptr());
                        }
                    }
                    (self.api.libvlc_media_player_set_media)(self.player, m);
                    (self.api.libvlc_media_release)(m);
                    #[cfg(windows)]
                    if let Some(h) = self.surface {
                        (self.api.libvlc_media_player_set_hwnd)(self.player, h as *mut c_void);
                    }
                }
            }
        }
        let adj = self.adjust.clone();
        self.set_video_adjust(adj.as_ref());
        Ok(true)
    }

    fn equalizer_info(&self) -> EqualizerInfo {
        unsafe {
            let n = (self.api.libvlc_audio_equalizer_get_band_count)();
            let frequencies = (0..n).map(|i| (self.api.libvlc_audio_equalizer_get_band_frequency)(i)).collect();
            let presets = (0..(self.api.libvlc_audio_equalizer_get_preset_count)())
                .filter_map(|i| {
                    let name = cstr((self.api.libvlc_audio_equalizer_get_preset_name)(i))?;
                    let e = (self.api.libvlc_audio_equalizer_new_from_preset)(i);
                    if e.is_null() {
                        return None;
                    }
                    let eq = Equalizer {
                        preamp: (self.api.libvlc_audio_equalizer_get_preamp)(e),
                        bands: (0..n).map(|b| (self.api.libvlc_audio_equalizer_get_amp_at_index)(e, b)).collect(),
                    };
                    (self.api.libvlc_audio_equalizer_release)(e);
                    Some((name, eq))
                })
                .collect();
            EqualizerInfo { frequencies, presets }
        }
    }

    fn set_equalizer(&mut self, eq: Option<&Equalizer>) -> Result<(), String> {
        unsafe {
            let Some(eq) = eq else {
                self.equalizer = None;
                (self.api.libvlc_media_player_set_equalizer)(self.player, std::ptr::null_mut());
                return Ok(());
            };
            let e = (self.api.libvlc_audio_equalizer_new)();
            if e.is_null() {
                return Err("Equalizer unavailable".into());
            }
            (self.api.libvlc_audio_equalizer_set_preamp)(e, eq.preamp.clamp(-20.0, 20.0));
            for (i, g) in eq.bands.iter().enumerate() {
                (self.api.libvlc_audio_equalizer_set_amp_at_index)(e, g.clamp(-20.0, 20.0), i as c_uint);
            }
            let r = (self.api.libvlc_media_player_set_equalizer)(self.player, e);
            (self.api.libvlc_audio_equalizer_release)(e); // the player copies the settings
            self.equalizer = Some(eq.clone());
            if r == 0 {
                Ok(())
            } else {
                Err("Could not apply equalizer".into())
            }
        }
    }

    fn set_video_adjust(&mut self, adj: Option<&VideoAdjust>) {
        self.adjust = adj.cloned();
        unsafe {
            match adj {
                None => (self.api.libvlc_video_set_adjust_int)(self.player, ADJUST_ENABLE, 0),
                Some(a) => {
                    (self.api.libvlc_video_set_adjust_int)(self.player, ADJUST_ENABLE, 1);
                    (self.api.libvlc_video_set_adjust_float)(self.player, ADJUST_CONTRAST, a.contrast.clamp(0.0, 2.0));
                    (self.api.libvlc_video_set_adjust_float)(self.player, ADJUST_BRIGHTNESS, a.brightness.clamp(0.0, 2.0));
                    (self.api.libvlc_video_set_adjust_float)(self.player, ADJUST_HUE, a.hue.clamp(-180.0, 180.0));
                    (self.api.libvlc_video_set_adjust_float)(self.player, ADJUST_SATURATION, a.saturation.clamp(0.0, 3.0));
                    (self.api.libvlc_video_set_adjust_float)(self.player, ADJUST_GAMMA, a.gamma.clamp(0.01, 10.0));
                }
            }
        }
    }

    fn set_crop(&mut self, geometry: Option<&str>) {
        let c = geometry.and_then(|g| CString::new(g).ok());
        let p = c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
        unsafe { (self.api.libvlc_video_set_crop_geometry)(self.player, p) }
    }

    fn audio_devices(&self) -> Vec<AudioDevice> {
        let mut out = Vec::new();
        unsafe {
            let list = (self.api.libvlc_audio_output_device_enum)(self.player);
            let mut p = list;
            while !p.is_null() {
                out.push(AudioDevice { id: cstr((*p).device).unwrap_or_default(), name: cstr((*p).description).unwrap_or_default() });
                p = (*p).next;
            }
            if !list.is_null() {
                (self.api.libvlc_audio_output_device_list_release)(list);
            }
        }
        out
    }

    fn set_audio_device(&mut self, id: &str) {
        self.device = id.to_string();
        if let Ok(c) = CString::new(id) {
            unsafe { (self.api.libvlc_audio_output_device_set)(self.player, std::ptr::null(), c.as_ptr()) }
        }
    }

    fn browse(&mut self, url: &str, timeout_ms: u32) -> Result<Vec<BrowseItem>, String> {
        let c = CString::new(url).map_err(|_| "URL contains NUL")?;
        unsafe {
            let m = (self.api.libvlc_media_new_location)(self.inst, c.as_ptr());
            if m.is_null() {
                return Err(format!("Invalid location: {}", errmsg(&self.api)));
            }
            (self.api.libvlc_media_parse_with_options)(m, PARSE_NETWORK, timeout_ms as c_int);
            let t = std::time::Instant::now();
            let mut st = 0;
            while t.elapsed().as_millis() < timeout_ms as u128 + 500 {
                st = (self.api.libvlc_media_get_parsed_status)(m);
                if st != 0 {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            let list = (self.api.libvlc_media_subitems)(m);
            let items = self.media_items(list);
            if !list.is_null() {
                (self.api.libvlc_media_list_release)(list);
            }
            (self.api.libvlc_media_release)(m);
            match st {
                PARSED_DONE => Ok(items),
                0 | 3 => Err("Timed out listing this location".into()),
                _ if items.is_empty() => Err("Could not list this location".into()),
                _ => Ok(items),
            }
        }
    }

    fn discovery_services(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        unsafe {
            let mut list: *mut *mut RawDiscoverer = std::ptr::null_mut();
            let n = (self.api.libvlc_media_discoverer_list_get)(self.inst, DISCOVERER_LAN, &mut list);
            for i in 0..n {
                let d = *list.add(i);
                out.push((cstr((*d).name).unwrap_or_default(), cstr((*d).longname).unwrap_or_default()));
            }
            if n > 0 {
                (self.api.libvlc_media_discoverer_list_release)(list, n);
            }
        }
        out
    }

    fn discover(&mut self, service: &str, wait_ms: u32) -> Result<Vec<BrowseItem>, String> {
        let c = CString::new(service).map_err(|_| "Invalid service")?;
        unsafe {
            let md = (self.api.libvlc_media_discoverer_new)(self.inst, c.as_ptr());
            if md.is_null() {
                return Err(format!("Discovery service {service} unavailable"));
            }
            if (self.api.libvlc_media_discoverer_start)(md) != 0 {
                (self.api.libvlc_media_discoverer_release)(md);
                return Err(format!("Could not start {service}"));
            }
            std::thread::sleep(std::time::Duration::from_millis(wait_ms as u64));
            let list = (self.api.libvlc_media_discoverer_media_list)(md);
            let items = self.media_items(list);
            if !list.is_null() {
                (self.api.libvlc_media_list_release)(list);
            }
            (self.api.libvlc_media_discoverer_stop)(md);
            (self.api.libvlc_media_discoverer_release)(md);
            Ok(items)
        }
    }
}
