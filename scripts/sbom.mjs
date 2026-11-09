// Writes a CycloneDX 1.5 SBOM (sbom.cdx.json) for Halcyon: every Rust crate
// in the resolved dependency graph (cargo metadata) and every npm package in
// package-lock.json, plus the runtime components loaded from the user's system.
// Usage: node scripts/sbom.mjs [output-path]
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { randomUUID } from "node:crypto";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = process.argv[2] ?? join(root, "sbom.cdx.json");
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));

const meta = JSON.parse(execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], {
  cwd: join(root, "src-tauri"), maxBuffer: 64 * 1024 * 1024,
}).toString());
const resolved = new Set(meta.resolve.nodes.map((n) => n.id));
const crates = meta.packages
  .filter((p) => resolved.has(p.id) && p.source) // skip the workspace crate itself
  .map((p) => ({
    type: "library",
    "bom-ref": `pkg:cargo/${p.name}@${p.version}`,
    name: p.name,
    version: p.version,
    purl: `pkg:cargo/${p.name}@${p.version}`,
    ...(p.license ? { licenses: [{ expression: p.license }] } : {}),
    ...(p.repository ? { externalReferences: [{ type: "vcs", url: p.repository }] } : {}),
  }));

const lock = JSON.parse(readFileSync(join(root, "package-lock.json"), "utf8"));
const npm = Object.entries(lock.packages ?? {})
  .filter(([path, p]) => path.startsWith("node_modules/") && p.version)
  .map(([path, p]) => {
    const name = path.slice(path.lastIndexOf("node_modules/") + 13);
    const purl = `pkg:npm/${name.replace("@", "%40")}@${p.version}`;
    return {
      type: "library", "bom-ref": purl, name, version: p.version, purl,
      scope: p.dev ? "excluded" : "required",
      ...(p.license ? { licenses: [{ expression: p.license }] } : {}),
    };
  });

const runtime = [
  { type: "application", "bom-ref": "ext:libvlc", name: "libVLC", version: "3.0.x", description: "Loaded at runtime from the user's VLC installation (not distributed)", licenses: [{ expression: "LGPL-2.1-or-later" }] },
  { type: "application", "bom-ref": "ext:ffmpeg", name: "FFmpeg (ffprobe, ffmpeg)", version: "user-installed", description: "Run as separate processes from the user's PATH (not distributed)" },
  { type: "framework", "bom-ref": "ext:webview2", name: "Microsoft Edge WebView2", version: "system", description: "System component on Windows" },
];

const bom = {
  bomFormat: "CycloneDX",
  specVersion: "1.5",
  serialNumber: `urn:uuid:${randomUUID()}`,
  version: 1,
  metadata: {
    timestamp: new Date().toISOString(),
    tools: { components: [{ type: "application", name: "halcyon scripts/sbom.mjs" }] },
    component: { type: "application", "bom-ref": "halcyon", name: "Halcyon", version: pkg.version },
  },
  components: [...crates, ...npm, ...runtime],
};
writeFileSync(out, JSON.stringify(bom, null, 2));
console.log(`SBOM: ${crates.length} crates, ${npm.length} npm packages, ${runtime.length} runtime components -> ${out}`);
