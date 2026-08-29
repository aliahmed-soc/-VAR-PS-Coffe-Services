import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const rls = readFileSync("supabase/migrations/20260829000200_rls.sql", "utf8");
const matrix = readFileSync("supabase/tests/rls/branch_isolation.sql", "utf8");
const harness = readFileSync("scripts/run-pg-tests.sh", "utf8");

const MONEY_TABLES = [
  "orders",
  "order_items",
  "payments",
  "inventory_balances",
  "inventory_movements",
  "gaming_sessions",
];

describe("RLS allow/deny matrix", () => {
  it("is executed by the Postgres CI harness", () => {
    expect(harness).toContain("supabase/tests/rls/branch_isolation.sql");
  });

  it("keeps money tables SELECT-only for clients", () => {
    for (const table of MONEY_TABLES) {
      expect(rls).toContain(`REVOKE ALL ON TABLE ${table} FROM anon, authenticated`);
      expect(rls).toContain(`GRANT SELECT ON TABLE ${table} TO authenticated`);
      expect(rls).not.toMatch(new RegExp(`CREATE POLICY \\w+ ON ${table}\\s+FOR INSERT`));
      expect(rls).not.toMatch(new RegExp(`CREATE POLICY \\w+ ON ${table}\\s+FOR ALL`));
    }
  });

  it("covers cashier isolation, write denials, admin catalog, and anon", () => {
    expect(matrix).toContain("cashier B1 must not see B2 orders");
    expect(matrix).toContain("cashier B1 must not see B2 payments");
    expect(matrix).toContain("cashier B1 must not see B2 inventory");
    expect(matrix).toContain("cashier B2 must not see B1 orders");
    expect(matrix).toContain("inactive cashier must not see B1 orders");
    expect(matrix).toContain("cashier direct order insert");
    expect(matrix).toContain("cashier catalog insert");
    expect(matrix).toContain("admin direct order insert");
    expect(matrix).toContain("admin must see both branches");
    expect(matrix).toContain("anon orders");
    expect(matrix).toContain("SECURITY DEFINER apply on own branch must work");
    expect(matrix).toContain("branch_forbidden");
  });
});
