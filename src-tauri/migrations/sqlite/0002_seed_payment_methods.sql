INSERT INTO payment_methods (id, code, name, name_ar, is_active, requires_reference, sort_order)
VALUES
  ('11111111-1111-1111-1111-111111111111', 'cash', 'Cash', 'نقدي', 1, 0, 1),
  ('22222222-2222-2222-2222-222222222222', 'card', 'Card', 'بطاقة', 0, 1, 2),
  ('33333333-3333-3333-3333-333333333333', 'other', 'Other', 'أخرى', 0, 1, 3);
