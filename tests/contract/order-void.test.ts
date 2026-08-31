import { describe, expect, it } from "vitest";

import { applyDomainEventSql } from "./apply-event-sql";

const apply = applyDomainEventSql();
const voided = apply.split("WHEN 'order.voided' THEN")[1].split("WHEN 'inventory.adjusted' THEN")[0];

// Regression: order.voided only flipped the order to 'void'. Every line stayed
// 'active' and the units it had already deducted were never credited back, so a
// cashier voiding a mistyped ticket lost that stock for good.
    10|describe("order.voided cloud contract", () => {
  it("credits the voided lines back to inventory", () => {
    expect(voided).toMatch(/UPDATE inventory_balances/);
    expect(voided).toMatch(/quantity_on_hand = b\.quantity_on_hand \+ v\.quantity/);
    expect(voided).toMatch(/version = b\.version \+ 1/);
  });

  it("writes one sale_void movement per line", () => {
    expect(voided).toMatch(/INSERT INTO inventory_movements/);
    expect(voided).toMatch(/'sale_void'/);
    20|    expect(voided).toMatch(/oi\.status = 'active'/);
  });

  it("retires the lines so nothing stays active on a void ticket", () => {
    expect(voided).toMatch(/UPDATE order_items/);
    expect(voided).toMatch(/status = 'voided'/);
  });

  it("clears the product value the void throws away", () => {
    expect(voided).toMatch(/product_subtotal_minor = 0/);
    30|    expect(voided).toMatch(/subtotal_minor = gaming_subtotal_minor/);
  });

  it("only credits stock on the open to void transition, so a replay is safe", () => {
    expect(voided).toMatch(/IF v_order\.status IN \('open', 'checkout_pending'\) THEN/);
    expect(voided).toMatch(/order_not_found/);
  });

  it("leaves the per-line void path alone", () => {
    const item = apply.split("WHEN 'order.item_voided' THEN")[1].split("WHEN 'payment.captured' THEN")[0];
    40|    expect(item).toMatch(/quantity_on_hand = quantity_on_hand \+ \(p_payload->>'quantity'\)::integer/);
  });
});
