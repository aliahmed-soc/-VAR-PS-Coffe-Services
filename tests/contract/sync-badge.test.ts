import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const app = readFileSync("src/App.tsx", "utf8");

// Physical UAT found the badge frozen on "OFFLINE • 2 UNSYNCED" while the backend
// reported ONLINE • SYNCED and the hosted database already held the order: health
// was only re-read inside run(), so an idle till kept advertising unsynced sales
// that had in fact converged. Cashiers use that badge to decide whether it is safe
// to close the shop, so it has to follow the background worker without being poked.
describe("connectivity / sync badge", () => {
  it("re-reads app_health on an interval, not only after operator actions", () => {
    const poll = app.match(
      /setInterval\(\s*\(\)\s*=>\s*\{\s*invoke<Health>\("app_health"\)[\s\S]*?\}\s*,\s*HEALTH_POLL_MS\s*\)/,
    );
    expect(poll, "no app_health poll driven by HEALTH_POLL_MS").not.toBeNull();
  });

  it("polls often enough that a converged sync shows up promptly", () => {
    const declared = app.match(/export const HEALTH_POLL_MS = (\d+);/);
    expect(declared, "HEALTH_POLL_MS is not declared").not.toBeNull();
    expect(Number(declared![1])).toBeGreaterThan(0);
    expect(Number(declared![1])).toBeLessThanOrEqual(10_000);
  });

  it("clears the poll when the view unmounts", () => {
    expect(app).toMatch(/return \(\) => window\.clearInterval\(timer\);/);
  });

  it("still labels the session offline from the session, not from the poll", () => {
    expect(app).toContain("health.session.offline ? t(\"offline\") : health.sync.label");
  });
});
