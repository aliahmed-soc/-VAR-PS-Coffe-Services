import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const en = JSON.parse(readFileSync("src/i18n/en/common.json", "utf8")) as Record<string, string>;
const ar = JSON.parse(readFileSync("src/i18n/ar/common.json", "utf8")) as Record<string, string>;
const i18n = readFileSync("src/i18n/index.ts", "utf8");

describe("Arabic / RTL", () => {
  it("keeps English and Arabic catalogs on the same keys", () => {
    expect(Object.keys(ar).sort()).toEqual(Object.keys(en).sort());
  });

  it("uses Arabic script for the Arabic catalog", () => {
    expect(ar.appName).toMatch(/[\u0600-\u06FF]/);
    expect(ar.pay).toMatch(/[\u0600-\u06FF]/);
    expect(ar.restartHint).toMatch(/[\u0600-\u06FF]/);
  });

  it("flips document direction when the language is Arabic", () => {
    expect(i18n).toContain('document.documentElement.dir = rtl ? "rtl" : "ltr"');
    expect(i18n).toContain('document.documentElement.lang = rtl ? "ar" : "en"');
  });
});
