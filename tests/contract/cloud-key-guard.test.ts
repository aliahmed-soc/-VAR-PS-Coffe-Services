import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import {
  checkCloudKey,
  classifyCloudKey,
  findElevatedKeysInBuffer,
} from "../../scripts/cloud-key-guard.mjs";

const workflow = readFileSync(".github/workflows/package-windows.yml", "utf8");
const validator = readFileSync("scripts/validate-nsis-artifact.ps1", "utf8");

function legacyJwt(role: string) {
  const head = Buffer.from(JSON.stringify({ alg: "HS256", typ: "JWT" })).toString("base64url");
  const body = Buffer.from(
    JSON.stringify({ iss: "supabase", ref: "abcdefghijklmnopqrst", role, iat: 1, exp: 2 }),
  ).toString("base64url");
  return `${head}.${body}.c2lnbmF0dXJlLXBsYWNlaG9sZGVy`;
}

describe("release-pipeline cloud key guard", () => {
  // Regression: a legacy service_role JWT was accepted by the pre-compile guard
  // because base64url hides the role claim from a substring search, and it was
  // compiled into the UAT installer. The runtime then refused it, so the build
  // reported "cloud not configured" and no hosted login was possible.
  it("rejects a legacy service_role JWT even though it has no literal role text", () => {
    const key = legacyJwt("service_role");
    expect(key).not.toContain("service_role");
    expect(classifyCloudKey(key)).toMatchObject({ kind: "elevated_jwt", elevated: true });
    expect(checkCloudKey(key).ok).toBe(false);
  });

  it("rejects supabase_admin JWTs and sb_secret_ keys", () => {
    expect(checkCloudKey(legacyJwt("supabase_admin")).ok).toBe(false);
    expect(checkCloudKey("sb_secret_abc123").ok).toBe(false);
    expect(checkCloudKey("has service_role inside").ok).toBe(false);
  });

  it("accepts a publishable key and a legacy anon JWT", () => {
    expect(checkCloudKey("sb_publishable_abc123").ok).toBe(true);
    expect(checkCloudKey(legacyJwt("anon")).ok).toBe(true);
  });

  it("requires a publishable key when the UAT build demands one", () => {
    const opts = { requirePublishable: true };
    expect(checkCloudKey("sb_publishable_abc123", opts).ok).toBe(true);
    expect(checkCloudKey(legacyJwt("anon"), opts).ok).toBe(false);
    expect(checkCloudKey("", opts).ok).toBe(false);
    expect(checkCloudKey("").ok).toBe(true);
  });

  it("detects an elevated key already embedded in a compiled artifact", () => {
    const embedded = Buffer.from(`\0\0padding${legacyJwt("service_role")}morestrings\0`, "latin1");
    expect(findElevatedKeysInBuffer(embedded)).toContain("elevated_jwt:service_role");
    expect(findElevatedKeysInBuffer(Buffer.from(`prefix${"sb_secret_"}xyz`))).toContain(
      "secret_prefix",
    );
    const clean = Buffer.from(`padding${legacyJwt("anon")}sb_publishable_abc`, "latin1");
    expect(findElevatedKeysInBuffer(clean)).toEqual([]);
  });

  it("is wired into both the pre-compile gate and the artifact validator", () => {
    expect(workflow).toContain("scripts/cloud-key-guard.mjs --env");
    expect(validator).toContain("scripts/cloud-key-guard.mjs");
    expect(validator).toContain("--scan");
  });
});
