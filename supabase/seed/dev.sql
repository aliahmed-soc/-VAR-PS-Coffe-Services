-- Local development seed. Do not run against production.

INSERT INTO branches (id, code, name, timezone, currency_code, is_active)
VALUES
  ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1', 'B1', 'Branch 1', 'Africa/Cairo', 'EGP', true),
  ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2', 'B2', 'Branch 2', 'Africa/Cairo', 'EGP', true)
ON CONFLICT (id) DO NOTHING;
