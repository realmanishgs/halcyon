// Minimal 24px stroke icon set drawn for Halcyon (1.75 stroke, round joins).
const paths: Record<string, string> = {
  play: "M7 4.5v15l12-7.5z",
  pause: "M7 4.5h3.5v15H7zM13.5 4.5H17v15h-3.5z",
  stop: "M6 6h12v12H6z",
  step: "M6 5v14l9-7zM18 5v14",
  back: "M11 7l-5 5 5 5M18 7l-5 5 5 5",
  fwd: "M13 7l5 5-5 5M6 7l5 5-5 5",
  volume: "M4 9.5h3.5L12 5.5v13l-4.5-4H4zM15.5 9a4 4 0 010 6M18 6.5a7.5 7.5 0 010 11",
  mute: "M4 9.5h3.5L12 5.5v13l-4.5-4H4zM16 9.5l5 5M21 9.5l-5 5",
  panel: "M4 5h16v14H4zM15 5v14",
  open: "M3.5 6.5h6l2 2h9v10h-17z",
  menu: "M4 7h16M4 12h16M4 17h16",
  home: "M4 11l8-6.5 8 6.5V20H4z",
  library: "M5 4v16M10 4v16M15 4.5l4 15",
  music: "M9 18V6l10-2v12M9 18a2.5 2.5 0 11-5 0 2.5 2.5 0 015 0zM19 16a2.5 2.5 0 11-5 0 2.5 2.5 0 015 0z",
  video: "M4 6.5h11v11H4zM15 10.5l5-3v9l-5-3z",
  list: "M9 7h11M9 12h11M9 17h11M4.5 7h.01M4.5 12h.01M4.5 17h.01",
  heart: "M12 19s-7-4.4-7-9.5A3.8 3.8 0 0112 7a3.8 3.8 0 017 2.5C19 14.6 12 19 12 19z",
  clock: "M12 4a8 8 0 100 16 8 8 0 000-16zM12 8v4.5l3 2",
  lab: "M3 12h2l2-5 3 10 3-14 3 12 2-3h3",
  network: "M12 4a8 8 0 100 16 8 8 0 000-16zM4 12h16M12 4c2.5 2.5 2.5 13.5 0 16M12 4c-2.5 2.5-2.5 13.5 0 16",
  settings: "M12 9a3 3 0 100 6 3 3 0 000-6zM12 3v2.5M12 18.5V21M3 12h2.5M18.5 12H21M5.6 5.6l1.8 1.8M16.6 16.6l1.8 1.8M5.6 18.4l1.8-1.8M16.6 7.4l1.8-1.8",
  disc: "M12 4a8 8 0 100 16 8 8 0 000-16zM12 10.5a1.5 1.5 0 100 3 1.5 1.5 0 000-3z",
};

export function Icon({ name, filled }: { name: keyof typeof paths | string; filled?: boolean }) {
  return (
    <svg className="i" viewBox="0 0 24 24" aria-hidden="true"
      fill={filled ? "currentColor" : "none"} stroke="currentColor"
      strokeWidth={filled ? 0 : 1.75} strokeLinecap="round" strokeLinejoin="round">
      <path d={paths[name]} />
    </svg>
  );
}
