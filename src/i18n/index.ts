import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./en/common.json";
import ar from "./ar/common.json";

export async function initI18n() {
  await i18n.use(initReactI18next).init({
    resources: {
      en: { common: en },
      ar: { common: ar },
    },
    lng: localStorage.getItem("psc-lang") === "ar" ? "ar" : "en",
    fallbackLng: "en",
    ns: ["common"],
    defaultNS: "common",
    interpolation: { escapeValue: false },
  });
  applyDir(i18n.language);
  i18n.on("languageChanged", applyDir);
}

function applyDir(lng: string) {
  const rtl = lng.startsWith("ar");
  document.documentElement.lang = rtl ? "ar" : "en";
  document.documentElement.dir = rtl ? "rtl" : "ltr";
}

export default i18n;
