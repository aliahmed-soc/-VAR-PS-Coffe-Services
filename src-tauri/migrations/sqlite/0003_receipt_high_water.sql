-- Receipt numbers are allocated from the highest one this device has already
-- issued, read out of its own orders table. A restored backup holds fewer orders
-- than the cloud, so the counter rewinds and re-issues a number the cloud still
-- stores under a different order: the cloud's global unique index then refuses
-- order.paid with a 409 forever, leaving the sale paid on the till and unpaid in
-- the cloud. Reconciliation records what the cloud has already handed out here,
-- and the allocator never issues at or below it.
CREATE TABLE IF NOT EXISTS receipt_high_water (
    branch_id TEXT NOT NULL,
    prefix TEXT NOT NULL,
    last_sequence INTEGER NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (branch_id, prefix)
);
