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

/// Directories searched for libvlc.dll, in priority order.
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
    for var in ["ProgramFiles", "ProgramW6432"] {
        if let Ok(pf) = std::env::var(var) {
            v.push(Path::new(&pf).join("VideoLAN").join("VLC"));
        }
    }
    v
}

impl VlcEngine {
    pub fn load() -> Result<Self, String> {
        Self::load_with_args(&[])
    }

    /// `extra` is appended to the libVLC command line (used by headless tests).
    pub fn load_with_args(extra: &[&str]) -> Result<Self, String> {
        let dir = candidate_dirs()
            .into_iter()
            .find(|d| d.join("libvlc.dll").is_file())
            .ok_or("libVLC not found. Install VLC 3.x (64-bit) or set HALCYON_LIBVLC_DIR.")?;
        unsafe {
            // Altered search path makes Windows resolve libvlccore.dll next to libvlc.dll.
            let lib: Library = libloading::os::windows::Library::load_with_flags(
                dir.join("libvlc.dll"),
                libloading::os::windows::LOAD_WITH_ALTERED_SEARCH_PATH,
            )
            .map_err(|e| format!("Failed to load libvlc.dll: {e}"))?
            .into();
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

    fn open(&mut self, path: &str, surface: Option<isize>) -> Result<(), String> {
        let c = CString::new(plain_path(path)).map_err(|_| "Path contains NUL")?;
        unsafe {
            (self.api.libvlc_media_player_stop)(self.player);
            let media = (self.api.libvlc_media_new_path)(self.inst, c.as_ptr());
            if media.is_null() {
                return Err(format!("Could not open media: {}", errmsg(&self.api)));
            }
            (self.api.libvlc_media_player_set_media)(self.player, media);
            (self.api.libvlc_media_release)(media); // player holds its own reference
            if let Some(h) = surface {
                (self.api.libvlc_media_player_set_hwnd)(self.player, h as *mut c_void);
            }
        }
        self.play()
    }

    fn play(&mut self) -> Result<(), String> {
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
        unsafe { (self.api.libvlc_media_player_set_time)(self.player, ms.max(0)) }
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
                rate: (self.api.libvlc_media_player_get_rate)(self.player),
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
