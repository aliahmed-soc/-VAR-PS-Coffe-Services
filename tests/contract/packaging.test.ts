import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const workflow = readFileSync(".github/workflows/package-windows.yml", "utf8");
const tauri = readFileSync("src-tauri/tauri.conf.json", "utf8");
const release = readFileSync("src-tauri/tauri.release.conf.json", "utf8");
const gitignore = readFileSync(".gitignore", "utf8");
const commands = readFileSync("src-tauri/src/commands.rs", "utf8");
const app = readFileSync("src/App.tsx", "utf8");
const transport = readFileSync("src-tauri/src/sync/transport.rs", "utf8");

describe("Windows packaging gate", () => {
  it("has a dedicated manual NSIS workflow", () => {
    expect(workflow).toContain("workflow_dispatch");
    expect(workflow).toContain("npm run tauri -- build --bundles nsis");
    expect(workflow).toContain("scripts/validate-nsis-artifact.ps1");
    expect(workflow).toContain("scripts/smoke-nsis-install.ps1");
    expect(workflow).toContain("actions/upload-artifact@v4");
    expect(workflow).not.toMatch(/softprops\/action-gh-release|gh release create/);
    expect(existsSync("scripts/validate-nsis-artifact.ps1")).toBe(true);
    expect(existsSync("scripts/smoke-nsis-install.ps1")).toBe(true);
  });

  it("keeps NSIS bundling active without committing artifacts", () => {
    const conf = JSON.parse(tauri);
    expect(conf.bundle.active).toBe(true);
    expect(conf.bundle.targets).toBe("nsis");
    expect(gitignore).toMatch(/src-tauri\/target/);
    expect(gitignore).toMatch(/\.pfx/);
    expect(gitignore).toMatch(/nsis-sha256\.txt/);
  });

  it("uses a release CSP without Vite localhost and rejects packaged secrets", () => {
    expect(release).not.toContain("localhost:1420");
    expect(release).not.toContain("localhost:1421");
    expect(transport).toContain("release_blocks_localhost");
    expect(transport).toContain("service_role_forbidden");
    expect(transport).toContain("debug_blocks_production");
    expect(commands).toContain("seed_dev_data is debug-only");
    expect(app).toContain("health?.debug");
  });
});
