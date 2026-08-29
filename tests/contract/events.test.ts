import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const catalog = JSON.parse(readFileSync("contracts/events.json", "utf8"));
const schema = JSON.parse(readFileSync("contracts/schema-contract.json", "utf8"));

describe("event catalog", () => {
  it("is frozen", () => {
    expect(catalog.frozen).toBe(true);
    expect(schema.frozen).toBe(true);
  });

  it("has required MVP events", () => {
    const types = catalog.events.map((e: { type: string }) => e.type);
    for (const required of [
      "session.started",
      "session.stopped",
      "order.item_added",
      "order.paid",
      "payment.reversed",
      "inventory.adjusted",
    ]) {
      expect(types).toContain(required);
    }
  });

  it("every event lists required payload fields", () => {
    for (const event of catalog.events) {
      expect(event.payload_required.length).toBeGreaterThan(0);
    }
  });

  it("schema contract lists operational tables on both sides", () => {
    expect(schema.operational_tables).toContain("orders");
    expect(schema.operational_tables).toContain("payments");
    expect(schema.sqlite_only).toContain("sync_outbox");
    expect(schema.postgres_only).toContain("sync_receipts");
  });
});
