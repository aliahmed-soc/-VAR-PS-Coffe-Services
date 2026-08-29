import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

type Station = {
  id: string;
  code: string;
  display_name: string;
  session_id: string | null;
  session_status: string | null;
  started_at: string | null;
  order_id: string | null;
};

type Health = {
  ok: boolean;
  device_id: string;
  session?: {
    display_name: string;
    branch_id: string;
    role: string;
    offline: boolean;
    user_id: string;
  } | null;
  sync: { label: string; pending: number };
};

function formatMoney(minor: number) {
  const sign = minor < 0 ? "-" : "";
  const abs = Math.abs(minor);
  const major = Math.trunc(abs / 100);
  const frac = String(abs % 100).padStart(2, "0");
  return `${sign}${major}.${frac} EGP`;
}

export default function App() {
  const { t, i18n } = useTranslation();
  const [health, setHealth] = useState<Health | null>(null);
  const [stations, setStations] = useState<Station[]>([]);
  const [charges, setCharges] = useState<Record<string, { duration_seconds: number; charge_minor: number }>>({});
  const [email, setEmail] = useState("admin@local");
  const [password, setPassword] = useState("");
  const [pin, setPin] = useState("1357");
  const [userId, setUserId] = useState("u-c1");
  const [error, setError] = useState<string | null>(null);
  const [tendered, setTendered] = useState("200");
  const [activeOrder, setActiveOrder] = useState<string | null>(null);

  async function refresh() {
    const h = await invoke<Health>("app_health");
    setHealth(h);
    if (h.session) {
      const list = await invoke<Station[]>("list_stations");
      setStations(list);
    }
  }

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      stations
        .filter((s) => s.session_id && s.session_status === "active")
        .forEach((s) => {
          invoke<{ duration_seconds: number; charge_minor: number }>("live_charge", {
            sessionId: s.session_id,
          })
            .then((c) => setCharges((prev) => ({ ...prev, [s.session_id!]: c })))
            .catch(() => undefined);
        });
    }, 1000);
    return () => window.clearInterval(timer);
  }, [stations]);

  const signedIn = Boolean(health?.session);

  async function run(fn: () => Promise<unknown>) {
    setError(null);
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  const header = useMemo(
    () => (
      <header className="flex items-center justify-between border-b border-slate-700 px-4 py-3">
        <div>
          <div className="text-lg font-semibold">{t("appName")}</div>
          <div className="text-sm text-slate-300">
            {health?.session
              ? `${health.session.branch_id} · ${health.session.display_name} · ${health.sync.label}`
              : health?.sync.label}
          </div>
        </div>
        <div className="flex gap-2">
          <button className="rounded bg-slate-800 px-3 py-1" onClick={() => i18n.changeLanguage("en")}>
            EN
          </button>
          <button className="rounded bg-slate-800 px-3 py-1" onClick={() => i18n.changeLanguage("ar")}>
            AR
          </button>
          {signedIn ? (
            <button className="rounded bg-slate-800 px-3 py-1" onClick={() => run(() => invoke("logout"))}>
              {t("logout")}
            </button>
          ) : null}
        </div>
      </header>
    ),
    [health, i18n, signedIn, t],
  );

  if (!signedIn) {
    return (
      <div className="min-h-full">
        {header}
        <main className="mx-auto mt-10 max-w-md space-y-4 rounded border border-slate-700 p-6">
          {error ? <p className="text-red-400">{error}</p> : null}
          <button className="w-full rounded bg-slate-700 py-2" onClick={() => run(() => invoke("seed_dev_data"))}>
            {t("seedDev")}
          </button>
          <input className="w-full rounded bg-slate-900 p-2" value={email} onChange={(e) => setEmail(e.target.value)} placeholder={t("email")} />
          <input className="w-full rounded bg-slate-900 p-2" type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder={t("password")} />
          <input className="w-full rounded bg-slate-900 p-2" value={pin} onChange={(e) => setPin(e.target.value)} placeholder={t("pin")} />
          <button
            className="w-full rounded bg-emerald-700 py-2"
            onClick={() => run(() => invoke("login_online", { email, password, pin }))}
          >
            {t("onlineLogin")}
          </button>
          <input className="w-full rounded bg-slate-900 p-2" value={userId} onChange={(e) => setUserId(e.target.value)} placeholder="user id" />
          <button
            className="w-full rounded bg-sky-800 py-2"
            onClick={() => run(() => invoke("unlock_offline", { userId, pin }))}
          >
            {t("offlineUnlock")}
          </button>
        </main>
      </div>
    );
  }

  return (
    <div className="min-h-full">
      {header}
      <main className="grid gap-4 p-4 md:grid-cols-2 xl:grid-cols-3">
        {error ? <p className="col-span-full text-red-400">{error}</p> : null}
        {stations.map((s) => {
          const live = s.session_id ? charges[s.session_id] : undefined;
          const playing = s.session_status === "active";
          return (
            <section key={s.id} className="rounded border border-slate-700 p-4">
              <div className="flex justify-between">
                <h2 className="text-xl font-semibold">{s.display_name}</h2>
                <span>{playing ? t("playing") : t("available")}</span>
              </div>
              {playing && live ? (
                <p className="mt-2 text-2xl tabular-nums">
                  {new Date(live.duration_seconds * 1000).toISOString().substring(11, 19)} · {formatMoney(live.charge_minor)}
                </p>
              ) : null}
              <div className="mt-4 flex flex-wrap gap-2">
                {!playing ? (
                  <button className="rounded bg-emerald-700 px-3 py-2" onClick={() => run(() => invoke("start_session", { stationId: s.id }))}>
                    {t("start")}
                  </button>
                ) : (
                  <button
                    className="rounded bg-amber-700 px-3 py-2"
                    onClick={() =>
                      run(async () => {
                        const stopped = await invoke<{ session_id: string }>("stop_session", { sessionId: s.session_id });
                        setActiveOrder(s.order_id);
                        return stopped;
                      })
                    }
                  >
                    {t("stop")}
                  </button>
                )}
                {s.order_id ? (
                  <>
                    <button
                      className="rounded bg-slate-700 px-3 py-2"
                      onClick={() =>
                        run(async () => {
                          await invoke("add_order_item", { orderId: s.order_id, productId: "p-coke", quantity: 1 });
                          setActiveOrder(s.order_id);
                        })
                      }
                    >
                      {t("addProduct")}
                    </button>
                    <button
                      className="rounded bg-emerald-800 px-3 py-2"
                      onClick={() =>
                        run(() =>
                          invoke("take_cash", {
                            orderId: s.order_id,
                            tenderedMinor: Math.round(Number(tendered) * 100),
                          }),
                        )
                      }
                    >
                      {t("pay")}
                    </button>
                  </>
                ) : null}
              </div>
            </section>
          );
        })}
        <section className="rounded border border-slate-700 p-4">
          <label className="block text-sm text-slate-300">{t("tendered")}</label>
          <input className="mt-1 w-full rounded bg-slate-900 p-2" value={tendered} onChange={(e) => setTendered(e.target.value)} />
          {activeOrder ? (
            <button
              className="mt-3 rounded bg-red-900 px-3 py-2"
              onClick={() => run(() => invoke("reverse_payment", { orderId: activeOrder, reason: "cashier correction" }))}
            >
              {t("reverse")}
            </button>
          ) : null}
        </section>
      </main>
    </div>
  );
}
