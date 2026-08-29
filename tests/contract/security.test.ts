import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

function walk(dir: string, acc: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      walk(path, acc);
    } else if (/\.(ts|tsx|js)$/.test(name)) {
      acc.push(path);
    }
  }
  return acc;
}

describe("webview security", () => {
  it("does not import supabase-js or a Supabase client in the UI", () => {
    for (const file of walk("src")) {
      const text = readFileSync(file, "utf8");
      expect(text, file).not.toMatch(/@supabase\/supabase-js/);
      expect(text, file).not.toMatch(/createClient\(/);
      expect(text, file).not.toMatch(/supabase\.co/);
      expect(text, file).not.toMatch(/sb_secret_/);
      expect(text, file).not.toMatch(/sb_publishable_/);
      expect(text, file).not.toMatch(/localStorage\.(get|set)Item\([^)]*token/i);
      expect(text, file).not.toMatch(/sessionStorage/);
    }
  });
});
