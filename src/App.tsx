import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

type Tab = "stations" | "pos" | "sales" | "ops";

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
  debug?: boolean;
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

type Product = {
  id: string;
  name: string;
  name_ar: string | null;
  sell_price_minor: number;
  quantity_on_hand: number;
};

type OrderItem = {
  id: string;
  name: string;
  quantity: number;
  unit_price_minor: number;
  line_total_minor: number;
  status: string;
};

type Order = {
  id: string;
  status: string;
  order_type: string;
  product_subtotal_minor: number;
  gaming_subtotal_minor: number;
  total_minor: number;
  amount_paid_minor: number;
  change_minor: number;
  receipt_number: string | null;
  receipt_snapshot: string | null;
  items: OrderItem[];
};

type SaleRow = {
  id: string;
  status: string;
  total_minor: number;
  receipt_number: string | null;
  closed_at: string | null;
};

type Report = {
  gaming_revenue_minor: number;
  product_revenue_minor: number;
  sales_revenue_minor: number;
  paid_orders: number;
};

type BackupRow = { path: string; name: string; bytes: number };

function formatMoney(minor: number) {
  const sign = minor < 0 ? "-" : "";
  const abs = Math.abs(minor);
  return `${sign}${Math.trunc(abs / 100)}.${String(abs % 100).padStart(2, "0")} EGP`;
}

function formatClock(seconds: number) {
  return new Date(seconds * 1000).toISOString().substring(11, 19);
}

export default function App() {
  const { t, i18n } = useTranslation();
  const [tab, setTab] = useState<Tab>("stations");
  const [health, setHealth] = useState<Health | null>(null);
  const [stations, setStations] = useState<Station[]>([]);
  const [products, setProducts] = useState<Product[]>([]);
  const [sales, setSales] = useState<SaleRow[]>([]);
  const [charges, setCharges] = useState<Record<string, { duration_seconds: number; charge_minor: number }>>({});
  const [order, setOrder] = useState<Order | null>(null);
  const [report, setReport] = useState<Report | null>(null);
  const [backups, setBackups] = useState<BackupRow[]>([]);
  const [restoreHint, setRestoreHint] = useState<string | null>(null);
  const [email, setEmail] = useState("admin@local");
  const [password, setPassword] = useState("");
  const [pin, setPin] = useState("1357");
  const [userId, setUserId] = useState("u-c1");
  const [error, setError] = useState<string | null>(null);
  const [tendered, setTendered] = useState("50");

  const signedIn = Boolean(health?.session);
  const isAdmin = health?.session?.role === "admin";

  const loadOrder = useCallback(async (orderId: string | null) => {
    if (!orderId) {
      setOrder(null);
      return;
    }
    const next = await invoke<Order>("get_order", { orderId });
    setOrder(next);
  }, []);

  const refresh = useCallback(async () => {
    const h = await invoke<Health>("app_health");
    setHealth(h);
    if (!h.session) {
      setStations([]);
      setProducts([]);
      setSales([]);
      return;
    }
    const [list, catalog, tickets] = await Promise.all([
      invoke<Station[]>("list_stations"),
      invoke<Product[]>("list_products"),
      invoke<SaleRow[]>("list_sales"),
    ]);
    setStations(list);
    setProducts(catalog);
    setSales(tickets);
    if (order) {
      await loadOrder(order.id);
    }
  }, [loadOrder, order]);

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

  async function run(fn: () => Promise<unknown>) {
    setError(null);
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  function setLang(lng: "en" | "ar") {
    i18n.changeLanguage(lng);
    localStorage.setItem("psc-lang", lng);
  }

  useEffect(() => {
    const saved = localStorage.getItem("psc-lang");
    if (saved === "ar" || saved === "en") {
      i18n.changeLanguage(saved);
    }
  }, [i18n]);

  const header = (
    <header className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-700 px-4 py-3">
      <div>
        <div className="text-lg font-semibold">{t("appName")}</div>
        <div className="text-sm text-slate-300">
          {health?.session
            ? `${health.session.branch_id} · ${health.session.display_name} · ${health.session.offline ? t("offline") : health.sync.label}`
            : health?.sync.label}
        </div>
      </div>
      <div className="flex flex-wrap gap-2">
        <button className="rounded bg-slate-800 px-3 py-1" onClick={() => setLang("en")}>
          EN
        </button>
        <button className="rounded bg-slate-800 px-3 py-1" onClick={() => setLang("ar")}>
          AR
        </button>
        {signedIn ? (
          <button className="rounded bg-slate-800 px-3 py-1" onClick={() => run(() => invoke("logout"))}>
            {t("logout")}
          </button>
        ) : null}
      </div>
    </header>
  );

  if (!signedIn) {
    return (
      <div className="min-h-full">
        {header}
        <main className="mx-auto mt-10 max-w-md space-y-4 rounded border border-slate-700 p-6">
          {error ? <p className="text-red-400">{error}</p> : null}
          {health?.debug ? (
            <button className="w-full rounded bg-slate-700 py-2" onClick={() => run(() => invoke("seed_dev_data"))}>
              {t("seedDev")}
            </button>
          ) : null}
          <input className="w-full rounded bg-slate-900 p-2" value={email} onChange={(e) => setEmail(e.target.value)} placeholder={t("email")} />
          <input className="w-full rounded bg-slate-900 p-2" type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder={t("password")} />
          <input className="w-full rounded bg-slate-900 p-2" value={pin} onChange={(e) => setPin(e.target.value)} placeholder={t("pin")} />
          <button className="w-full rounded bg-emerald-700 py-2" onClick={() => run(() => invoke("login_online", { email, password, pin }))}>
            {t("onlineLogin")}
          </button>
          <input className="w-full rounded bg-slate-900 p-2" value={userId} onChange={(e) => setUserId(e.target.value)} placeholder={t("userId")} />
          <button className="w-full rounded bg-sky-800 py-2" onClick={() => run(() => invoke("unlock_offline", { userId, pin }))}>
            {t("offlineUnlock")}
          </button>
        </main>
      </div>
    );
  }

  const tabs: { id: Tab; label: string }[] = [
    { id: "stations", label: t("sessions") },
    { id: "pos", label: t("pos") },
    { id: "sales", label: t("sales") },
    { id: "ops", label: t("ops") },
  ];

  return (
    <div className="min-h-full">
      {header}
      <nav className="flex flex-wrap gap-2 border-b border-slate-800 px-4 py-2">
        {tabs.map((item) => (
          <button
            key={item.id}
            className={`rounded px-3 py-1 ${tab === item.id ? "bg-emerald-800" : "bg-slate-800"}`}
            onClick={() => setTab(item.id)}
          >
            {item.label}
          </button>
        ))}
      </nav>
      <div className="grid gap-4 p-4 xl:grid-cols-[1fr_22rem]">
        <main className="min-w-0 space-y-4">
          {error ? <p className="text-red-400">{error}</p> : null}

          {tab === "stations" ? (
            <div className="grid gap-4 md:grid-cols-2">
              {stations.map((s) => {
                const live = s.session_id ? charges[s.session_id] : undefined;
                const playing = s.session_status === "active";
                const checkout = s.session_status === "stopped";
                return (
                  <section key={s.id} className="rounded border border-slate-700 p-4">
                    <div className="flex justify-between">
                      <h2 className="text-xl font-semibold">{s.display_name}</h2>
                      <span>{playing ? t("playing") : checkout ? t("checkout") : t("available")}</span>
                    </div>
                    {playing && live ? (
                      <p className="mt-2 text-2xl tabular-nums">
                        {formatClock(live.duration_seconds)} · {formatMoney(live.charge_minor)}
                      </p>
                    ) : null}
                    <div className="mt-4 flex flex-wrap gap-2">
                      {!playing && !checkout ? (
                        <button className="rounded bg-emerald-700 px-3 py-2" onClick={() => run(() => invoke("start_session", { stationId: s.id }))}>
                          {t("start")}
                        </button>
                      ) : null}
                      {playing ? (
                        <button
                          className="rounded bg-amber-700 px-3 py-2"
                          onClick={() =>
                            run(async () => {
                              await invoke("stop_session", { sessionId: s.session_id });
                              await loadOrder(s.order_id);
                            })
                          }
                        >
                          {t("stop")}
                        </button>
                      ) : null}
                      {checkout && s.session_id ? (
                        <button
                          className="rounded bg-sky-800 px-3 py-2"
                          onClick={() => run(() => invoke("resume_session", { sessionId: s.session_id, reason: "cashier resume" }))}
                        >
                          {t("resume")}
                        </button>
                      ) : null}
                      {s.order_id ? (
                        <button className="rounded bg-slate-700 px-3 py-2" onClick={() => loadOrder(s.order_id)}>
                          {t("selectOrder")}
                        </button>
                      ) : null}
                    </div>
                  </section>
                );
              })}
            </div>
          ) : null}

          {tab === "pos" ? (
            <section className="space-y-3 rounded border border-slate-700 p-4">
              <button
                className="rounded bg-emerald-700 px-3 py-2"
                onClick={() =>
                  run(async () => {
                    const opened = await invoke<{ order_id: string }>("open_pos_order");
                    await loadOrder(opened.order_id);
                  })
                }
              >
                {t("newWalkIn")}
              </button>
              {sales
                .filter((row) => row.status === "open" || row.status === "checkout_pending")
                .map((row) => (
                  <button
                    key={row.id}
                    className="flex w-full items-center justify-between rounded border border-slate-800 px-3 py-2 text-start"
                    onClick={() => loadOrder(row.id)}
                  >
                    <span>{row.id.slice(0, 8)} · {row.status}</span>
                    <span className="tabular-nums">{formatMoney(row.total_minor)}</span>
                  </button>
                ))}
            </section>
          ) : null}

          {tab === "sales" ? (
            <section className="space-y-2">
              {sales.map((row) => (
                <button
                  key={row.id}
                  className="flex w-full items-center justify-between rounded border border-slate-700 px-3 py-2 text-start"
                  onClick={() => loadOrder(row.id)}
                >
                  <span>
                    {row.receipt_number ?? row.id.slice(0, 8)} · {row.status}
                  </span>
                  <span className="tabular-nums">{formatMoney(row.total_minor)}</span>
                </button>
              ))}
            </section>
          ) : null}

          {tab === "ops" ? (
            <section className="space-y-4 rounded border border-slate-700 p-4">
              <div className="flex gap-2">
                <button
                  className="rounded bg-slate-700 px-3 py-2"
                  onClick={() =>
                    run(async () => {
                      setReport(await invoke<Report>("sales_today"));
                      if (isAdmin) {
                        const listed = await invoke<{ backups: BackupRow[] }>("list_backups");
                        setBackups(listed.backups);
                      }
                    })
                  }
                >
                  {t("todaySales")}
                </button>
                {isAdmin ? (
                  <button
                    className="rounded bg-emerald-800 px-3 py-2"
                    onClick={() =>
                      run(async () => {
                        await invoke("backup_now");
                        const listed = await invoke<{ backups: BackupRow[] }>("list_backups");
                        setBackups(listed.backups);
                      })
                    }
                  >
                    {t("backupNow")}
                  </button>
                ) : (
                  <span className="self-center text-sm text-slate-400">{t("adminOnly")}</span>
                )}
              </div>
              {report ? (
                <dl className="grid gap-2 sm:grid-cols-2">
                  <div>
                    {t("gamingRevenue")}: <span className="tabular-nums">{formatMoney(report.gaming_revenue_minor)}</span>
                  </div>
                  <div>
                    {t("productRevenue")}: <span className="tabular-nums">{formatMoney(report.product_revenue_minor)}</span>
                  </div>
                  <div>
                    {t("total")}: <span className="tabular-nums">{formatMoney(report.sales_revenue_minor)}</span>
                  </div>
                  <div>
                    {t("paidOrders")}: <span className="tabular-nums">{report.paid_orders}</span>
                  </div>
                </dl>
              ) : null}
              {isAdmin ? (
                <div className="space-y-2">
                  {backups.length === 0 ? <p className="text-slate-400">{t("noBackups")}</p> : null}
                  {backups.map((b) => (
                    <div key={b.path} className="flex items-center justify-between gap-2 rounded border border-slate-800 p-2">
                      <span className="truncate text-sm">{b.name}</span>
                      <button
                        className="rounded bg-amber-900 px-2 py-1"
                        onClick={() =>
                          run(async () => {
                            await invoke("restore_backup", { backupPath: b.path });
                            setRestoreHint(t("restartHint"));
                          })
                        }
                      >
                        {t("restore")}
                      </button>
                    </div>
                  ))}
                  {restoreHint ? <p className="text-amber-300">{restoreHint}</p> : null}
                </div>
              ) : null}
            </section>
          ) : null}
        </main>

        <aside className="space-y-4 rounded border border-slate-700 p-4">
          <h3 className="font-semibold">{t("items")}</h3>
          <div className="grid gap-2">
            {products.map((p) => (
              <button
                key={p.id}
                disabled={!order || order.status === "paid" || order.status === "void"}
                className="flex items-center justify-between rounded bg-slate-800 px-3 py-2 disabled:opacity-40"
                onClick={() =>
                  run(async () => {
                    if (!order) return;
                    await invoke("add_order_item", { orderId: order.id, productId: p.id, quantity: 1 });
                    await loadOrder(order.id);
                  })
                }
              >
                <span>{i18n.language.startsWith("ar") && p.name_ar ? p.name_ar : p.name}</span>
                <span className="text-sm text-slate-300">
                  {formatMoney(p.sell_price_minor)} · {t("stock")} {p.quantity_on_hand}
                </span>
              </button>
            ))}
          </div>

          {order ? (
            <div className="space-y-2 border-t border-slate-800 pt-3">
              <p className="text-sm text-slate-400">
                {order.order_type} · {order.status}
                {order.receipt_number ? ` · ${order.receipt_number}` : ""}
              </p>
              {order.items
                .filter((i) => i.status === "active")
                .map((item) => (
                  <div key={item.id} className="flex items-center justify-between text-sm">
                    <span>
                      {item.quantity} × {item.name}
                    </span>
                    <span className="flex items-center gap-2">
                      <span className="tabular-nums">{formatMoney(item.line_total_minor)}</span>
                      {order.status !== "paid" ? (
                        <button
                          className="text-red-300"
                          onClick={() =>
                            run(async () => {
                              await invoke("void_order_item", { itemId: item.id, reason: "cashier void" });
                              await loadOrder(order.id);
                            })
                          }
                        >
                          {t("voidItem")}
                        </button>
                      ) : null}
                    </span>
                  </div>
                ))}
              <p>
                {t("due")}: <span className="text-xl tabular-nums">{formatMoney(order.total_minor)}</span>
              </p>
              {order.status === "paid" ? (
                <p>
                  {t("change")}: <span className="tabular-nums">{formatMoney(order.change_minor)}</span>
                </p>
              ) : null}
              <label className="block text-sm text-slate-300">{t("tendered")}</label>
              <input className="w-full rounded bg-slate-900 p-2" value={tendered} onChange={(e) => setTendered(e.target.value)} />
              {order.status !== "paid" && order.status !== "void" ? (
                <button
                  className="w-full rounded bg-emerald-800 py-2"
                  onClick={() =>
                    run(async () => {
                      await invoke("take_cash", {
                        orderId: order.id,
                        tenderedMinor: Math.round(Number(tendered) * 100),
                      });
                      await loadOrder(order.id);
                    })
                  }
                >
                  {t("confirmPay")}
                </button>
              ) : null}
              {order.status !== "paid" && order.status !== "void" ? (
                <button
                  className="w-full rounded bg-slate-800 py-2"
                  onClick={() =>
                    run(async () => {
                      await invoke("void_order", { orderId: order.id, reason: "cashier void" });
                      setOrder(null);
                    })
                  }
                >
                  {t("voidOrder")}
                </button>
              ) : null}
              {isAdmin && order.status === "paid" ? (
                <button
                  className="w-full rounded bg-red-900 py-2"
                  onClick={() =>
                    run(async () => {
                      await invoke("reverse_payment", { orderId: order.id, reason: "cashier correction" });
                      await loadOrder(order.id);
                    })
                  }
                >
                  {t("reverse")}
                </button>
              ) : null}
            </div>
          ) : (
            <p className="text-slate-400">{t("selectOrder")}</p>
          )}
        </aside>
      </div>
    </div>
  );
}
