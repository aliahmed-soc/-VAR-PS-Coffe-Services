import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const transport = readFileSync("src-tauri/src/sync/transport.rs", "utf8");
const example = readFileSync(".env.example", "utf8");
const validate = readFileSync("scripts/validate-nsis-artifact.ps1", "utf8");
const pkg = readFileSync(".github/workflows/package-windows.yml", "utf8");

describe("hosted production config guards", () => {
  it("targets the hosted project URL without embedding secret keys", () => {
    expect(transport).toContain("https://rbxtxtlssknjioaveytg.supabase.co");
    expect(transport).toContain("sb_publishable_");
    expect(transport).toContain("b[9] == b'_'");
    expect(transport).toContain("elevated_key_forbidden");
    expect(transport).toContain("hosted_requires_https");
    expect(transport.split("#[cfg(test)]")[0]).not.toMatch(/sb_secret_/);
    expect(example).not.toMatch(/sb_secret_[A-Za-z0-9]/);
  });

  it("bootstraps first online login from RLS PostgREST in Rust, not React", () => {
    const login = readFileSync("src-tauri/src/commands.rs", "utf8");
    const reference = readFileSync("src-tauri/src/auth/reference.rs", "utf8");
    const ui = readFileSync("src/App.tsx", "utf8");
    expect(login).toContain("complete_online_login");
    expect(login).not.toContain("no local branch assignment");
    expect(reference).toContain("fetch_reference_snapshot");
    expect(reference).toContain("user_branch_roles");
    expect(reference).toContain("cashier must have exactly one active branch");
    expect(ui).toContain('invoke("login_online"');
    expect(ui).not.toMatch(/createClient|supabase-js|\/rest\/v1\//);
  });

  it("keeps packaging from compiling or shipping project secrets", () => {
    expect(validate).toContain("sb_secret_");
    expect(pkg).toContain("never sb_secret");
    expect(pkg).toContain("PSC_SUPABASE_ANON_KEY");
  });
});
