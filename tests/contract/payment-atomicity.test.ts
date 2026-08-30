import { describe, expect, it } from "vitest";

import { applyDomainEventSql } from "./apply-event-sql";

const apply = applyDomainEventSql();

describe("payment cloud atomicity", () => {
  it("does not close a sale on payment.captured and order.paid together", () => {
    expect(apply).not.toMatch(/WHEN 'order\.paid',\s*'payment\.captured'/);
    expect(apply).toMatch(/WHEN 'payment\.captured' THEN/);
    expect(apply).toMatch(/WHEN 'order\.paid' THEN/);
  });

  it("keeps payment.captured from writing paid financial state", () => {
    const captured = apply.split("WHEN 'payment.captured' THEN")[1].split("WHEN 'order.paid' THEN")[0];
    expect(captured).not.toMatch(/status = 'paid'/);
    expect(captured).not.toMatch(/receipt_snapshot/);
    expect(captured).not.toMatch(/INSERT INTO payments/);
    expect(captured).toMatch(/Sequencing\/audit only/);
  });

  it("finalizes the sale only on order.paid", () => {
    const paid = apply.split("WHEN 'order.paid' THEN")[1].split("WHEN 'payment.reversed' THEN")[0];
    expect(paid).toMatch(/status = 'paid'/);
    expect(paid).toMatch(/INSERT INTO payments/);
    expect(paid).toMatch(/Never derive tax from tax_rate_bps/);
    expect(paid).toMatch(/receipt_snapshot/);
    expect(paid).toMatch(/amount_mismatch/);
    expect(paid).toMatch(/branch_mismatch/);
    expect(paid).toMatch(/checkout_pending/);
  });

  it("rejects reverse before canonical order.paid", () => {
    const reversed = apply.split("WHEN 'payment.reversed' THEN")[1].split("WHEN 'order.voided' THEN")[0];
    expect(reversed).toMatch(/order_not_paid/);
    expect(reversed).toMatch(/v_order\.status <> 'paid'/);
  });

  // Regression: order.paid refuses to overwrite a stored receipt, so a reversal
  // that left receipt_snapshot behind made every repayment unsyncable.
  it("retires the receipt on reversal while keeping order.paid immutable", () => {
    const reversed = apply.split("WHEN 'payment.reversed' THEN")[1].split("WHEN 'order.voided' THEN")[0];
    expect(reversed).toMatch(/receipt_number = NULL/);
    expect(reversed).toMatch(/receipt_snapshot = NULL/);
    expect(reversed).toMatch(/status = 'checkout_pending'/);
    expect(reversed).toMatch(/'reversal'/);

    const paid = apply.split("WHEN 'order.paid' THEN")[1].split("WHEN 'payment.reversed' THEN")[0];
    expect(paid).toMatch(/receipt_already_stored/);
  });
});
