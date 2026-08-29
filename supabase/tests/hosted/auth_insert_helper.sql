-- Portable hosted Auth insert + JWT helpers.
-- Hosted auth.users has many columns; local stub has only id/email.
-- Do not add this file to scripts/run-pg-tests.sh.

CREATE OR REPLACE FUNCTION public.hosted_auth_user_placeholder(p_col text, p_typ text)
RETURNS text
LANGUAGE plpgsql
AS $$
BEGIN
  IF p_col = 'instance_id' THEN
    RETURN '''00000000-0000-0000-0000-000000000000''::uuid';
  END IF;
  IF p_col IN ('aud', 'role') THEN
    RETURN '''authenticated''';
  END IF;
  IF p_col = 'encrypted_password' THEN
    RETURN quote_literal(encode(gen_random_bytes(16), 'hex'));
  END IF;
  IF p_col IN ('raw_app_meta_data', 'raw_user_meta_data') OR p_typ IN ('jsonb', 'json') THEN
    RETURN '''{}''::jsonb';
  END IF;
  IF p_typ LIKE 'timestamp%' THEN
    RETURN 'now()';
  END IF;
  IF p_typ IN ('boolean', 'bool') THEN
    RETURN 'false';
  END IF;
  IF p_typ LIKE 'int%' OR p_typ LIKE 'bigint%' OR p_typ LIKE 'smallint%' OR p_typ LIKE 'numeric%' THEN
    RETURN '0';
  END IF;
  IF p_typ LIKE 'uuid%' THEN
    RETURN '''00000000-0000-0000-0000-000000000000''::uuid';
  END IF;
  -- Unique-ish tokens so empty-string UNIQUE constraints do not collide.
  RETURN quote_literal('ht-' || replace(gen_random_uuid()::text, '-', ''));
END;
$$;

CREATE OR REPLACE FUNCTION public.hosted_insert_auth_user(p_id uuid, p_email text)
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
  cols text[] := ARRAY[]::text[];
  vals text[] := ARRAY[]::text[];
  rec record;
  sql text;
BEGIN
  IF to_regclass('auth.users') IS NULL THEN
    RAISE EXCEPTION 'auth.users missing';
  END IF;
  IF EXISTS (SELECT 1 FROM auth.users WHERE id = p_id) THEN
    RETURN;
  END IF;

  FOR rec IN
    SELECT a.attname AS column_name,
           format_type(a.atttypid, a.atttypmod) AS typ,
           a.attnotnull AS not_null,
           pg_get_expr(ad.adbin, ad.adrelid) AS col_default,
           a.attgenerated AS generated,
           a.attidentity AS identity
    FROM pg_attribute a
    JOIN pg_class c ON c.oid = a.attrelid
    JOIN pg_namespace n ON n.oid = c.relnamespace
    LEFT JOIN pg_attrdef ad ON ad.adrelid = a.attrelid AND ad.adnum = a.attnum
    WHERE n.nspname = 'auth'
      AND c.relname = 'users'
      AND a.attnum > 0
      AND NOT a.attisdropped
    ORDER BY a.attnum
  LOOP
    IF rec.generated <> '' OR rec.identity <> '' THEN
      CONTINUE;
    END IF;
    IF rec.column_name = 'id' THEN
      cols := cols || quote_ident(rec.column_name);
      vals := vals || format('%L::uuid', p_id);
    ELSIF rec.column_name = 'email' THEN
      cols := cols || quote_ident(rec.column_name);
      vals := vals || format('%L', p_email);
    ELSIF rec.col_default IS NOT NULL THEN
      CONTINUE;
    ELSIF rec.not_null THEN
      cols := cols || quote_ident(rec.column_name);
      vals := vals || public.hosted_auth_user_placeholder(rec.column_name, rec.typ);
    END IF;
  END LOOP;

  sql := format(
    'INSERT INTO auth.users (%s) VALUES (%s)',
    array_to_string(cols, ', '),
    array_to_string(vals, ', ')
  );
  EXECUTE sql;
END;
$$;

CREATE OR REPLACE FUNCTION public.hosted_set_jwt(p_uid uuid)
RETURNS void
LANGUAGE plpgsql
AS $$
BEGIN
  PERFORM set_config('request.jwt.claim.sub', COALESCE(p_uid::text, ''), true);
  IF p_uid IS NULL THEN
    PERFORM set_config('request.jwt.claims', '{}', true);
  ELSE
    PERFORM set_config(
      'request.jwt.claims',
      jsonb_build_object('sub', p_uid::text, 'role', 'authenticated')::text,
      true
    );
  END IF;
END;
$$;

CREATE OR REPLACE FUNCTION public.hosted_delete_auth_user(p_id uuid)
RETURNS void
LANGUAGE plpgsql
AS $$
BEGIN
  IF to_regclass('auth.identities') IS NOT NULL THEN
    EXECUTE 'DELETE FROM auth.identities WHERE user_id = $1' USING p_id;
  END IF;
  IF to_regclass('auth.users') IS NOT NULL THEN
    DELETE FROM auth.users WHERE id = p_id;
  END IF;
END;
$$;
