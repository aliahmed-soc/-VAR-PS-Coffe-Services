import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { applyDomainEventSql } from "./apply-event-sql";

const sqlite = readFileSync("src-tauri/migrations/sqlite/0001_init.sql", "utf8");
const postgres = readFileSync("supabase/migrations/20260829000100_init.sql", "utf8");
const apply = applyDomainEventSql();
const domain = readFileSync("src-tauri/crates/cafe-domain/src/pricing.rs", "utf8");
const schema = JSON.parse(readFileSync("contracts/schema-contract.json", "utf8"));

function walk(dir: string, acc: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      walk(path, acc);
    } else if (/\.(ts|tsx|js)$/.test(name)) {
      acc.push(path);
    }
  }
  return acc;
}

describe("MVP linear-only gaming pricing", () => {
  it("freezes the integer floor formula", () => {
    expect(schema.pricing_mvp.rule_type).toBe("linear");
    expect(schema.pricing_mvp.stepped).toBe(false);
    expect(schema.pricing_mvp.billing_increment).toBe(false);
    expect(schema.pricing_mvp.formula).toBe(
      "floor(rate_minor_per_hour * duration_seconds / 3600)",
    );
    expect(domain).toContain("rate_minor_per_hour.saturating_mul(duration_seconds) / 3600");
    expect(domain).toContain("stepped pricing is not supported in MVP");
    expect(domain).not.toContain("fn stepped_charge_minor");
    expect(domain).not.toMatch(/seconds\s*\+=\s*inc/);
  });

  it("constrains SQLite and PostgreSQL to linear rules", () => {
    expect(sqlite).toContain("CHECK (rule_type = 'linear')");
    expect(postgres).toContain("CHECK (rule_type = 'linear')");
    expect(sqlite).toContain("rate_minor_per_hour INTEGER NOT NULL CHECK (rate_minor_per_hour >= 0)");
    expect(postgres).toContain("rate_minor_per_hour bigint NOT NULL CHECK (rate_minor_per_hour >= 0)");
    expect(apply).toContain("mvp_linear_pricing_required");
    expect(existsSync("supabase/tests/pricing/linear_mvp.sql")).toBe(true);
  });

  it("keeps pricing math out of the webview", () => {
    for (const file of walk("src")) {
      const text = readFileSync(file, "utf8");
      expect(text, file).not.toMatch(/billing_increment/);
      expect(text, file).not.toMatch(/stepped_charge|base_charge_minor|step_charge_minor/);
      expect(text, file).not.toMatch(/rate_minor_per_hour\s*\*/);
      expect(text, file).not.toMatch(/\/\s*3600/);
      expect(text, file).not.toContain("linear_charge_minor");
    }
  });
});
