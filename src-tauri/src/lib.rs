mod convert;
mod db;
mod engine;
#[cfg(test)]
mod fuzz;
mod lab;
mod library;
mod probe;
mod sources;
mod surface;

use engine::{
    AudioDevice, EngineConfig, EngineInfo, EngineMeta, Equalizer, EqualizerInfo, MediaEngine, MediaStats, PlaybackStatus,
    TrackSelection, VideoAdjust,
};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

struct AppState {
    engine: Mutex<Option<Box<dyn MediaEngine>>>,
    engine_error: Option<String>,
    surface: Mutex<Option<isize>>,
    db: std::sync::OnceLock<db::Db>,
    browser: Mutex<Option<engine::vlc::VlcEngine>>,
    jobs: Mutex<std::collections::HashMap<u64, std::sync::Arc<std::sync::atomic::AtomicBool>>>,
    job_seq: std::sync::atomic::AtomicU64,
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

// ---------------------------------------------------------------- phase 2–7 commands

fn db(s: &AppState) -> Res<&db::Db> {
    s.db.get().ok_or_else(|| "Storage not available".to_string())
}

fn next_job(s: &AppState) -> u64 {
    s.job_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// The UI's first call: with HALCYON_PERF set, prints the cold-start time (process start → UI script running).
#[tauri::command]
fn get_settings(s: State<AppState>) -> Res<serde_json::Map<String, serde_json::Value>> {
    if std::env::var_os("HALCYON_PERF").is_some() {
        if let Some(t) = STARTED.get() {
            println!("perf: ui_ready_ms={}", t.elapsed().as_millis());
        }
    }
    db(&s)?.settings()
}
#[tauri::command]
fn set_setting(key: String, value: serde_json::Value, s: State<AppState>) -> Res<()> {
    db(&s)?.set_setting(&key, &value)
}

#[tauri::command]
async fn configure_engine(cfg: EngineConfig, s: State<'_, AppState>) -> Res<bool> {
    let changed = with_engine(&s, |e| e.configure(&cfg))?;
    db(&s)?.set_setting("engine", &serde_json::to_value(&cfg).unwrap())?;
    Ok(changed)
}
#[tauri::command]
fn engine_config(s: State<AppState>) -> EngineConfig {
    db(&s).ok().and_then(|d| d.setting("engine")).unwrap_or_default()
}
#[tauri::command]
fn equalizer_info(s: State<AppState>) -> Res<EqualizerInfo> { with_engine(&s, |e| Ok(e.equalizer_info())) }
#[tauri::command]
fn set_equalizer(eq: Option<Equalizer>, s: State<AppState>) -> Res<()> { with_engine(&s, |e| e.set_equalizer(eq.as_ref())) }
#[tauri::command]
fn set_video_adjust(adj: Option<VideoAdjust>, s: State<AppState>) -> Res<()> { with_engine(&s, |e| Ok(e.set_video_adjust(adj.as_ref()))) }
#[tauri::command]
fn set_crop(geometry: Option<String>, s: State<AppState>) -> Res<()> { with_engine(&s, |e| Ok(e.set_crop(geometry.as_deref()))) }
#[tauri::command]
fn audio_devices(s: State<AppState>) -> Res<Vec<AudioDevice>> { with_engine(&s, |e| Ok(e.audio_devices())) }
#[tauri::command]
fn set_audio_device(id: String, s: State<AppState>) -> Res<()> { with_engine(&s, |e| Ok(e.set_audio_device(&id))) }

/// Network browsing runs on its own libVLC instance so a slow share never blocks playback.
fn with_browser<T>(s: &AppState, f: impl FnOnce(&mut engine::vlc::VlcEngine) -> Res<T>) -> Res<T> {
    let mut g = s.browser.lock().map_err(|_| "browser lock poisoned")?;
    if g.is_none() {
        *g = Some(engine::vlc::VlcEngine::load_with_args(&["--aout=adummy", "--vout=vdummy"])?);
    }
    f(g.as_mut().unwrap())
}
#[tauri::command]
async fn browse(url: String, app: tauri::AppHandle) -> Res<Vec<engine::BrowseItem>> {
    tauri::async_runtime::spawn_blocking(move || with_browser(&app.state::<AppState>(), |b| b.browse(&url, 8000)))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn discovery_services(app: tauri::AppHandle) -> Res<Vec<(String, String)>> {
    tauri::async_runtime::spawn_blocking(move || with_browser(&app.state::<AppState>(), |b| Ok(b.discovery_services())))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn discover(service: String, app: tauri::AppHandle) -> Res<Vec<engine::BrowseItem>> {
    tauri::async_runtime::spawn_blocking(move || with_browser(&app.state::<AppState>(), |b| b.discover(&service, 4000)))
        .await
        .map_err(|e| e.to_string())?
}

/// Plugin modules shipped with the engine, grouped by category folder.
#[tauri::command]
fn engine_plugins(s: State<AppState>) -> Res<Vec<(String, Vec<String>)>> {
    let loc = with_engine(&s, |e| Ok(e.info().location))?;
    let dir = std::path::Path::new(&loc).join("plugins");
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else { return Ok(out) };
    let mut cats: Vec<_> = rd.flatten().filter(|e| e.path().is_dir()).collect();
    cats.sort_by_key(|e| e.file_name());
    for c in cats {
        let mut names: Vec<String> = std::fs::read_dir(c.path())
            .map(|r| {
                r.flatten()
                    .filter_map(|f| {
                        let n = f.file_name().to_string_lossy().into_owned();
                        let stem = n.strip_prefix("lib").unwrap_or(&n);
                        let stem = stem.split("_plugin").next()?;
                        (n.contains("_plugin")).then(|| stem.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        out.push((c.file_name().to_string_lossy().into_owned(), names));
    }
    Ok(out)
}

// ---------- library ----------
#[tauri::command]
fn library_roots(s: State<AppState>) -> Res<Vec<String>> { db(&s)?.roots() }
#[tauri::command]
fn library_add_root(path: String, s: State<AppState>) -> Res<()> { db(&s)?.add_root(&engine::plain_path(&path)) }
#[tauri::command]
fn library_remove_root(path: String, s: State<AppState>) -> Res<()> { db(&s)?.remove_root(&path) }
#[tauri::command]
async fn library_scan(app: tauri::AppHandle) -> Res<library::ScanProgress> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let d = db(&st)?;
        library::scan(d, |p| {
            let _ = app.emit("library-progress", p);
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn library_list(kind: Option<String>, search: Option<String>, favorites: bool, sort: Option<String>, s: State<AppState>) -> Res<Vec<db::MediaRow>> {
    db(&s)?.list_media(kind.as_deref(), search.as_deref(), favorites, sort.as_deref())
}
#[tauri::command]
fn library_albums(s: State<AppState>) -> Res<Vec<db::Album>> { db(&s)?.albums() }
#[tauri::command]
fn library_album_tracks(album: String, key: String, s: State<AppState>) -> Res<Vec<db::MediaRow>> { db(&s)?.album_tracks(&album, &key) }
#[tauri::command]
fn library_artists(s: State<AppState>) -> Res<Vec<db::Artist>> { db(&s)?.artists() }
#[tauri::command]
fn library_artist_tracks(artist: String, s: State<AppState>) -> Res<Vec<db::MediaRow>> { db(&s)?.artist_tracks(&artist) }
#[tauri::command]
fn media_row(path: String, s: State<AppState>) -> Res<Option<db::MediaRow>> { Ok(db(&s)?.get_media(&path)) }
#[tauri::command]
async fn set_favorite(path: String, favorite: bool, app: tauri::AppHandle) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let d = db(&st)?;
        if favorite && !engine::is_url(&path) && d.get_media(&path).map(|m| m.kind == "unknown").unwrap_or(true) {
            let _ = library::index_file(d, std::path::Path::new(&path));
        }
        d.set_favorite(&path, favorite)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn data_url(path: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(path).ok().filter(|b| b.len() <= 4 * 1024 * 1024)?;
    let mime = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len() * 4 / 3 + 32);
    s.push_str(&format!("data:{mime};base64,"));
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    Some(s)
}

/// Cover art as a data: URL (no filesystem scope is granted to the webview).
#[tauri::command]
async fn cover(path: String, app: tauri::AppHandle) -> Res<Option<String>> {
    let cache = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("covers");
    tauri::async_runtime::spawn_blocking(move || library::cover(&path, &cache).and_then(|p| data_url(&p)))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn lyrics(path: String, app: tauri::AppHandle) -> Res<Option<library::Lyrics>> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let embedded = match db(&st).ok().and_then(|d| d.get_media(&path)) {
            Some(m) if m.kind != "unknown" => m.lyrics,
            _ => probe::probe(&path).ok().map(|p| library::row_from_probe(&path, &p, 0, 0)).and_then(|r| r.lyrics),
        };
        library::lyrics(&path, embedded)
    })
    .await
    .map_err(|e| e.to_string())
}

// ---------- playlists ----------
#[tauri::command]
fn playlists(s: State<AppState>) -> Res<Vec<db::Playlist>> { db(&s)?.playlists() }
#[tauri::command]
fn playlist_create(name: String, rules: Option<db::SmartRules>, items: Vec<String>, s: State<AppState>) -> Res<i64> {
    db(&s)?.create_playlist(&name, rules.as_ref(), &items)
}
#[tauri::command]
fn playlist_update(id: i64, name: String, rules: Option<db::SmartRules>, s: State<AppState>) -> Res<()> {
    db(&s)?.update_playlist(id, &name, rules.as_ref())
}
#[tauri::command]
fn playlist_delete(id: i64, s: State<AppState>) -> Res<()> { db(&s)?.delete_playlist(id) }
#[tauri::command]
fn playlist_items(id: i64, s: State<AppState>) -> Res<Vec<String>> { db(&s)?.playlist_items(id) }
#[tauri::command]
fn playlist_set_items(id: i64, items: Vec<String>, s: State<AppState>) -> Res<()> { db(&s)?.set_playlist_items(id, &items) }
#[tauri::command]
fn playlist_add(id: i64, items: Vec<String>, s: State<AppState>) -> Res<()> { db(&s)?.add_to_playlist(id, &items) }
#[tauri::command]
fn playlist_export(name: String, items: Vec<String>, target: String) -> Res<()> {
    library::export_playlist(std::path::Path::new(&target), &name, &items)
}
#[tauri::command]
fn playlist_import(path: String, s: State<AppState>) -> Res<i64> {
    let p = std::path::Path::new(&path);
    let items = sources::read_playlist(p);
    let name = p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Imported".into());
    db(&s)?.create_playlist(&name, None, &items)
}

// ---------- history ----------
#[tauri::command]
fn record_play(path: String, listened_ms: i64, s: State<AppState>) -> Res<()> { db(&s)?.record_play(&path, listened_ms) }
#[tauri::command]
fn listening_stats(s: State<AppState>) -> Res<db::ListeningStats> { db(&s)?.stats() }
#[tauri::command]
fn clear_history(s: State<AppState>) -> Res<()> { db(&s)?.clear_history() }
#[tauri::command]
fn resume_get(path: String, s: State<AppState>) -> Res<Option<i64>> { Ok(db(&s)?.resume_get(&path)) }
#[tauri::command]
fn resume_set(path: String, ms: Option<i64>, s: State<AppState>) -> Res<()> { db(&s)?.resume_set(&path, ms) }

// ---------- conversion ----------
#[derive(Clone, Serialize)]
struct JobEvent {
    id: u64,
    progress: f32,
    done: bool,
    error: Option<String>,
    output: Option<String>,
}

#[tauri::command]
fn convert_presets() -> Vec<convert::Preset> {
    convert::PRESETS.to_vec()
}
#[tauri::command]
fn convert_start(input: String, output: String, preset: String, options: convert::Options, app: tauri::AppHandle, s: State<AppState>) -> u64 {
    let id = next_job(&s);
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    s.jobs.lock().unwrap().insert(id, cancel.clone());
    std::thread::spawn(move || {
        let r = convert::run(&input, &output, &preset, &options, &cancel, |p| {
            let _ = app.emit("convert-progress", JobEvent { id, progress: p, done: false, error: None, output: None });
        });
        let _ = app.emit("convert-progress", JobEvent { id, progress: 1.0, done: true, error: r.err(), output: Some(output) });
        app.state::<AppState>().jobs.lock().unwrap().remove(&id);
    });
    id
}
#[tauri::command]
fn cancel_job(id: u64, s: State<AppState>) {
    if let Some(c) = s.jobs.lock().unwrap().get(&id) {
        c.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

// ---------- Immersive Audio Lab ----------
#[derive(Clone, Serialize)]
struct LabEvent {
    id: u64,
    progress: f32,
    result: Option<lab::Analysis>,
    error: Option<String>,
}

#[tauri::command]
fn lab_analyze(path: String, stream: Option<usize>, app: tauri::AppHandle, s: State<AppState>) -> u64 {
    let id = next_job(&s);
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    s.jobs.lock().unwrap().insert(id, cancel.clone());
    std::thread::spawn(move || {
        let r = lab::analyze(&path, stream, &cancel, |p| {
            let _ = app.emit("lab-progress", LabEvent { id, progress: p, result: None, error: None });
        });
        let ev = match r {
            Ok(a) => LabEvent { id, progress: 1.0, result: Some(a), error: None },
            Err(e) => LabEvent { id, progress: 1.0, result: None, error: Some(e) },
        };
        let _ = app.emit("lab-progress", ev);
        app.state::<AppState>().jobs.lock().unwrap().remove(&id);
    });
    id
}
#[tauri::command]
fn lab_export(analysis: lab::Analysis, target: String) -> Res<()> {
    let body = if target.to_ascii_lowercase().ends_with(".csv") { lab::report_csv(&analysis) } else { lab::report_json(&analysis) };
    std::fs::write(&target, body).map_err(|e| e.to_string())
}
/// Decodes one audio stream (e.g. an IAMF substream) to WAV for auditioning.
#[tauri::command]
async fn lab_extract(path: String, stream: usize, app: tauri::AppHandle) -> Res<String> {
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("lab");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem = std::path::Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let out = dir.join(format!("{stem}-a{stream}.wav"));
    let o = out.clone();
    tauri::async_runtime::spawn_blocking(move || lab::extract_stream(&path, stream, &o)).await.map_err(|e| e.to_string())??;
    Ok(out.to_string_lossy().into_owned())
}
#[tauri::command]
fn probe_file(path: String) -> Res<probe::ProbeResult> {
    probe::probe(&path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    STARTED.get_or_init(std::time::Instant::now);
    let (engine, engine_error) = match engine::vlc::VlcEngine::load() {
        Ok(e) => (Some(Box::new(e) as Box<dyn MediaEngine>), None),
        Err(e) => (None, Some(e)),
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            engine: Mutex::new(engine),
            engine_error,
            surface: Mutex::new(None),
            db: std::sync::OnceLock::new(),
            browser: Mutex::new(None),
            jobs: Mutex::new(Default::default()),
            job_seq: Default::default(),
        })
        .setup(|app| {
            let st = app.state::<AppState>();
            match app.path().app_data_dir() {
                Ok(dir) => {
                    let _ = std::fs::create_dir_all(&dir);
                    match db::Db::open(&dir.join("halcyon.db")) {
                        Ok(d) => {
                            let _ = st.db.set(d);
                        }
                        Err(e) => eprintln!("storage: {e}"),
                    }
                }
                Err(e) => eprintln!("storage: {e}"),
            }
            if let Ok(mut g) = st.engine.lock() {
                if let Some(e) = g.as_mut() {
                    let handle = app.handle().clone();
                    e.set_event_sink(std::sync::Arc::new(move |ev| {
                        let _ = handle.emit("engine", ev);
                    }));
                    let cfg: EngineConfig = st.db.get().and_then(|d| d.setting("engine")).unwrap_or_default();
                    if let Err(err) = e.configure(&cfg) {
                        eprintln!("engine config rejected, using defaults: {err}");
                    }
                }
            }
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
            set_deinterlace, set_chapter, video_size, stats, meta, snapshot, set_video_rect, set_cursor_hidden,
            get_settings, set_setting, configure_engine, engine_config, equalizer_info, set_equalizer, set_video_adjust,
            set_crop, audio_devices, set_audio_device, browse, discovery_services, discover, engine_plugins,
            library_roots, library_add_root, library_remove_root, library_scan, library_list, library_albums,
            library_album_tracks, library_artists, library_artist_tracks, media_row, set_favorite, cover, lyrics,
            playlists, playlist_create, playlist_update, playlist_delete, playlist_items, playlist_set_items,
            playlist_add, playlist_export, playlist_import, record_play, listening_stats, clear_history, resume_get,
            resume_set, convert_presets, convert_start, cancel_job, lab_analyze, lab_export, lab_extract, probe_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running Halcyon");
}
