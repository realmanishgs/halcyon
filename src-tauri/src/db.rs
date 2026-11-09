//! Local SQLite store: settings, media library, playlists (manual and smart),
//! favourites, resume positions and listening history. Everything stays on
//! this device; there is no sync and no telemetry.

use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

pub struct Db(Mutex<Connection>);

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS roots (path TEXT PRIMARY KEY);
CREATE TABLE IF NOT EXISTS media (
  path TEXT PRIMARY KEY, kind TEXT NOT NULL, title TEXT, artist TEXT, album TEXT, album_artist TEXT,
  genre TEXT, year INTEGER, track INTEGER, disc INTEGER, duration_ms INTEGER, codec TEXT,
  width INTEGER, height INTEGER, size INTEGER, mtime INTEGER, added_at INTEGER NOT NULL,
  favorite INTEGER NOT NULL DEFAULT 0, play_count INTEGER NOT NULL DEFAULT 0, last_played INTEGER, lyrics TEXT);
CREATE INDEX IF NOT EXISTS media_kind ON media(kind);
CREATE INDEX IF NOT EXISTS media_album ON media(album_artist, album);
CREATE TABLE IF NOT EXISTS playlists (id INTEGER PRIMARY KEY, name TEXT NOT NULL, rules TEXT, created INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS playlist_items (playlist_id INTEGER NOT NULL, pos INTEGER NOT NULL, path TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS pli ON playlist_items(playlist_id, pos);
CREATE TABLE IF NOT EXISTS plays (path TEXT NOT NULL, at INTEGER NOT NULL, listened_ms INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS plays_at ON plays(at);
CREATE TABLE IF NOT EXISTS resume (path TEXT PRIMARY KEY, ms INTEGER NOT NULL);
";

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn e(err: rusqlite::Error) -> String {
    err.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MediaRow {
    pub path: String,
    pub kind: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i64>,
    pub track: Option<i64>,
    pub disc: Option<i64>,
    pub duration_ms: Option<i64>,
    pub codec: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub size: Option<i64>,
    pub mtime: Option<i64>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub play_count: i64,
    pub last_played: Option<i64>,
    pub lyrics: Option<String>,
}

const COLS: &str = "path, kind, title, artist, album, album_artist, genre, year, track, disc, duration_ms, codec, width, height, size, mtime, favorite, play_count, last_played, lyrics";

fn row(r: &rusqlite::Row) -> rusqlite::Result<MediaRow> {
    Ok(MediaRow {
        path: r.get(0)?,
        kind: r.get(1)?,
        title: r.get(2)?,
        artist: r.get(3)?,
        album: r.get(4)?,
        album_artist: r.get(5)?,
        genre: r.get(6)?,
        year: r.get(7)?,
        track: r.get(8)?,
        disc: r.get(9)?,
        duration_ms: r.get(10)?,
        codec: r.get(11)?,
        width: r.get(12)?,
        height: r.get(13)?,
        size: r.get(14)?,
        mtime: r.get(15)?,
        favorite: r.get::<_, i64>(16)? != 0,
        play_count: r.get(17)?,
        last_played: r.get(18)?,
        lyrics: r.get(19)?,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct Album {
    pub album: String,
    pub artist: String,
    pub year: Option<i64>,
    pub tracks: i64,
    pub duration_ms: i64,
    pub first_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Artist {
    pub artist: String,
    pub albums: i64,
    pub tracks: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rule {
    pub field: String,
    pub op: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartRules {
    /// "all" or "any".
    #[serde(rename = "match")]
    pub match_: String,
    pub rules: Vec<Rule>,
    pub limit: Option<i64>,
    /// "title", "artist", "album", "added", "played", "plays", "random".
    pub sort: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub rules: Option<SmartRules>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListeningStats {
    pub total_ms: i64,
    pub plays: i64,
    pub top_tracks: Vec<(String, i64)>,
    pub top_artists: Vec<(String, i64)>,
    /// (unix day, listened ms) for the last 30 days.
    pub per_day: Vec<(i64, i64)>,
}

/// Columns a smart rule may reference, with their SQL expression. Anything
/// else is rejected, so rule values never reach SQL except as parameters.
fn rule_column(field: &str) -> Option<(&'static str, bool)> {
    // (expression, numeric)
    Some(match field {
        "title" => ("COALESCE(title, path)", false),
        "artist" => ("COALESCE(artist, '')", false),
        "album" => ("COALESCE(album, '')", false),
        "genre" => ("COALESCE(genre, '')", false),
        "kind" => ("kind", false),
        "path" => ("path", false),
        "year" => ("COALESCE(year, 0)", true),
        "plays" => ("play_count", true),
        "favorite" => ("favorite", true),
        "duration_s" => ("COALESCE(duration_ms, 0) / 1000", true),
        "added_days" => ("(strftime('%s','now') - added_at) / 86400", true),
        "played_days" => ("(strftime('%s','now') - COALESCE(last_played, 0)) / 86400", true),
        _ => return None,
    })
}

pub fn smart_where(r: &SmartRules) -> Result<(String, Vec<String>), String> {
    let mut parts = Vec::new();
    let mut args = Vec::new();
    for rule in &r.rules {
        let (col, numeric) = rule_column(&rule.field).ok_or_else(|| format!("Unknown field {}", rule.field))?;
        let sql = match (rule.op.as_str(), numeric) {
            ("contains", false) => format!("{col} LIKE '%' || ? || '%' ESCAPE '\\'"),
            ("not_contains", false) => format!("{col} NOT LIKE '%' || ? || '%' ESCAPE '\\'"),
            ("is", false) => format!("{col} = ? COLLATE NOCASE"),
            ("is_not", false) => format!("{col} <> ? COLLATE NOCASE"),
            ("is", true) => format!("{col} = CAST(? AS INTEGER)"),
            ("is_not", true) => format!("{col} <> CAST(? AS INTEGER)"),
            ("gt", true) => format!("{col} > CAST(? AS INTEGER)"),
            ("lt", true) => format!("{col} < CAST(? AS INTEGER)"),
            (op, _) => return Err(format!("Operator {op} not valid for {}", rule.field)),
        };
        parts.push(sql);
        let v = if rule.op.contains("contains") {
            rule.value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
        } else {
            rule.value.clone()
        };
        args.push(v);
    }
    let joiner = if r.match_ == "any" { " OR " } else { " AND " };
    let w = if parts.is_empty() { "1".to_string() } else { parts.join(joiner) };
    Ok((w, args))
}

fn order_by(sort: Option<&str>) -> &'static str {
    match sort {
        Some("artist") => "artist COLLATE NOCASE, album COLLATE NOCASE, disc, track",
        Some("album") => "album_artist COLLATE NOCASE, album COLLATE NOCASE, disc, track",
        Some("added") => "added_at DESC",
        Some("played") => "last_played DESC",
        Some("plays") => "play_count DESC",
        Some("random") => "random()",
        _ => "COALESCE(title, path) COLLATE NOCASE",
    }
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, String> {
        let c = Connection::open(path).map_err(e)?;
        c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").map_err(e)?;
        c.execute_batch(SCHEMA).map_err(e)?;
        Ok(Self(Mutex::new(c)))
    }

    #[cfg(test)]
    pub fn memory() -> Self {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(SCHEMA).unwrap();
        Self(Mutex::new(c))
    }

    fn c(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    // ---------- settings ----------
    pub fn settings(&self) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        let c = self.c();
        let mut st = c.prepare("SELECT key, value FROM settings").map_err(e)?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(e)?;
        let mut m = serde_json::Map::new();
        for r in rows.flatten() {
            if let Ok(v) = serde_json::from_str(&r.1) {
                m.insert(r.0, v);
            }
        }
        Ok(m)
    }

    pub fn set_setting(&self, key: &str, value: &serde_json::Value) -> Result<(), String> {
        self.c()
            .execute("INSERT INTO settings(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value.to_string()])
            .map(|_| ())
            .map_err(e)
    }

    pub fn setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let v: Option<String> = self.c().query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional().ok()?;
        v.and_then(|v| serde_json::from_str(&v).ok())
    }

    // ---------- resume positions ----------
    pub fn resume_get(&self, path: &str) -> Option<i64> {
        self.c().query_row("SELECT ms FROM resume WHERE path = ?1", [path], |r| r.get(0)).optional().ok().flatten()
    }

    pub fn resume_set(&self, path: &str, ms: Option<i64>) -> Result<(), String> {
        let c = self.c();
        match ms {
            Some(ms) => c.execute("INSERT INTO resume(path, ms) VALUES(?1, ?2) ON CONFLICT(path) DO UPDATE SET ms = excluded.ms", params![path, ms]),
            None => c.execute("DELETE FROM resume WHERE path = ?1", [path]),
        }
        .map(|_| ())
        .map_err(e)
    }

    // ---------- library ----------
    pub fn roots(&self) -> Result<Vec<String>, String> {
        let c = self.c();
        let mut st = c.prepare("SELECT path FROM roots ORDER BY path").map_err(e)?;
        let v = st.query_map([], |r| r.get(0)).map_err(e)?.flatten().collect();
        Ok(v)
    }

    pub fn add_root(&self, path: &str) -> Result<(), String> {
        self.c().execute("INSERT OR IGNORE INTO roots(path) VALUES(?1)", [path]).map(|_| ()).map_err(e)
    }

    /// Removes a root and the library entries that only it contained.
    pub fn remove_root(&self, path: &str) -> Result<(), String> {
        let c = self.c();
        c.execute("DELETE FROM roots WHERE path = ?1", [path]).map_err(e)?;
        let roots: Vec<String> = {
            let mut st = c.prepare("SELECT path FROM roots").map_err(e)?;
            let v = st.query_map([], |r| r.get(0)).map_err(e)?.flatten().collect();
            v
        };
        let mut st = c.prepare("SELECT path FROM media").map_err(e)?;
        let all: Vec<String> = st.query_map([], |r| r.get(0)).map_err(e)?.flatten().collect();
        for p in all {
            if p.starts_with(path) && !roots.iter().any(|r| p.starts_with(r.as_str())) {
                c.execute("DELETE FROM media WHERE path = ?1", [&p]).map_err(e)?;
            }
        }
        Ok(())
    }

    /// (size, mtime) of what is stored for each path under the library.
    pub fn known(&self) -> Result<std::collections::HashMap<String, (i64, i64)>, String> {
        let c = self.c();
        let mut st = c.prepare("SELECT path, COALESCE(size, -1), COALESCE(mtime, -1) FROM media").map_err(e)?;
        let v = st.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?)))).map_err(e)?.flatten().collect();
        Ok(v)
    }

    pub fn upsert_media(&self, m: &MediaRow) -> Result<(), String> {
        self.c()
            .execute(
                "INSERT INTO media(path, kind, title, artist, album, album_artist, genre, year, track, disc, duration_ms, codec, width, height, size, mtime, added_at, lyrics)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
                 ON CONFLICT(path) DO UPDATE SET kind=excluded.kind, title=excluded.title, artist=excluded.artist, album=excluded.album,
                   album_artist=excluded.album_artist, genre=excluded.genre, year=excluded.year, track=excluded.track, disc=excluded.disc,
                   duration_ms=excluded.duration_ms, codec=excluded.codec, width=excluded.width, height=excluded.height, size=excluded.size,
                   mtime=excluded.mtime, lyrics=excluded.lyrics",
                params![
                    m.path, m.kind, m.title, m.artist, m.album, m.album_artist, m.genre, m.year, m.track, m.disc, m.duration_ms, m.codec,
                    m.width, m.height, m.size, m.mtime, now(), m.lyrics
                ],
            )
            .map(|_| ())
            .map_err(e)
    }

    pub fn remove_media(&self, path: &str) -> Result<(), String> {
        self.c().execute("DELETE FROM media WHERE path = ?1", [path]).map(|_| ()).map_err(e)
    }

    pub fn get_media(&self, path: &str) -> Option<MediaRow> {
        self.c().query_row(&format!("SELECT {COLS} FROM media WHERE path = ?1"), [path], row).optional().ok().flatten()
    }

    /// `kind`: "audio", "video" or None for both. `search` matches title/artist/album/path.
    pub fn list_media(&self, kind: Option<&str>, search: Option<&str>, favorites: bool, sort: Option<&str>) -> Result<Vec<MediaRow>, String> {
        let mut w = vec!["kind <> 'unknown'".to_string()];
        let mut a: Vec<String> = Vec::new();
        if let Some(k) = kind {
            w.push("kind = ?".into());
            a.push(k.into());
        }
        if let Some(q) = search.filter(|q| !q.trim().is_empty()) {
            w.push("(COALESCE(title,'') || ' ' || COALESCE(artist,'') || ' ' || COALESCE(album,'') || ' ' || path) LIKE '%' || ? || '%' ESCAPE '\\'".into());
            a.push(q.trim().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        }
        if favorites {
            w.push("favorite = 1".into());
        }
        let sql = format!("SELECT {COLS} FROM media WHERE {} ORDER BY {}", w.join(" AND "), order_by(sort));
        let c = self.c();
        let mut st = c.prepare(&sql).map_err(e)?;
        let v = st.query_map(params_from_iter(a.iter()), row).map_err(e)?.flatten().collect();
        Ok(v)
    }

    pub fn albums(&self) -> Result<Vec<Album>, String> {
        let c = self.c();
        let mut st = c
            .prepare(
                "SELECT COALESCE(album, 'Unknown album'), COALESCE(album_artist, artist, 'Unknown artist') AS aa, MAX(year), COUNT(*),
                        COALESCE(SUM(duration_ms), 0), MIN(path)
                 FROM media WHERE kind = 'audio' GROUP BY 1, 2 ORDER BY aa COLLATE NOCASE, 1 COLLATE NOCASE",
            )
            .map_err(e)?;
        let v = st
            .query_map([], |r| Ok(Album { album: r.get(0)?, artist: r.get(1)?, year: r.get(2)?, tracks: r.get(3)?, duration_ms: r.get(4)?, first_path: r.get(5)? }))
            .map_err(e)?
            .flatten()
            .collect();
        Ok(v)
    }

    pub fn album_tracks(&self, album: &str, artist: &str) -> Result<Vec<MediaRow>, String> {
        let c = self.c();
        let mut st = c
            .prepare(&format!(
                "SELECT {COLS} FROM media WHERE kind = 'audio' AND COALESCE(album, 'Unknown album') = ?1
                 AND COALESCE(album_artist, artist, 'Unknown artist') = ?2 ORDER BY COALESCE(disc, 1), COALESCE(track, 9999), path"
            ))
            .map_err(e)?;
        let v = st.query_map(params![album, artist], row).map_err(e)?.flatten().collect();
        Ok(v)
    }

    pub fn artists(&self) -> Result<Vec<Artist>, String> {
        let c = self.c();
        let mut st = c
            .prepare(
                "SELECT COALESCE(artist, 'Unknown artist'), COUNT(DISTINCT COALESCE(album, '')), COUNT(*)
                 FROM media WHERE kind = 'audio' GROUP BY 1 ORDER BY 1 COLLATE NOCASE",
            )
            .map_err(e)?;
        let v = st.query_map([], |r| Ok(Artist { artist: r.get(0)?, albums: r.get(1)?, tracks: r.get(2)? })).map_err(e)?.flatten().collect();
        Ok(v)
    }

    pub fn artist_tracks(&self, artist: &str) -> Result<Vec<MediaRow>, String> {
        let c = self.c();
        let mut st = c
            .prepare(&format!(
                "SELECT {COLS} FROM media WHERE kind = 'audio' AND COALESCE(artist, 'Unknown artist') = ?1
                 ORDER BY album COLLATE NOCASE, COALESCE(disc, 1), COALESCE(track, 9999)"
            ))
            .map_err(e)?;
        let v = st.query_map([artist], row).map_err(e)?.flatten().collect();
        Ok(v)
    }

    pub fn set_favorite(&self, path: &str, fav: bool) -> Result<(), String> {
        let c = self.c();
        let n = c.execute("UPDATE media SET favorite = ?2 WHERE path = ?1", params![path, fav as i64]).map_err(e)?;
        if n == 0 && fav {
            // Not in the library yet (opened directly): keep a minimal row so the favourite sticks.
            c.execute("INSERT INTO media(path, kind, added_at, favorite) VALUES(?1, 'unknown', ?2, 1)", params![path, now()]).map_err(e)?;
        }
        Ok(())
    }

    // ---------- history & stats ----------
    pub fn record_play(&self, path: &str, listened_ms: i64) -> Result<(), String> {
        let c = self.c();
        let t = now();
        c.execute("INSERT INTO plays(path, at, listened_ms) VALUES(?1, ?2, ?3)", params![path, t, listened_ms]).map_err(e)?;
        c.execute("UPDATE media SET play_count = play_count + 1, last_played = ?2 WHERE path = ?1", params![path, t]).map_err(e)?;
        Ok(())
    }

    pub fn stats(&self) -> Result<ListeningStats, String> {
        let c = self.c();
        let (total_ms, plays): (i64, i64) =
            c.query_row("SELECT COALESCE(SUM(listened_ms), 0), COUNT(*) FROM plays", [], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?;
        let pairs = |sql: &str| -> Result<Vec<(String, i64)>, String> {
            let mut st = c.prepare(sql).map_err(e)?;
            let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?.flatten().collect();
            Ok(v)
        };
        let top_tracks = pairs("SELECT path, COUNT(*) n FROM plays GROUP BY path ORDER BY n DESC LIMIT 10")?;
        let top_artists = pairs(
            "SELECT COALESCE(m.artist, 'Unknown artist'), COUNT(*) n FROM plays p LEFT JOIN media m ON m.path = p.path
             WHERE m.kind IS NULL OR m.kind = 'audio' GROUP BY 1 ORDER BY n DESC LIMIT 10",
        )?;
        let since = now() - 30 * 86400;
        let mut st = c.prepare("SELECT at / 86400 d, SUM(listened_ms) FROM plays WHERE at >= ?1 GROUP BY d ORDER BY d").map_err(e)?;
        let per_day = st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?.flatten().collect();
        Ok(ListeningStats { total_ms, plays, top_tracks, top_artists, per_day })
    }

    pub fn clear_history(&self) -> Result<(), String> {
        self.c().execute_batch("DELETE FROM plays; UPDATE media SET play_count = 0, last_played = NULL;").map_err(e)
    }

    // ---------- playlists ----------
    pub fn playlists(&self) -> Result<Vec<Playlist>, String> {
        let c = self.c();
        let mut st = c
            .prepare("SELECT p.id, p.name, p.rules, (SELECT COUNT(*) FROM playlist_items i WHERE i.playlist_id = p.id) FROM playlists p ORDER BY p.name COLLATE NOCASE")
            .map_err(e)?;
        let v: Vec<(i64, String, Option<String>, i64)> =
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(e)?.flatten().collect();
        drop(st);
        drop(c);
        let mut out = Vec::new();
        for (id, name, rules, count) in v {
            let rules: Option<SmartRules> = rules.and_then(|r| serde_json::from_str(&r).ok());
            let count = match &rules {
                Some(r) => self.smart_items(r).map(|v| v.len() as i64).unwrap_or(0),
                None => count,
            };
            out.push(Playlist { id, name, rules, count });
        }
        Ok(out)
    }

    pub fn create_playlist(&self, name: &str, rules: Option<&SmartRules>, items: &[String]) -> Result<i64, String> {
        if let Some(r) = rules {
            smart_where(r)?; // validate before storing
        }
        let c = self.c();
        let rules = rules.map(|r| serde_json::to_string(r).unwrap());
        c.execute("INSERT INTO playlists(name, rules, created) VALUES(?1, ?2, ?3)", params![name, rules, now()]).map_err(e)?;
        let id = c.last_insert_rowid();
        drop(c);
        self.set_playlist_items(id, items)?;
        Ok(id)
    }

    pub fn update_playlist(&self, id: i64, name: &str, rules: Option<&SmartRules>) -> Result<(), String> {
        if let Some(r) = rules {
            smart_where(r)?;
        }
        let rules = rules.map(|r| serde_json::to_string(r).unwrap());
        self.c().execute("UPDATE playlists SET name = ?2, rules = ?3 WHERE id = ?1", params![id, name, rules]).map(|_| ()).map_err(e)
    }

    pub fn delete_playlist(&self, id: i64) -> Result<(), String> {
        let c = self.c();
        c.execute("DELETE FROM playlist_items WHERE playlist_id = ?1", [id]).map_err(e)?;
        c.execute("DELETE FROM playlists WHERE id = ?1", [id]).map(|_| ()).map_err(e)
    }

    pub fn set_playlist_items(&self, id: i64, items: &[String]) -> Result<(), String> {
        let mut c = self.c();
        let tx = c.transaction().map_err(e)?;
        tx.execute("DELETE FROM playlist_items WHERE playlist_id = ?1", [id]).map_err(e)?;
        for (i, p) in items.iter().enumerate() {
            tx.execute("INSERT INTO playlist_items(playlist_id, pos, path) VALUES(?1, ?2, ?3)", params![id, i as i64, p]).map_err(e)?;
        }
        tx.commit().map_err(e)
    }

    pub fn add_to_playlist(&self, id: i64, items: &[String]) -> Result<(), String> {
        let mut cur = self.playlist_items(id)?;
        cur.extend(items.iter().cloned());
        self.set_playlist_items(id, &cur)
    }

    /// Items of a manual or smart playlist, in play order.
    pub fn playlist_items(&self, id: i64) -> Result<Vec<String>, String> {
        let rules: Option<String> = self
            .c()
            .query_row("SELECT rules FROM playlists WHERE id = ?1", [id], |r| r.get(0))
            .optional()
            .map_err(e)?
            .ok_or("No such playlist")?;
        if let Some(r) = rules.and_then(|r| serde_json::from_str::<SmartRules>(&r).ok()) {
            return Ok(self.smart_items(&r)?.into_iter().map(|m| m.path).collect());
        }
        let c = self.c();
        let mut st = c.prepare("SELECT path FROM playlist_items WHERE playlist_id = ?1 ORDER BY pos").map_err(e)?;
        let v = st.query_map([id], |r| r.get(0)).map_err(e)?.flatten().collect();
        Ok(v)
    }

    pub fn smart_items(&self, r: &SmartRules) -> Result<Vec<MediaRow>, String> {
        let (w, args) = smart_where(r)?;
        let limit = r.limit.filter(|l| *l > 0).map(|l| format!(" LIMIT {}", l.min(100_000))).unwrap_or_default();
        let sql = format!("SELECT {COLS} FROM media WHERE kind <> 'unknown' AND ({w}) ORDER BY {}{limit}", order_by(r.sort.as_deref()));
        let c = self.c();
        let mut st = c.prepare(&sql).map_err(e)?;
        let v = st.query_map(params_from_iter(args.iter()), row).map_err(e)?.flatten().collect();
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(path: &str, kind: &str, artist: &str, album: &str, track: i64, genre: &str) -> MediaRow {
        MediaRow {
            path: path.into(),
            kind: kind.into(),
            title: Some(path.trim_end_matches(".flac").into()),
            artist: Some(artist.into()),
            album: Some(album.into()),
            genre: Some(genre.into()),
            track: Some(track),
            duration_ms: Some(180_000),
            ..Default::default()
        }
    }

    fn seeded() -> Db {
        let db = Db::memory();
        for r in [
            m("b.flac", "audio", "Ana", "Blue", 2, "Jazz"),
            m("a.flac", "audio", "Ana", "Blue", 1, "Jazz"),
            m("c.flac", "audio", "Bo", "Red", 1, "Rock"),
            m("v.mkv", "video", "", "", 0, ""),
        ] {
            db.upsert_media(&r).unwrap();
        }
        db
    }

    #[test]
    fn settings_round_trip() {
        let db = Db::memory();
        db.set_setting("volume", &serde_json::json!(80)).unwrap();
        db.set_setting("volume", &serde_json::json!(65)).unwrap();
        assert_eq!(db.settings().unwrap()["volume"], serde_json::json!(65));
        assert_eq!(db.setting::<i64>("volume"), Some(65));
    }

    #[test]
    fn albums_artists_and_track_order() {
        let db = seeded();
        let albums = db.albums().unwrap();
        assert_eq!(albums.len(), 2);
        assert_eq!((albums[0].album.as_str(), albums[0].tracks), ("Blue", 2));
        let t = db.album_tracks("Blue", "Ana").unwrap();
        assert_eq!(t.iter().map(|m| m.path.as_str()).collect::<Vec<_>>(), ["a.flac", "b.flac"]);
        assert_eq!(db.artists().unwrap().len(), 2);
        assert_eq!(db.list_media(Some("video"), None, false, None).unwrap().len(), 1);
        assert_eq!(db.list_media(None, Some("red"), false, None).unwrap().len(), 1);
        // LIKE wildcards in the search are literal.
        assert_eq!(db.list_media(None, Some("%"), false, None).unwrap().len(), 0);
    }

    #[test]
    fn favourites_history_and_stats() {
        let db = seeded();
        db.set_favorite("c.flac", true).unwrap();
        db.set_favorite("outside.mp3", true).unwrap();
        // The app indexes a file before favouriting it; a bare fallback row is not listed.
        assert_eq!(db.list_media(None, None, true, None).unwrap().len(), 1);
        db.record_play("c.flac", 120_000).unwrap();
        db.record_play("c.flac", 60_000).unwrap();
        db.record_play("a.flac", 30_000).unwrap();
        let s = db.stats().unwrap();
        assert_eq!((s.plays, s.total_ms), (3, 210_000));
        assert_eq!(s.top_tracks[0], ("c.flac".to_string(), 2));
        assert_eq!(s.top_artists[0], ("Bo".to_string(), 2));
        assert_eq!(db.get_media("c.flac").unwrap().play_count, 2);
    }

    #[test]
    fn smart_playlists_filter_and_reject_unknown_fields() {
        let db = seeded();
        db.record_play("c.flac", 1000).unwrap();
        let jazz = SmartRules { match_: "all".into(), rules: vec![Rule { field: "genre".into(), op: "is".into(), value: "jazz".into() }], limit: None, sort: Some("album".into()) };
        let id = db.create_playlist("Jazz", Some(&jazz), &[]).unwrap();
        assert_eq!(db.playlist_items(id).unwrap(), ["a.flac", "b.flac"]);
        let played = SmartRules { match_: "any".into(), rules: vec![Rule { field: "plays".into(), op: "gt".into(), value: "0".into() }], limit: Some(5), sort: None };
        assert_eq!(db.smart_items(&played).unwrap().len(), 1);
        let bad = SmartRules { match_: "all".into(), rules: vec![Rule { field: "path; DROP TABLE media".into(), op: "is".into(), value: "x".into() }], limit: None, sort: None };
        assert!(db.create_playlist("x", Some(&bad), &[]).is_err());
        let bad_op = SmartRules { match_: "all".into(), rules: vec![Rule { field: "plays".into(), op: "contains".into(), value: "x".into() }], limit: None, sort: None };
        assert!(smart_where(&bad_op).is_err());
        assert_eq!(db.playlists().unwrap()[0].count, 2);
    }

    #[test]
    fn manual_playlists_keep_order() {
        let db = seeded();
        let id = db.create_playlist("Mix", None, &["c.flac".into(), "a.flac".into()]).unwrap();
        db.add_to_playlist(id, &["v.mkv".into()]).unwrap();
        assert_eq!(db.playlist_items(id).unwrap(), ["c.flac", "a.flac", "v.mkv"]);
        db.delete_playlist(id).unwrap();
        assert!(db.playlist_items(id).is_err());
    }

    #[test]
    fn resume_positions() {
        let db = Db::memory();
        db.resume_set("x", Some(5000)).unwrap();
        assert_eq!(db.resume_get("x"), Some(5000));
        db.resume_set("x", None).unwrap();
        assert_eq!(db.resume_get("x"), None);
    }

    #[test]
    fn remove_root_keeps_files_covered_by_another_root() {
        let db = Db::memory();
        db.add_root("C:\\M").unwrap();
        db.add_root("C:\\M\\Sub").unwrap();
        db.upsert_media(&m("C:\\M\\a.flac", "audio", "", "", 1, "")).unwrap();
        db.upsert_media(&m("C:\\M\\Sub\\b.flac", "audio", "", "", 1, "")).unwrap();
        db.remove_root("C:\\M").unwrap();
        assert!(db.get_media("C:\\M\\a.flac").is_none());
        assert!(db.get_media("C:\\M\\Sub\\b.flac").is_some());
    }
}
