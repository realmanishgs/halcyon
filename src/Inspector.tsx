import type { Diagnostics, EngineMeta, OpenResult, ProbeStream } from "./engine";
import { basename } from "./engine";

function Row({ k, v }: { k: string; v?: string | number | null }) {
  if (v === undefined || v === null || v === "") return null;
  return (<><dt>{k}</dt><dd>{v}</dd></>);
}

const fmtBitrate = (b?: string) => (b ? `${Math.round(Number(b) / 1000)} kb/s` : undefined);
const fmtBytes = (b?: string) => {
  if (!b) return undefined;
  const n = Number(b);
  return n > 1e9 ? `${(n / 1e9).toFixed(2)} GB` : `${(n / 1e6).toFixed(1)} MB`;
};
const fmtFps = (r?: string) => {
  if (!r || r === "0/0") return undefined;
  const [a, b] = r.split("/").map(Number);
  return b ? `${(a / b).toFixed(3).replace(/\.?0+$/, "")} fps` : r;
};

function StreamBlock({ s }: { s: ProbeStream }) {
  return (
    <div className="stream">
      <div className="stream-h">
        <span className="tag">{s.codec_type ?? "data"}</span>
        <span>#{s.index}</span>
        {s.tags.language && <span className="muted">{s.tags.language}</span>}
        {s.tags.title && <span className="muted">{s.tags.title}</span>}
      </div>
      <dl className="kv">
        <Row k="Codec" v={s.codec_long_name ?? s.codec_name} />
        <Row k="Profile" v={s.profile} />
        <Row k="Resolution" v={s.width && s.height ? `${s.width} × ${s.height}` : undefined} />
        <Row k="Frame rate" v={fmtFps(s.r_frame_rate)} />
        <Row k="Pixel format" v={s.pix_fmt} />
        <Row k="Transfer" v={s.color_transfer} />
        <Row k="Primaries" v={s.color_primaries} />
        <Row k="Sample rate" v={s.sample_rate ? `${s.sample_rate} Hz` : undefined} />
        <Row k="Channels" v={s.channels ? `${s.channels}${s.channel_layout ? ` (${s.channel_layout})` : ""}` : undefined} />
        <Row k="Sample format" v={s.sample_fmt} />
        <Row k="Bit depth" v={s.bits_per_raw_sample} />
        <Row k="Bitrate" v={fmtBitrate(s.bit_rate)} />
      </dl>
    </div>
  );
}

export function Inspector({ opened, meta, diag }: { opened: OpenResult | null; meta: EngineMeta | null; diag: Diagnostics | null }) {
  const p = opened?.probe;
  return (
    <aside className="inspector" aria-label="Media inspector">
      <h2>File</h2>
      {opened ? (
        <dl className="kv">
          <Row k="Name" v={basename(opened.path)} />
          <Row k="Container" v={p?.format.format_long_name ?? p?.format.format_name} />
          <Row k="Duration" v={p?.format.duration ? `${Number(p.format.duration).toFixed(2)} s` : undefined} />
          <Row k="Size" v={fmtBytes(p?.format.size)} />
          <Row k="Bitrate" v={fmtBitrate(p?.format.bit_rate)} />
          <Row k="Chapters" v={p?.chapters.length || undefined} />
        </dl>
      ) : <p className="muted">No media open.</p>}
      {opened?.probe_error && <p className="muted">Inspection unavailable: {opened.probe_error}</p>}

      {p && p.streams.length > 0 && (<><h2>Streams ({p.streams.length})</h2>{p.streams.map((s) => <StreamBlock key={s.index} s={s} />)}</>)}

      {meta && Object.values(meta).some(Boolean) && (
        <>
          <h2>Metadata</h2>
          <dl className="kv">
            <Row k="Title" v={meta.title} />
            <Row k="Artist" v={meta.artist} />
            <Row k="Album" v={meta.album} />
            <Row k="Genre" v={meta.genre} />
            <Row k="Date" v={meta.date} />
          </dl>
        </>
      )}

      <h2>Engine</h2>
      <dl className="kv">
        <Row k="Playback" v={diag?.engine ? `${diag.engine.name} ${diag.engine.version}` : diag?.engine_error ?? "—"} />
        <Row k="Inspection" v={diag?.ffprobe ? "ffprobe (out-of-process)" : "ffprobe not found"} />
      </dl>
    </aside>
  );
}
