// Release-pipeline guard for the compile-time Supabase key.
//
// The desktop runtime already refuses elevated keys (see
// `resolve_supabase_config` in src-tauri/src/sync/transport.rs), but a rejected
// key produces a build that simply cannot reach the cloud. This guard stops the
// elevated key from ever being compiled in, and re-checks the built binaries.
//
// A legacy Supabase JWT carries its role inside a base64url payload, so a plain
// substring search for "service_role" does not see it.
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const ELEVATED_ROLES = new Set(["service_role", "supabase_admin"]);
const SECRET_PREFIX = "sb_secret_";
const PUBLISHABLE_PREFIX = "sb_publishable_";

export function jwtRole(token) {
  const parts = token.split(".");
  if (parts.length < 2) return null;
  try {
    const json = Buffer.from(parts[1], "base64url").toString("utf8");
    const role = JSON.parse(json).role;
    return typeof role === "string" ? role : null;
  } catch {
    return null;
  }
}

export function classifyCloudKey(raw) {
  const key = (raw ?? "").trim();
  if (!key) return { kind: "missing", elevated: false };
  if (key.startsWith(SECRET_PREFIX)) return { kind: "secret_prefix", elevated: true };
  if (key.includes("service_role") || key.includes("supabase_admin")) {
    return { kind: "elevated_literal", elevated: true };
  }
  const role = jwtRole(key);
  if (role && ELEVATED_ROLES.has(role)) {
    return { kind: "elevated_jwt", elevated: true, role };
  }
  if (key.startsWith(PUBLISHABLE_PREFIX)) return { kind: "publishable", elevated: false };
  if (role) return { kind: "legacy_jwt", elevated: false, role };
  return { kind: "unknown", elevated: false };
}

export function checkCloudKey(raw, { requirePublishable = false } = {}) {
  const c = classifyCloudKey(raw);
  if (c.elevated) {
    return { ...c, ok: false, reason: `elevated cloud key rejected (${c.kind})` };
  }
  if (c.kind === "missing") {
    return requirePublishable
      ? { ...c, ok: false, reason: "PSC_SUPABASE_ANON_KEY is required for this UAT build" }
      : { ...c, ok: true };
  }
  if (requirePublishable && c.kind !== "publishable") {
    return {
      ...c,
      ok: false,
      reason: `PSC_SUPABASE_ANON_KEY must be an ${PUBLISHABLE_PREFIX}… key, got ${c.kind}`,
    };
  }
  return { ...c, ok: true };
}

/** Elevated credentials embedded in a compiled artifact, as classification kinds. */
export function findElevatedKeysInBuffer(buf) {
  const ascii = buf.toString("latin1");
  const hits = [];
  if (ascii.includes(SECRET_PREFIX)) hits.push("secret_prefix");
  for (const m of ascii.matchAll(/eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}/g)) {
    const role = jwtRole(m[0]);
    if (role && ELEVATED_ROLES.has(role)) hits.push(`elevated_jwt:${role}`);
  }
  return [...new Set(hits)];
}

function main(argv) {
  const mode = argv[0];
  if (mode === "--env") {
    const res = checkCloudKey(process.env.PSC_SUPABASE_ANON_KEY, {
      requirePublishable: process.env.REQUIRE_PUBLISHABLE_KEY === "true",
    });
    if (!res.ok) {
      console.error(`cloud-key-guard: ${res.reason}`);
      return 1;
    }
    console.log(`cloud-key-guard: compile-time key accepted (kind=${res.kind}, value not printed)`);
    return 0;
  }
  if (mode === "--scan") {
    const files = argv.slice(1);
    if (files.length === 0) {
      console.error("cloud-key-guard: --scan needs at least one file");
      return 2;
    }
    let bad = 0;
    for (const f of files) {
      const hits = findElevatedKeysInBuffer(readFileSync(f));
      if (hits.length > 0) {
        console.error(`cloud-key-guard: ${f} embeds elevated credentials: ${hits.join(", ")}`);
        bad = 1;
      } else {
        console.log(`cloud-key-guard: ${f} clean`);
      }
    }
    return bad;
  }
  console.error("usage: cloud-key-guard.mjs --env | --scan <file...>");
  return 2;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exit(main(process.argv.slice(2)));
}
