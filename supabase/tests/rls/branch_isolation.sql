-- Manual / pgTAP-style expectations. Run against local Supabase after seed.
-- Branch 1 cashier must not read Branch 2 orders.
-- Anonymous must be denied.
-- Admin may read both branches.
-- This file documents the required matrix; automated SQL tests run when supabase test is available.

-- EXPECT: SET ROLE authenticated + jwt branch 1 cashier
--   SELECT count(*) FROM orders WHERE branch_id = 'b2' => 0 visible via RLS
-- EXPECT: anon
--   SELECT * FROM orders => permission denied
