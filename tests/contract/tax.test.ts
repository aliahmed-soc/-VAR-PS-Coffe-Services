import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const sqlite = readFileSync("src-tauri/migrations/sqlite/0001_init.sql", "utf8");
const postgres = readFileSync("supabase/migrations/20260829000100_init.sql", "utf8");
const apply = readFileSync("supabase/migrations/20260829000300_apply_domain_event.sql", "utf8");
const schema = JSON.parse(readFileSync("contracts/schema-contract.json", "utf8"));
const catalog = JSON.parse(readFileSync("contracts/events.json", "utf8"));

const MONEY = [
  "product_subtotal_minor",
  "gaming_subtotal_minor",
  "subtotal_minor",
  "discount_minor",
  "tax_minor",
  "tax_rate_bps",
  "total_minor",
];

describe("P1-4 tax-ready snapshots", () => {
  it("defaults tax to zero in the frozen contract", () => {
    expect(schema.tax_mvp.tax_minor).toBe(0);
    expect(schema.tax_mvp.tax_rate_bps).toBe(0);
    expect(schema.tax_mvp.vat_ui).toBe(false);
    expect(schema.tax_mvp.auto_vat).toBe(false);
  });

  it("keeps the same money columns on SQLite and PostgreSQL", () => {
    for (const col of MONEY) {
      expect(sqlite).toContain(col);
      expect(postgres).toContain(col);
      expect(schema.order_money_columns).toContain(col);
    }
    expect(sqlite).toContain("total_minor = subtotal_minor + tax_minor - discount_minor");
    expect(postgres).toContain("total_minor = subtotal_minor + tax_minor - discount_minor");
    expect(sqlite).toContain("tax_minor >= 0");
    expect(postgres).toContain("tax_minor >= 0");
  });

  it("rejects negative tax at both databases", () => {
    expect(sqlite).toContain("CHECK (tax_minor >= 0)");
    expect(postgres).toContain("CHECK (tax_minor >= 0)");
    expect(sqlite).toContain("CHECK (tax_rate_bps >= 0)");
    expect(postgres).toContain("CHECK (tax_rate_bps >= 0)");
  });

  it("makes tax immutable after payment", () => {
    expect(sqlite).toContain("paid_tax_immutable");
    expect(postgres).toContain("paid_tax_immutable");
  });

  it("replays receipt-snapshot tax and does not recalculate from rate", () => {
    expect(apply).toContain("Never derive tax from tax_rate_bps");
    expect(apply).toContain("receipt_snapshot");
    expect(apply).toContain("v_snap->>'tax_minor'");
    expect(apply).not.toMatch(/tax_minor\s*=\s*\([^)]*tax_rate_bps/);
  });

  it("freezes tax fields on order.paid", () => {
    const paid = catalog.events.find((e: { type: string }) => e.type === "order.paid");
    for (const key of ["tax_minor", "tax_rate_bps", "subtotal_minor", "discount_minor", "receipt_snapshot"]) {
      expect(paid.payload_required).toContain(key);
    }
  });
});
