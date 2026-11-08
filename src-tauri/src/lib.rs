mod engine;
mod probe;
mod sources;
mod surface;

use engine::{EngineInfo, EngineMeta, MediaEngine, MediaStats, PlaybackStatus, TrackSelection};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

struct AppState {
    engine: Mutex<Option<Box<dyn MediaEngine>>>,
    engine_error: Option<String>,
    surface: Mutex<Option<isize>>,
}

type Res<T> = Result<T, String>;

fn with_engine<T>(s: &AppState, f: impl FnOnce(&mut dyn MediaEngine) -> Res<T>) -> Res<T> {
    let mut g = s.engine.lock().map_err(|_| "engine lock poisoned")?;
    match g.as_mut() {
        Some(e) => f(e.as_mut()),
        None => Err(s.engine_error.clone().unwrap_or_else(|| "No media engine".into())),
    }
}

#[derive(Serialize)]
struct OpenResult {
    path: String,
    probe: Option<probe::ProbeResult>,
    probe_error: Option<String>,
}

#[derive(Serialize)]
struct Diagnostics {
    engine: Option<EngineInfo>,
    engine_error: Option<String>,
    ffprobe: Option<String>,
}

#[tauri::command]
fn diagnostics(s: State<AppState>) -> Diagnostics {
    let engine = s.engine.lock().ok().and_then(|g| g.as_ref().map(|e| e.info()));
    Diagnostics {
        engine,
        engine_error: s.engine_error.clone(),
        ffprobe: probe::ffprobe_path().map(|p| p.display().to_string()),
    }
}

/// Files/folders passed on the command line ("Open with…" / file association).
#[tauri::command]
fn startup_files() -> Vec<String> {
    let args: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    sources::expand(&args)
}

/// Folders → media files, playlists → entries, URLs pass through.
#[tauri::command]
async fn expand_sources(items: Vec<String>) -> Res<Vec<String>> {
    tauri::async_runtime::spawn_blocking(move || sources::expand(&items)).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn is_subtitle_file(path: String) -> bool {
    sources::is_subtitle(&path)
}

#[tauri::command]
async fn open_media(path: String, s: State<'_, AppState>) -> Res<OpenResult> {
    let local = !engine::is_url(&path);
    let p = path.clone();
    let probed = if local {
        Some(tauri::async_runtime::spawn_blocking(move || probe::probe(&p)).await.map_err(|e| e.to_string())?)
    } else {
        None
    };
    let surface = *s.surface.lock().map_err(|_| "surface lock poisoned")?;
    // Probe failure is reported but does not block: libVLC may still play it.
    with_engine(&s, |e| e.open(&path, surface))?;
    let (probe, probe_error) = match probed {
        Some(Ok(r)) => (Some(r), None),
        Some(Err(e)) => (None, Some(e)),
        None => (None, None),
    };
    Ok(OpenResult { path, probe, probe_error })
}

#[tauri::command]
fn play(s: State<AppState>) -> Res<()> { with_engine(&s, |e| e.play()) }
#[tauri::command]
fn set_paused(paused: bool, s: State<AppState>) -> Res<()> { with_engine(&s, |e| Ok(e.set_paused(paused))) }
#[tauri::command]
fn stop(s: State<AppState>) -> Res<()> { with_engine(&s, |e| Ok(e.stop())) }
#[tauri::command]
fn seek(ms: i64, s: State<AppState>) -> Res<()> { with_engine(&s, |e| Ok(e.seek_ms(ms))) }
#[tauri::command]
fn set_volume(volume: i32, s: State<AppState>) -> Res<()> { with_engine(&s, |e| Ok(e.set_volume(volume))) }
#[tauri::command]
fn set_muted(muted: bool, s: State<AppState>) -> Res<()> { with_engine(&s, |e| Ok(e.set_muted(muted))) }
#[tauri::command]
fn set_rate(rate: f32, s: State<AppState>) -> Res<()> { with_engine(&s, |e| e.set_rate(rate)) }
#[tauri::command]
fn next_frame(s: State<AppState>) -> Res<()> { with_engine(&s, |e| Ok(e.next_frame())) }
#[tauri::command]
fn status(s: State<AppState>) -> Res<PlaybackStatus> { with_engine(&s, |e| Ok(e.status())) }
#[tauri::command]
fn tracks(s: State<AppState>) -> Res<TrackSelection> { with_engine(&s, |e| Ok(e.tracks())) }
#[tauri::command]
fn set_audio_track(id: i32, s: State<AppState>) -> Res<()> { with_engine(&s, |e| e.set_audio_track(id)) }
#[tauri::command]
fn set_subtitle_track(id: i32, s: State<AppState>) -> Res<()> { with_engine(&s, |e| e.set_subtitle_track(id)) }
#[tauri::command]
fn add_subtitle_file(path: String, s: State<AppState>) -> Res<()> { with_engine(&s, |e| e.add_subtitle_file(&path)) }
#[tauri::command]
fn set_audio_delay(ms: i64, s: State<AppState>) -> Res<()> { with_engine(&s, |e| e.set_audio_delay_ms(ms)) }
#[tauri::command]
fn set_subtitle_delay(ms: i64, s: State<AppState>) -> Res<()> { with_engine(&s, |e| e.set_subtitle_delay_ms(ms)) }
#[tauri::command]
fn set_aspect_ratio(aspect: Option<String>, s: State<AppState>) -> Res<()> {
    with_engine(&s, |e| Ok(e.set_aspect_ratio(aspect.as_deref())))
}
#[tauri::command]
fn set_deinterlace(mode: Option<String>, s: State<AppState>) -> Res<()> {
    with_engine(&s, |e| Ok(e.set_deinterlace(mode.as_deref())))
}
#[tauri::command]
fn set_chapter(chapter: i32, s: State<AppState>) -> Res<()> { with_engine(&s, |e| Ok(e.set_chapter(chapter))) }
#[tauri::command]
fn video_size(s: State<AppState>) -> Res<Option<(u32, u32)>> { with_engine(&s, |e| Ok(e.video_size())) }
#[tauri::command]
fn stats(s: State<AppState>) -> Res<Option<MediaStats>> { with_engine(&s, |e| Ok(e.stats())) }
#[tauri::command]
fn meta(s: State<AppState>) -> Res<EngineMeta> { with_engine(&s, |e| Ok(e.meta())) }

/// Saves the current frame as PNG under Pictures/Halcyon and returns its path.
#[tauri::command]
fn snapshot(stem: String, app: tauri::AppHandle, s: State<AppState>) -> Res<String> {
    let dir = app.path().picture_dir().map_err(|e| e.to_string())?.join("Halcyon");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let safe: String = stem.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).take(60).collect();
    let path = dir.join(format!("{safe}-{stamp}.png"));
    let p = path.to_string_lossy().into_owned();
    with_engine(&s, |e| e.snapshot(&p))?;
    Ok(p)
}

/// Rectangle in physical pixels relative to the webview (= client area) origin.
#[tauri::command]
fn set_video_rect(x: i32, y: i32, w: i32, h: i32, visible: bool, s: State<AppState>) -> Res<()> {
    if let Some(hwnd) = *s.surface.lock().map_err(|_| "surface lock poisoned")? {
        surface::place(hwnd, x, y, w, h, visible);
    }
    Ok(())
}

#[tauri::command]
fn set_cursor_hidden(hidden: bool) {
    surface::set_cursor_hidden(hidden);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (engine, engine_error) = match engine::vlc::VlcEngine::load() {
        Ok(e) => (Some(Box::new(e) as Box<dyn MediaEngine>), None),
        Err(e) => (None, Some(e)),
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { engine: Mutex::new(engine), engine_error, surface: Mutex::new(None) })
        .setup(|app| {
            let win = app.get_webview_window("main").expect("main window");
            #[cfg(windows)]
            {
                let parent = win.hwnd()?.0 as isize;
                let handle = app.handle().clone();
                let sink = move |ev: surface::SurfaceEvent| {
                    let _ = handle.emit("surface", ev);
                };
                match surface::create(parent, sink) {
                    Ok(h) => *app.state::<AppState>().surface.lock().unwrap() = Some(h),
                    Err(e) => eprintln!("{e}"),
                }
            }
            let _ = win;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            diagnostics, startup_files, expand_sources, is_subtitle_file, open_media, play, set_paused, stop,
            seek, set_volume, set_muted, set_rate, next_frame, status, tracks, set_audio_track,
            set_subtitle_track, add_subtitle_file, set_audio_delay, set_subtitle_delay, set_aspect_ratio,
            set_deinterlace, set_chapter, video_size, stats, meta, snapshot, set_video_rect, set_cursor_hidden
        ])
        .run(tauri::generate_context!())
        .expect("error while running Halcyon");
}
