import { readFileSync, readdirSync } from "node:fs";

const DIR = "supabase/migrations";
const DEFINITION = "CREATE OR REPLACE FUNCTION public.apply_domain_event";

/**
 * Source of the `apply_domain_event` definition that is actually live, i.e. the
 * last migration that redefines it. Pinning a contract to the original
 * migration would let a later migration change the RPC unnoticed.
 */
export function applyDomainEventSql(): string {
  const file = readdirSync(DIR)
    .filter((name) => name.endsWith(".sql"))
    .sort()
    .reverse()
    .find((name) => readFileSync(`${DIR}/${name}`, "utf8").includes(DEFINITION));
  if (!file) throw new Error(`no migration in ${DIR} defines apply_domain_event`);
  return readFileSync(`${DIR}/${file}`, "utf8");
}
