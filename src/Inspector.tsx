import type { Diagnostics, EngineMeta, MediaStats, OpenResult, PlaybackStatus, ProbeStream } from "./engine";
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

/** Labels derived strictly from fields ffprobe reported. */
function badges(s: ProbeStream): string[] {
  const out: string[] = [];
  if (s.color_transfer === "smpte2084") out.push("HDR10 / PQ");
  if (s.color_transfer === "arib-std-b67") out.push("HLG");
  const sd = (s as unknown as { side_data_list?: { side_data_type?: string; dv_profile?: number }[] }).side_data_list ?? [];
  const dovi = sd.find((d) => /DOVI/i.test(d.side_data_type ?? ""));
  if (dovi) out.push(`Dolby Vision${dovi.dv_profile !== undefined ? ` P${dovi.dv_profile}` : ""}`);
  if (/atmos/i.test(s.profile ?? "")) out.push("Dolby Atmos");
  if (s.codec_name === "truehd") out.push("Dolby TrueHD");
  if (s.codec_name === "eac3") out.push("Dolby Digital Plus");
  if (s.codec_name === "ac3") out.push("Dolby Digital");
  if (s.codec_name === "dts") out.push(s.profile ?? "DTS");
  if (s.width && s.height && s.width * s.height >= 7680 * 4320 * 0.9) out.push("8K");
  else if (s.width && s.height && s.width * s.height >= 3840 * 2160 * 0.9) out.push("4K");
  const sr = Number(s.sample_rate ?? 0);
  const bits = Number(s.bits_per_raw_sample ?? 0) || (s.sample_fmt?.includes("32") ? 32 : s.sample_fmt?.includes("s24") ? 24 : 0);
  if (s.codec_type === "audio" && (sr > 48000 || bits >= 24)) out.push(`Hi-Res ${bits ? `${bits}-bit/` : ""}${sr / 1000} kHz`);
  return out;
}

function StreamBlock({ s }: { s: ProbeStream }) {
  const b = badges(s);
  return (
    <div className="stream">
      <div className="stream-h">
        <span className="tag">{s.codec_type ?? "data"}</span>
        <span>#{s.index}</span>
        {s.tags.language && <span className="muted">{s.tags.language}</span>}
        {s.tags.title && <span className="muted">{s.tags.title}</span>}
      </div>
      {b.length > 0 && <div className="badges">{b.map((x) => <span key={x} className="badge">{x}</span>)}</div>}
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

export function Inspector({ opened, meta, diag, stats, videoSize, status }: {
  opened: OpenResult | null; meta: EngineMeta | null; diag: Diagnostics | null;
  stats: MediaStats | null; videoSize: [number, number] | null; status: PlaybackStatus;
}) {
  const p = opened?.probe;
  return (
    <div className="inspector" aria-label="Media inspector">
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

      {p && p.stream_groups.length > 0 && (
        <>
          <h2>Stream groups ({p.stream_groups.length})</h2>
          {p.stream_groups.map((g, i) => (
            <div className="stream" key={i}>
              <div className="stream-h"><span className="tag">{String(g.type ?? "group")}</span><span>#{String(g.index ?? i)}</span></div>
              <dl className="kv">
                <Row k="ID" v={g.id !== undefined ? String(g.id) : undefined} />
                <Row k="Streams" v={g.nb_streams !== undefined ? String(g.nb_streams) : undefined} />
                {Array.isArray(g.components) && <Row k="Components" v={String((g.components as unknown[]).length)} />}
              </dl>
            </div>
          ))}
        </>
      )}

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

      {stats && (
        <>
          <h2>Playback statistics</h2>
          <dl className="kv">
            <Row k="Output size" v={videoSize ? `${videoSize[0]} × ${videoSize[1]}` : undefined} />
            <Row k="Input bitrate" v={`${Math.round(stats.input_bitrate_kbps)} kb/s`} />
            <Row k="Read" v={`${(stats.read_bytes / 1e6).toFixed(1)} MB`} />
            <Row k="Video decoded" v={status.has_video ? stats.decoded_video : undefined} />
            <Row k="Frames shown" v={status.has_video ? stats.displayed_pictures : undefined} />
            <Row k="Frames lost" v={status.has_video ? stats.lost_pictures : undefined} />
            <Row k="Audio decoded" v={stats.decoded_audio} />
            <Row k="Audio buffers lost" v={stats.lost_abuffers} />
            <Row k="Demux errors" v={stats.demux_corrupted + stats.demux_discontinuity} />
          </dl>
        </>
      )}

      <h2>Engine</h2>
      <dl className="kv">
        <Row k="Playback" v={diag?.engine ? `${diag.engine.name} ${diag.engine.version}` : diag?.engine_error ?? "—"} />
        <Row k="Inspection" v={diag?.ffprobe ? "ffprobe (out-of-process)" : "ffprobe not found"} />
      </dl>
    </div>
  );
}
