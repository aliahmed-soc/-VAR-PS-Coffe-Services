-- Drop hosted-test helpers if a previous script left them.
-- Safe on production: functions are test-only. Do not add to scripts/run-pg-tests.sh.

DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public' AND p.proname = 'hosted_cleanup_rls_matrix'
  ) THEN
    PERFORM public.hosted_cleanup_rls_matrix();
  END IF;
  IF EXISTS (
    SELECT 1 FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'public' AND p.proname = 'hosted_cleanup_event_acceptance'
  ) THEN
    PERFORM public.hosted_cleanup_event_acceptance();
  END IF;
END $$;

DROP FUNCTION IF EXISTS public.test_hosted_event_acceptance();
DROP FUNCTION IF EXISTS public.hosted_cleanup_event_acceptance();
DROP FUNCTION IF EXISTS public.test_hosted_rls_matrix();
DROP FUNCTION IF EXISTS public.hosted_cleanup_rls_matrix();
DROP FUNCTION IF EXISTS public.test_rls_expect_deny(text, text);
DROP FUNCTION IF EXISTS public.hosted_delete_auth_user(uuid);
DROP FUNCTION IF EXISTS public.hosted_set_jwt(uuid);
DROP FUNCTION IF EXISTS public.hosted_insert_auth_user(uuid, text);
DROP FUNCTION IF EXISTS public.hosted_auth_user_placeholder(text, text);
