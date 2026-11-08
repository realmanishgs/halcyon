mod engine;
mod probe;
mod surface;

use engine::{EngineInfo, EngineMeta, MediaEngine, PlaybackStatus, TrackSelection};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Manager, State};

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

/// File passed on the command line ("Open with…" / file association).
#[tauri::command]
fn startup_file() -> Option<String> {
    std::env::args().skip(1).find(|a| std::path::Path::new(a).is_file())
}

#[tauri::command]
async fn open_media(path: String, s: State<'_, AppState>) -> Res<OpenResult> {
    let p = path.clone();
    let probed = tauri::async_runtime::spawn_blocking(move || probe::probe(&p))
        .await
        .map_err(|e| e.to_string())?;
    let surface = *s.surface.lock().map_err(|_| "surface lock poisoned")?;
    // Probe failure is reported but does not block: libVLC may still play it.
    with_engine(&s, |e| e.open(&path, surface))?;
    let (probe, probe_error) = match probed {
        Ok(r) => (Some(r), None),
        Err(e) => (None, Some(e)),
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
fn meta(s: State<AppState>) -> Res<EngineMeta> { with_engine(&s, |e| Ok(e.meta())) }

/// Rectangle in physical pixels relative to the webview (= client area) origin.
#[tauri::command]
fn set_video_rect(x: i32, y: i32, w: i32, h: i32, visible: bool, s: State<AppState>) -> Res<()> {
    if let Some(hwnd) = *s.surface.lock().map_err(|_| "surface lock poisoned")? {
        surface::place(hwnd, x, y, w, h, visible);
    }
    Ok(())
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
                match surface::create(parent) {
                    Ok(h) => *app.state::<AppState>().surface.lock().unwrap() = Some(h),
                    Err(e) => eprintln!("{e}"),
                }
            }
            let _ = win;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            diagnostics, startup_file, open_media, play, set_paused, stop, seek, set_volume, set_rate,
            next_frame, status, tracks, set_audio_track, set_subtitle_track, meta, set_video_rect
        ])
        .run(tauri::generate_context!())
        .expect("error while running Halcyon");
}
