import { Fragment, useCallback, useEffect, useMemo, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import {
  api, basename, displayTitle, formatTime,
  type Album, type Artist, type BrowseItem, type ListeningStats, type MediaRow, type Playlist, type Rule,
  type ScanProgress, type SmartRules,
} from "./engine";
import { Icon } from "./icons";

export interface PlayActions {
  play: (items: string[], start?: number) => void;
  enqueue: (items: string[]) => void;
  say: (text: string) => void;
}

// ---------------------------------------------------------------- shared bits

const covers = new Map<string, Promise<string | null>>();
export function coverFor(path: string): Promise<string | null> {
  if (!covers.has(path)) covers.set(path, api.cover(path).catch(() => null));
  return covers.get(path)!;
}

export function Cover({ path, size = 160, icon = "music" }: { path: string | null; size?: number; icon?: string }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setSrc(null);
    if (path) coverFor(path).then((s) => { if (live) setSrc(s); });
    return () => { live = false; };
  }, [path]);
  return src
    ? <img className="cover" src={src} alt="" style={{ width: size, height: size }} />
    : <div className="cover" style={{ width: size, height: size }}><Icon name={icon} /></div>;
}

function Heart({ row, onChange }: { row: MediaRow; onChange: (fav: boolean) => void }) {
  return (
    <button className="icon-btn sm" aria-pressed={row.favorite} aria-label={row.favorite ? "Remove from favourites" : "Add to favourites"}
      title="Favourite" onClick={(e) => { e.stopPropagation(); onChange(!row.favorite); }}>
      <Icon name="heart" filled={row.favorite} />
    </button>
  );
}

export function TrackTable({ rows, actions, showAlbum = true, onChanged, playlists }: {
  rows: MediaRow[]; actions: PlayActions; showAlbum?: boolean; onChanged?: () => void; playlists?: Playlist[];
}) {
  const paths = useMemo(() => rows.map((r) => r.path), [rows]);
  const fav = (r: MediaRow, f: boolean) => { api.setFavorite(r.path, f).then(onChanged); };
  if (!rows.length) return <p className="muted">Nothing here yet.</p>;
  return (
    <div className="tracks" role="table" aria-label="Tracks">
      {rows.map((r, i) => (
        <div key={r.path} className="track" role="row" tabIndex={0} title={r.path}
          onDoubleClick={() => actions.play(paths, i)}
          onKeyDown={(e) => { if (e.key === "Enter") actions.play(paths, i); }}>
          <span className="tr-num" role="cell">{r.track ?? i + 1}</span>
          <span className="tr-title" role="cell">
            <button className="tr-play" onClick={() => actions.play(paths, i)}>{displayTitle(r)}</button>
            <span className="tr-sub">{[r.artist, showAlbum ? r.album : null].filter(Boolean).join(" — ") || (r.kind === "video" && r.width ? `${r.width}×${r.height} · ${r.codec ?? ""}` : "")}</span>
          </span>
          <span className="tr-meta hide-sm" role="cell">{r.play_count ? `${r.play_count} plays` : ""}</span>
          <span className="tr-time" role="cell">{r.duration_ms ? formatTime(r.duration_ms) : ""}</span>
          <span className="tr-actions" role="cell">
            <Heart row={r} onChange={(f) => fav(r, f)} />
            <button className="icon-btn sm" title="Add to queue" aria-label="Add to queue" onClick={() => { actions.enqueue([r.path]); actions.say("Added to queue"); }}><Icon name="plus" /></button>
            {playlists && playlists.some((p) => !p.rules) && (
              <select className="mini-select" aria-label="Add to playlist" value="" onChange={(e) => {
                const id = Number(e.target.value);
                if (id) api.addToPlaylist(id, [r.path]).then(() => actions.say("Added to playlist"));
              }}>
                <option value="">+ list</option>
                {playlists.filter((p) => !p.rules).map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
              </select>
            )}
          </span>
        </div>
      ))}
    </div>
  );
}

function useEvent<T>(name: string, fn: (p: T) => void) {
  useEffect(() => {
    const un = listen<T>(name, (e) => fn(e.payload));
    return () => { un.then((f) => f()); };
  }, [name]); // eslint-disable-line react-hooks/exhaustive-deps
}

function Search({ value, onChange, placeholder }: { value: string; onChange: (v: string) => void; placeholder: string }) {
  return <input className="input search" type="text" placeholder={placeholder} value={value} onChange={(e) => onChange(e.target.value)} aria-label={placeholder} />;
}

// ---------------------------------------------------------------- library (folders + everything)

export function LibraryPage({ actions }: { actions: PlayActions }) {
  const [roots, setRoots] = useState<string[]>([]);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [rows, setRows] = useState<MediaRow[]>([]);
  const [q, setQ] = useState("");
  const [kind, setKind] = useState<string | null>(null);
  const [favs, setFavs] = useState(false);
  const [sort, setSort] = useState("title");
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const reload = useCallback(() => {
    api.roots().then(setRoots);
    api.list(kind, q, favs, sort).then(setRows);
    api.playlists().then(setPlaylists);
  }, [kind, q, favs, sort]);
  useEffect(() => { reload(); }, [reload]);
  useEvent<ScanProgress>("library-progress", (p) => { setProgress(p); if (p.finished) reload(); });

  const scan = () => { setProgress({ done: 0, total: 0, added: 0, removed: 0, failed: 0, current: "", finished: false }); api.scan().catch((e) => actions.say(String(e))); };
  const add = async () => {
    const d = await openDialog({ directory: true, multiple: false });
    if (typeof d === "string") { await api.addRoot(d); reload(); scan(); }
  };
  const scanning = progress && !progress.finished;
  return (
    <section className="page">
      <div className="page-head">
        <h1>Library</h1>
        <div className="row">
          <button className="btn" onClick={add}><Icon name="folder" /> Add folder</button>
          <button className="btn" onClick={scan} disabled={!roots.length || !!scanning}>Rescan</button>
        </div>
      </div>
      {roots.length === 0 ? (
        <div className="empty-card">
          <p>Add the folders that hold your music and videos. Halcyon reads their tags with ffprobe and keeps the index on this device.</p>
          <button className="btn primary" onClick={add}><Icon name="folder" /> Add a folder</button>
        </div>
      ) : (
        <div className="roots">
          {roots.map((r) => (
            <span key={r} className="root-chip" title={r}>{r}
              <button className="icon-btn sm" aria-label={`Remove ${r}`} onClick={() => api.removeRoot(r).then(reload)}><Icon name="close" /></button>
            </span>
          ))}
        </div>
      )}
      {progress && (
        <div className="scan" role="status">
          <progress max={Math.max(1, progress.total)} value={progress.done} />
          <span className="small muted">
            {progress.finished
              ? `Indexed ${progress.total} files · ${progress.added} new or changed · ${progress.removed} removed${progress.failed ? ` · ${progress.failed} unreadable` : ""}`
              : `Scanning ${progress.done}/${progress.total}${progress.current ? ` · ${progress.current}` : ""}`}
          </span>
        </div>
      )}
      <div className="toolbar">
        <Search value={q} onChange={setQ} placeholder="Search title, artist, album, path" />
        <div className="chips">
          {[[null, "All"], ["audio", "Music"], ["video", "Videos"]].map(([k, l]) => (
            <button key={l} className={`chip${kind === k ? " on" : ""}`} onClick={() => setKind(k)}>{l}</button>
          ))}
          <button className={`chip${favs ? " on" : ""}`} onClick={() => setFavs((f) => !f)}>♥ Favourites</button>
        </div>
        <select className="select" value={sort} onChange={(e) => setSort(e.target.value)} aria-label="Sort">
          <option value="title">Title</option><option value="artist">Artist</option><option value="album">Album</option>
          <option value="added">Recently added</option><option value="played">Recently played</option><option value="plays">Most played</option>
        </select>
        <button className="btn small" disabled={!rows.length} onClick={() => actions.play(rows.map((r) => r.path))}><Icon name="play" filled /> Play all</button>
      </div>
      <p className="small muted">{rows.length} items</p>
      <TrackTable rows={rows} actions={actions} onChanged={reload} playlists={playlists} />
    </section>
  );
}

// ---------------------------------------------------------------- music

export function MusicPage({ actions }: { actions: PlayActions }) {
  const [tab, setTab] = useState<"albums" | "artists" | "songs">("albums");
  const [albums, setAlbums] = useState<Album[]>([]);
  const [artists, setArtists] = useState<Artist[]>([]);
  const [songs, setSongs] = useState<MediaRow[]>([]);
  const [open, setOpen] = useState<{ title: string; sub: string; rows: MediaRow[]; cover: string | null } | null>(null);
  const [q, setQ] = useState("");
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const reload = useCallback(() => {
    api.albums().then(setAlbums);
    api.artists().then(setArtists);
    api.list("audio", q, false, "artist").then(setSongs);
    api.playlists().then(setPlaylists);
  }, [q]);
  useEffect(() => { reload(); }, [reload]);
  const ql = q.trim().toLowerCase();

  if (open) {
    const total = open.rows.reduce((a, r) => a + (r.duration_ms ?? 0), 0);
    return (
      <section className="page">
        <button className="btn small" onClick={() => setOpen(null)}><Icon name="back" /> Music</button>
        <div className="album-head">
          <Cover path={open.cover} size={180} />
          <div>
            <h1>{open.title}</h1>
            <p className="muted">{open.sub} · {open.rows.length} tracks · {formatTime(total)}</p>
            <div className="row">
              <button className="btn primary" onClick={() => actions.play(open.rows.map((r) => r.path))}><Icon name="play" filled /> Play</button>
              <button className="btn" onClick={() => actions.play([...open.rows.map((r) => r.path)].sort(() => Math.random() - 0.5))}><Icon name="shuffle" /> Shuffle</button>
              <button className="btn" onClick={() => { actions.enqueue(open.rows.map((r) => r.path)); actions.say("Added to queue"); }}><Icon name="plus" /> Queue</button>
            </div>
          </div>
        </div>
        <TrackTable rows={open.rows} actions={actions} showAlbum={false} playlists={playlists}
          onChanged={() => setOpen({ ...open })} />
      </section>
    );
  }
  return (
    <section className="page">
      <div className="page-head">
        <h1>Music</h1>
        <Search value={q} onChange={setQ} placeholder="Filter" />
      </div>
      <div className="tabs flat" role="tablist">
        {(["albums", "artists", "songs"] as const).map((t) => (
          <button key={t} role="tab" className="tab" aria-selected={tab === t} onClick={() => setTab(t)}>
            {t === "albums" ? `Albums · ${albums.length}` : t === "artists" ? `Artists · ${artists.length}` : `Songs · ${songs.length}`}
          </button>
        ))}
      </div>
      {albums.length === 0 && songs.length === 0 && <p className="muted">No music indexed yet. Add folders in Library.</p>}
      {tab === "albums" && (
        <div className="grid">
          {albums.filter((a) => !ql || `${a.album} ${a.artist}`.toLowerCase().includes(ql)).map((a) => (
            <button key={`${a.artist}\u0000${a.album}`} className="card" onClick={() => api.albumTracks(a.album, a.artist).then((rows) => setOpen({ title: a.album, sub: `${a.artist}${a.year ? ` · ${a.year}` : ""}`, rows, cover: a.first_path }))}>
              <Cover path={a.first_path} size={148} />
              <span className="card-t">{a.album}</span>
              <span className="card-s">{a.artist}</span>
            </button>
          ))}
        </div>
      )}
      {tab === "artists" && (
        <div className="list">
          {artists.filter((a) => !ql || a.artist.toLowerCase().includes(ql)).map((a) => (
            <button key={a.artist} className="list-row" onClick={() => api.artistTracks(a.artist).then((rows) => setOpen({ title: a.artist, sub: `${a.albums} albums`, rows, cover: rows[0]?.path ?? null }))}>
              <span className="lr-t">{a.artist}</span><span className="muted small">{a.albums} albums · {a.tracks} tracks</span>
            </button>
          ))}
        </div>
      )}
      {tab === "songs" && <TrackTable rows={songs} actions={actions} onChanged={reload} playlists={playlists} />}
    </section>
  );
}

// ---------------------------------------------------------------- videos

export function VideosPage({ actions }: { actions: PlayActions }) {
  const [rows, setRows] = useState<MediaRow[]>([]);
  const [q, setQ] = useState("");
  const [sort, setSort] = useState("title");
  const reload = useCallback(() => { api.list("video", q, false, sort).then(setRows); }, [q, sort]);
  useEffect(() => { reload(); }, [reload]);
  return (
    <section className="page">
      <div className="page-head">
        <h1>Videos</h1>
        <div className="row">
          <Search value={q} onChange={setQ} placeholder="Filter" />
          <select className="select" value={sort} onChange={(e) => setSort(e.target.value)} aria-label="Sort">
            <option value="title">Title</option><option value="added">Recently added</option><option value="played">Recently played</option>
          </select>
        </div>
      </div>
      {rows.length === 0 && <p className="muted">No videos indexed yet. Add folders in Library.</p>}
      <div className="grid wide">
        {rows.map((r, i) => (
          <button key={r.path} className="card" title={r.path} onClick={() => actions.play(rows.map((x) => x.path), i)}>
            <Cover path={r.path} size={220} icon="video" />
            <span className="card-t">{displayTitle(r)}</span>
            <span className="card-s">{[r.duration_ms ? formatTime(r.duration_ms) : null, r.height ? `${r.height}p` : null, r.codec].filter(Boolean).join(" · ")}</span>
          </button>
        ))}
      </div>
    </section>
  );
}

// ---------------------------------------------------------------- playlists

const FIELDS: [string, string, boolean][] = [
  ["title", "Title", false], ["artist", "Artist", false], ["album", "Album", false], ["genre", "Genre", false],
  ["kind", "Kind (audio/video)", false], ["path", "Path", false], ["year", "Year", true], ["plays", "Play count", true],
  ["favorite", "Favourite (1/0)", true], ["duration_s", "Duration (s)", true], ["added_days", "Days since added", true],
  ["played_days", "Days since played", true],
];
const OPS: Record<string, [string, string][]> = {
  text: [["contains", "contains"], ["not_contains", "does not contain"], ["is", "is"], ["is_not", "is not"]],
  num: [["is", "="], ["is_not", "≠"], ["gt", ">"], ["lt", "<"]],
};

function RuleEditor({ rules, onChange }: { rules: SmartRules; onChange: (r: SmartRules) => void }) {
  const set = (i: number, r: Partial<Rule>) => onChange({ ...rules, rules: rules.rules.map((x, j) => (j === i ? { ...x, ...r } : x)) });
  return (
    <div className="rules">
      <div className="row">
        Match
        <select className="select" value={rules.match} onChange={(e) => onChange({ ...rules, match: e.target.value as "all" | "any" })}>
          <option value="all">all</option><option value="any">any</option>
        </select>
        of these rules
      </div>
      {rules.rules.map((r, i) => {
        const numeric = FIELDS.find((f) => f[0] === r.field)?.[2] ?? false;
        return (
          <div key={i} className="row rule">
            <select className="select" value={r.field} aria-label="Field" onChange={(e) => {
              const n = FIELDS.find((f) => f[0] === e.target.value)?.[2];
              set(i, { field: e.target.value, op: n ? "gt" : "contains" });
            }}>
              {FIELDS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
            </select>
            <select className="select" value={r.op} aria-label="Operator" onChange={(e) => set(i, { op: e.target.value })}>
              {OPS[numeric ? "num" : "text"].map(([v, l]) => <option key={v} value={v}>{l}</option>)}
            </select>
            <input className="input" type="text" value={r.value} aria-label="Value" onChange={(e) => set(i, { value: e.target.value })} />
            <button className="icon-btn sm" aria-label="Remove rule" onClick={() => onChange({ ...rules, rules: rules.rules.filter((_, j) => j !== i) })}><Icon name="close" /></button>
          </div>
        );
      })}
      <div className="row">
        <button className="btn small" onClick={() => onChange({ ...rules, rules: [...rules.rules, { field: "genre", op: "contains", value: "" }] })}><Icon name="plus" /> Rule</button>
        Limit <input className="input num" type="number" min={0} value={rules.limit ?? ""} aria-label="Limit" onChange={(e) => onChange({ ...rules, limit: e.target.value ? Number(e.target.value) : null })} />
        Sort
        <select className="select" value={rules.sort ?? "title"} onChange={(e) => onChange({ ...rules, sort: e.target.value })}>
          <option value="title">Title</option><option value="artist">Artist</option><option value="album">Album</option>
          <option value="added">Recently added</option><option value="played">Recently played</option><option value="plays">Most played</option><option value="random">Random</option>
        </select>
      </div>
    </div>
  );
}

export function PlaylistsPage({ actions, queue }: { actions: PlayActions; queue: string[] }) {
  const [lists, setLists] = useState<Playlist[]>([]);
  const [sel, setSel] = useState<Playlist | null>(null);
  const [items, setItems] = useState<string[]>([]);
  const [rows, setRows] = useState<Record<string, MediaRow | null>>({});
  const [editing, setEditing] = useState<{ name: string; rules: SmartRules | null } | null>(null);
  const reload = useCallback(() => api.playlists().then((l) => { setLists(l); setSel((s) => (s ? l.find((x) => x.id === s.id) ?? null : null)); }), []);
  useEffect(() => { reload(); }, [reload]);
  useEffect(() => {
    if (!sel) { setItems([]); return; }
    api.playlistItems(sel.id).then(async (it) => {
      setItems(it);
      const m: Record<string, MediaRow | null> = {};
      await Promise.all(it.slice(0, 500).map(async (p) => { m[p] = await api.mediaRow(p).catch(() => null); }));
      setRows(m);
    });
  }, [sel]);

  const create = async (smart: boolean) => {
    const id = await api.createPlaylist(smart ? "New smart playlist" : "New playlist",
      smart ? { match: "all", rules: [{ field: "favorite", op: "is", value: "1" }], limit: null, sort: "title" } : null, []);
    await reload();
    const l = (await api.playlists()).find((x) => x.id === id) ?? null;
    setSel(l);
    setEditing(l ? { name: l.name, rules: l.rules } : null);
  };
  const fromQueue = async () => {
    if (!queue.length) return;
    const id = await api.createPlaylist(`Queue ${new Date().toLocaleDateString()}`, null, queue);
    await reload();
    setSel((await api.playlists()).find((x) => x.id === id) ?? null);
    actions.say("Queue saved as playlist");
  };
  const importFile = async () => {
    const f = await openDialog({ multiple: false, filters: [{ name: "Playlists", extensions: ["m3u", "m3u8", "pls", "xspf"] }] });
    if (typeof f === "string") { await api.importPlaylist(f); reload(); actions.say("Playlist imported"); }
  };
  const exportFile = async () => {
    if (!sel) return;
    const t = await saveDialog({ defaultPath: `${sel.name}.m3u8`, filters: [{ name: "M3U8", extensions: ["m3u8"] }, { name: "XSPF", extensions: ["xspf"] }] });
    if (t) { await api.exportPlaylist(sel.name, items, t); actions.say(`Exported ${basename(t)}`); }
  };
  const move = (i: number, d: number) => {
    const j = i + d;
    if (!sel || sel.rules || j < 0 || j >= items.length) return;
    const c = [...items]; [c[i], c[j]] = [c[j], c[i]];
    setItems(c); api.setPlaylistItems(sel.id, c);
  };

  return (
    <section className="page split">
      <div className="pl-list">
        <div className="page-head"><h1>Playlists</h1></div>
        <div className="row">
          <button className="btn small" onClick={() => create(false)}><Icon name="plus" /> New</button>
          <button className="btn small" onClick={() => create(true)}><Icon name="plus" /> Smart</button>
          <button className="btn small" onClick={fromQueue} disabled={!queue.length} title="Save the current queue">Save queue</button>
          <button className="btn small" onClick={importFile}>Import…</button>
        </div>
        <div className="list">
          {lists.map((l) => (
            <button key={l.id} className={`list-row${sel?.id === l.id ? " on" : ""}`} onClick={() => { setSel(l); setEditing(null); }}>
              <span className="lr-t">{l.rules ? "✦ " : ""}{l.name}</span><span className="muted small">{l.count}</span>
            </button>
          ))}
          {!lists.length && <p className="muted small">No playlists yet.</p>}
        </div>
      </div>
      <div className="pl-body">
        {!sel ? <p className="muted">Select or create a playlist. Smart playlists update themselves from rules.</p> : (
          <>
            <div className="page-head">
              {editing ? (
                <input className="input title-input" value={editing.name} aria-label="Playlist name" onChange={(e) => setEditing({ ...editing, name: e.target.value })} />
              ) : <h1>{sel.name}</h1>}
              <div className="row">
                {editing ? (
                  <>
                    <button className="btn primary small" onClick={async () => {
                      try { await api.updatePlaylist(sel.id, editing.name.trim() || "Untitled", editing.rules); setEditing(null); reload(); }
                      catch (e) { actions.say(String(e)); }
                    }}>Save</button>
                    <button className="btn small" onClick={() => setEditing(null)}>Cancel</button>
                  </>
                ) : (
                  <>
                    <button className="btn primary small" disabled={!items.length} onClick={() => actions.play(items)}><Icon name="play" filled /> Play</button>
                    <button className="btn small" disabled={!items.length} onClick={() => { actions.enqueue(items); actions.say("Added to queue"); }}>Queue</button>
                    <button className="btn small" onClick={() => setEditing({ name: sel.name, rules: sel.rules })}>Edit</button>
                    <button className="btn small" onClick={exportFile} disabled={!items.length}>Export…</button>
                    <button className="btn small" onClick={async () => { await api.deletePlaylist(sel.id); setSel(null); reload(); }}>Delete</button>
                  </>
                )}
              </div>
            </div>
            {editing?.rules && <RuleEditor rules={editing.rules} onChange={(r) => setEditing({ ...editing, rules: r })} />}
            <ol className="queue-list">
              {items.map((p, i) => {
                const r = rows[p];
                return (
                  <li key={`${i}-${p}`} className="queue-item" title={p} onDoubleClick={() => actions.play(items, i)}>
                    <span className="qi-num">{i + 1}</span>
                    <button className="qi-name" onClick={() => actions.play(items, i)}>{r ? displayTitle(r) : basename(p)}{r?.artist ? <span className="muted"> — {r.artist}</span> : null}</button>
                    {!sel.rules && (
                      <span className="qi-actions">
                        <button className="icon-btn sm" onClick={() => move(i, -1)} disabled={i === 0} aria-label="Move up"><Icon name="up" /></button>
                        <button className="icon-btn sm" onClick={() => move(i, 1)} disabled={i === items.length - 1} aria-label="Move down"><Icon name="down" /></button>
                        <button className="icon-btn sm" aria-label="Remove" onClick={() => { const c = items.filter((_, j) => j !== i); setItems(c); api.setPlaylistItems(sel.id, c).then(reload); }}><Icon name="close" /></button>
                      </span>
                    )}
                  </li>
                );
              })}
            </ol>
            {!items.length && <p className="muted">{sel.rules ? "No library items match these rules." : "Empty. Add tracks from Library or Music (+ list)."}</p>}
          </>
        )}
      </div>
    </section>
  );
}

// ---------------------------------------------------------------- network

export function NetworkPage({ actions }: { actions: PlayActions }) {
  const [url, setUrl] = useState("smb://");
  const [trail, setTrail] = useState<{ name: string; url: string }[]>([]);
  const [items, setItems] = useState<BrowseItem[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [services, setServices] = useState<[string, string][]>([]);
  useEffect(() => { api.discoveryServices().then(setServices).catch(() => {}); }, []);
  const go = useCallback(async (u: string, name: string, reset = false) => {
    setBusy(true); setErr(null);
    try {
      const r = await api.browse(u);
      setItems(r);
      setTrail((t) => (reset ? [{ name, url: u }] : [...t, { name, url: u }]));
      setUrl(u);
    } catch (e) { setErr(String(e)); } finally { setBusy(false); }
  }, []);
  const discover = async (svc: string, label: string) => {
    setBusy(true); setErr(null);
    try { setItems(await api.discover(svc)); setTrail([{ name: label, url: "" }]); } catch (e) { setErr(String(e)); } finally { setBusy(false); }
  };
  const open = (it: BrowseItem) => {
    if (it.kind === "directory" || it.kind === "unknown" && !/\.[a-z0-9]{2,5}$/i.test(it.url)) go(it.url, it.name);
    else { const files = (items ?? []).filter((x) => x.kind !== "directory").map((x) => x.url); actions.play(files, Math.max(0, files.indexOf(it.url))); }
  };
  return (
    <section className="page">
      <div className="page-head"><h1>Network</h1></div>
      <form className="row" onSubmit={(e) => { e.preventDefault(); if (url.includes("://")) go(url, url, true); }}>
        <input className="input grow" type="text" value={url} onChange={(e) => setUrl(e.target.value)} aria-label="Location"
          placeholder="smb://server/share · ftp:// · sftp:// · nfs:// · upnp://" />
        <button className="btn primary" type="submit" disabled={busy}>Browse</button>
        <button className="btn" type="button" disabled={!url.includes("://")} onClick={() => actions.play([url])}>Play URL</button>
      </form>
      <h2 className="section">Discover on this network</h2>
      <div className="chips">
        {services.map(([n, l]) => <button key={n} className="chip" disabled={busy} onClick={() => discover(n, l)}>{l || n}</button>)}
        {!services.length && <span className="muted small">No discovery services in this engine.</span>}
      </div>
      {trail.length > 0 && (
        <div className="crumbs">
          {trail.map((t, i) => (
            <Fragment key={i}>
              {i > 0 && <span className="muted">/</span>}
              <button className="link" disabled={!t.url} onClick={() => { setTrail(trail.slice(0, i)); go(t.url, t.name); }}>{t.name}</button>
            </Fragment>
          ))}
        </div>
      )}
      {busy && <p className="muted" role="status">Working… (slow shares can take a few seconds)</p>}
      {err && <p className="error" role="alert">{err}</p>}
      {items && !busy && (
        <div className="list">
          {items.map((it) => (
            <button key={it.url} className="list-row" onClick={() => open(it)} title={it.url}>
              <Icon name={it.kind === "directory" ? "folder" : "music"} /><span className="lr-t">{it.name}</span><span className="muted small">{it.kind}</span>
            </button>
          ))}
          {!items.length && <p className="muted">Nothing found here.</p>}
        </div>
      )}
    </section>
  );
}

// ---------------------------------------------------------------- listening stats

export function StatsPage({ privateMode, setPrivateMode }: { privateMode: boolean; setPrivateMode: (v: boolean) => void }) {
  const [s, setS] = useState<ListeningStats | null>(null);
  const [rows, setRows] = useState<Record<string, MediaRow | null>>({});
  const reload = useCallback(() => api.stats().then(async (st) => {
    setS(st);
    const m: Record<string, MediaRow | null> = {};
    await Promise.all(st.top_tracks.map(async ([p]) => { m[p] = await api.mediaRow(p).catch(() => null); }));
    setRows(m);
  }), []);
  useEffect(() => { reload(); }, [reload]);
  const today = Math.floor(Date.now() / 86400000);
  const days = Array.from({ length: 30 }, (_, i) => today - 29 + i);
  const per = new Map(s?.per_day ?? []);
  const max = Math.max(1, ...days.map((d) => per.get(d) ?? 0));
  return (
    <section className="page">
      <div className="page-head">
        <h1>Listening</h1>
        <div className="row">
          <label className="check"><input type="checkbox" checked={privateMode} onChange={(e) => setPrivateMode(e.target.checked)} /> Private session (record nothing)</label>
          <button className="btn small" onClick={() => api.clearHistory().then(reload)}>Clear history</button>
        </div>
      </div>
      <p className="muted small">Stored only on this device. A play counts after 30 s or half the track.</p>
      <div className="stat-cards">
        <div className="stat"><span className="stat-v">{formatTime(s?.total_ms ?? 0)}</span><span className="stat-l">listened</span></div>
        <div className="stat"><span className="stat-v">{s?.plays ?? 0}</span><span className="stat-l">plays</span></div>
      </div>
      <h2 className="section">Last 30 days</h2>
      <div className="bars" role="img" aria-label="Listening time per day for the last 30 days">
        {days.map((d) => <span key={d} className="bar" style={{ height: `${((per.get(d) ?? 0) / max) * 100}%` }} title={`${new Date(d * 86400000).toLocaleDateString()}: ${formatTime(per.get(d) ?? 0)}`} />)}
      </div>
      <div className="two-col">
        <div>
          <h2 className="section">Top tracks</h2>
          <ol className="ranked">{s?.top_tracks.map(([p, n]) => <li key={p} title={p}>{rows[p] ? displayTitle(rows[p]!) : basename(p)} <span className="muted small">{n}×</span></li>)}</ol>
        </div>
        <div>
          <h2 className="section">Top artists</h2>
          <ol className="ranked">{s?.top_artists.map(([a, n]) => <li key={a}>{a} <span className="muted small">{n}×</span></li>)}</ol>
        </div>
      </div>
    </section>
  );
}
