import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import {
  Activity,
  Boxes,
  Cable,
  Gauge,
  Globe,
  MonitorPlay,
  RefreshCw,
  RotateCcw,
  Wifi,
  Zap,
} from "lucide-react";
import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  applyDns,
  applyTcpTweaks,
  benchmarkDns,
  listTcpTweaks,
  networkStatus,
  pingTest,
  resetTcpTweaks,
} from "../lib/api";
import type {
  AdapterInventory,
  DnsBenchmarkResult,
  DnsServer,
  NetworkStatus,
  PingResult,
  TcpTweak,
} from "../lib/types";
import { errMsg } from "../lib/errors";
import { formatBytes } from "../lib/format";
import { Badge, Card } from "./ui";

function ms(v: number | null): string {
  return v === null ? "—" : `${v.toFixed(1)} ms`;
}

function linkSpeed(bps: number | null): string {
  if (!bps || bps <= 0) return "—";
  if (bps >= 1_000_000_000) {
    const gbps = bps / 1_000_000_000;
    return `${gbps % 1 === 0 ? gbps.toFixed(0) : gbps.toFixed(1)} Gbps`;
  }
  return `${Math.round(bps / 1_000_000)} Mbps`;
}

const KIND_TONE: Record<AdapterInventory["kind"], "cyan" | "violet" | "amber" | "slate"> = {
  ethernet: "cyan",
  wifi: "violet",
  vpn: "amber",
  virtual: "slate",
  bluetooth: "slate",
  other: "slate",
};

function KindIcon({ kind }: { kind: AdapterInventory["kind"] }) {
  if (kind === "wifi") return <Wifi className="h-3.5 w-3.5" />;
  if (kind === "vpn") return <Globe className="h-3.5 w-3.5" />;
  if (kind === "virtual") return <Boxes className="h-3.5 w-3.5" />;
  return <Cable className="h-3.5 w-3.5" />;
}

interface SeriesPoint {
  t: string;
  ms: number;
}

export function Network() {
  const isWindows =
    typeof navigator !== "undefined" && /windows|win32/i.test(navigator.userAgent);
  const [status, setStatus] = useState<NetworkStatus | null>(null);
  const [results, setResults] = useState<DnsBenchmarkResult[] | null>(null);
  const [tweaks, setTweaks] = useState<TcpTweak[]>([]);
  const [adapter, setAdapter] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [running, setRunning] = useState(false);
  const [applying, setApplying] = useState<string | null>(null);
  const [tweakBusy, setTweakBusy] = useState(false);
  const [pingHost, setPingHost] = useState("");
  const [pinging, setPinging] = useState(false);
  const [ping, setPing] = useState<PingResult | null>(null);
  const [monitoring, setMonitoring] = useState(false);
  const [series, setSeries] = useState<SeriesPoint[]>([]);
  const [live, setLive] = useState<PingResult | null>(null);
  const monitorBusy = useRef(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [s, t] = await Promise.all([networkStatus(), listTcpTweaks()]);
      setStatus(s);
      setTweaks(t);
      setAdapter((prev) => {
        if (prev && s.adapters.some((a) => a.guid === prev)) return prev;
        const active = s.adapters.find((a) => a.isActive) ?? s.adapters[0];
        return active?.guid ?? "";
      });
      if (!pingHost.trim()) {
        const gw = s.gateway;
        if (gw) setPingHost(gw);
      }
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, [pingHost]);

  useEffect(() => {
    void load();
    // Initial load only; the gateway prefill must not retrigger this effect.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!monitoring) return;
    const tick = async () => {
      if (monitorBusy.current) return;
      const host = pingHost.trim();
      if (!host) {
        setMonitoring(false);
        setError("Enter a host to monitor.");
        return;
      }
      monitorBusy.current = true;
      try {
        const r = await pingTest(host, 4);
        setLive(r);
        setSeries((prev) => [
          ...prev.slice(-59),
          {
            t: new Date().toLocaleTimeString([], {
              minute: "2-digit",
              second: "2-digit",
            }),
            ms: r.medianMs ?? 0,
          },
        ]);
      } catch (e) {
        setError(errMsg(e));
        setMonitoring(false);
      } finally {
        monitorBusy.current = false;
      }
    };
    void tick();
    const id = setInterval(() => void tick(), 2000);
    return () => clearInterval(id);
  }, [monitoring, pingHost]);

  async function onBenchmark() {
    setRunning(true);
    setError(null);
    setResults(null);
    try {
      setResults(await benchmarkDns([], 3));
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setRunning(false);
    }
  }

  async function onApply(server: DnsServer) {
    if (!adapter) return;
    if (
      !window.confirm(
        `Set ${server.name} (${server.ip}) as the DNS server for this adapter? A snapshot is created first.`,
      )
    ) {
      return;
    }
    setApplying(server.ip);
    setError(null);
    setNotice(null);
    try {
      await applyDns(adapter, [server.ip]);
      setNotice(`Applied ${server.ip}. Reversible via Rollback Center.`);
      await load();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setApplying(null);
    }
  }

  async function onPing() {
    const host = pingHost.trim();
    if (!host) {
      setError("Enter a host to ping (IP or hostname).");
      return;
    }
    setPinging(true);
    setError(null);
    setPing(null);
    try {
      setPing(await pingTest(host, 8));
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setPinging(false);
    }
  }

  async function onApplyTweaks() {
    if (!window.confirm("Apply the experimental TCP/IP tweaks? A snapshot is created first and every change is reversible.")) return;
    setTweakBusy(true);
    setError(null);
    setNotice(null);
    try {
      const r = await applyTcpTweaks();
      setNotice(
        r.changes > 0
          ? `Applied ${r.changes} TCP tweaks (snapshot ${r.snapshotId.slice(0, 8)}…).`
          : "TCP tweaks were already applied.",
      );
      setTweaks(await listTcpTweaks());
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setTweakBusy(false);
    }
  }

  async function onResetTweaks() {
    if (!window.confirm("Revert all TCP/IP tweaks to driver defaults?")) return;
    setTweakBusy(true);
    setError(null);
    setNotice(null);
    try {
      const r = await resetTcpTweaks();
      setNotice(
        r.changes > 0 ? `Reverted ${r.changes} TCP tweaks.` : "No tweaks to revert.",
      );
      setTweaks(await listTcpTweaks());
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setTweakBusy(false);
    }
  }

  const sortedResults = sortDnsResults(results);

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Network</h1>
          <p className="text-sm text-slate-500">
            Inspect every adapter and driver, benchmark DNS resolvers, and watch your real
            connection stability. DNS affects lookups and CDN selection — not in-game ping after
            a connection is established.
          </p>
        </div>
        <button
          onClick={load}
          disabled={loading}
          className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
        >
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          Refresh
        </button>
      </header>

      {error && (
        <div className="rounded-lg border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
          {error}
        </div>
      )}
      {notice && (
        <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-300">
          {notice}
        </div>
      )}

      <AdapterCard status={status} />

      <Card title="Connection">
        {status ? (
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <div>
              <div className="text-xs uppercase tracking-wider text-slate-500">Gateway</div>
              <div className="mt-1 text-lg font-semibold text-slate-100">
                {status.gateway ?? "not detected"}
              </div>
            </div>
            <div>
              <div className="text-xs uppercase tracking-wider text-slate-500">Current DNS</div>
              <div className="mt-1 text-lg font-semibold text-slate-100">
                {status.currentDns.length > 0 ? status.currentDns.join(", ") : "not detected"}
              </div>
            </div>
            <div>
              <div className="text-xs uppercase tracking-wider text-slate-500">Apply target</div>
              <select
                value={adapter}
                onChange={(e) => setAdapter(e.currentTarget.value)}
                className="mt-1 w-full rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 focus:border-cyan-500 focus:outline-none"
              >
                {status.adapters.map((a) => (
                  <option key={a.guid} value={a.guid}>
                    {a.name}
                    {a.isActive ? " (active)" : ""}
                  </option>
                ))}
              </select>
            </div>
          </div>
        ) : (
          <p className="text-sm text-slate-500">No network interfaces detected.</p>
        )}
      </Card>

      <Card
        title="DNS Benchmark"
        action={
          <button
            onClick={onBenchmark}
            disabled={running}
            className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
          >
            <Gauge className="h-3.5 w-3.5" />
            {running ? "Benchmarking…" : "Run benchmark"}
          </button>
        }
      >
        {!results ? (
          <p className="text-sm text-slate-500">
            Queries each resolver with a few common domains and reports median latency and
            packet loss. Run it before and after applying to see the difference.
          </p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {sortedResults.map((r) => (
              <li key={r.ip} className="flex items-center gap-3 py-2.5">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-slate-800">
                  <Globe className="h-4 w-4 text-cyan-400" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-slate-200">{r.name}</span>
                    <span className="font-mono text-xs text-slate-500">{r.ip}</span>
                    {r.isCurrent && <Badge tone="emerald">current</Badge>}
                    {r.lossPercent > 0 && <Badge tone="amber">{r.lossPercent.toFixed(0)}% loss</Badge>}
                  </div>
                  <div className="mt-0.5 text-xs tabular-nums text-slate-500">
                    median {ms(r.medianMs)} · p95 {ms(r.p95Ms)} · min {ms(r.minMs)}
                  </div>
                </div>
                <button
                  onClick={() => onApply({ name: r.name, ip: r.ip, isCurrent: r.isCurrent })}
                  disabled={applying === r.ip || !adapter}
                  className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
                >
                  <Zap className="h-3 w-3" />
                  {applying === r.ip ? "Applying…" : "Apply"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <Card title="Ping & stability monitor">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
          <input
            value={pingHost}
            onChange={(e) => setPingHost(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && void onPing()}
            placeholder="gateway, game server, or 1.1.1.1"
            className="flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-slate-100 placeholder-slate-500 focus:border-cyan-500 focus:outline-none"
          />
          {status?.gateway && pingHost.trim() !== status.gateway && (
            <button
              onClick={() => setPingHost(status.gateway ?? "")}
              className="rounded-lg bg-slate-800 px-3 py-2 text-xs font-medium text-slate-300 transition-colors hover:bg-slate-700"
            >
              Use gateway
            </button>
          )}
          <button
            onClick={onPing}
            disabled={pinging || monitoring}
            className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
          >
            <Activity className="h-4 w-4" />
            {pinging ? "Pinging…" : "Ping"}
          </button>
          <button
            onClick={() => {
              if (monitoring) {
                setMonitoring(false);
              } else {
                if (!pingHost.trim()) {
                  setError("Enter a host to monitor.");
                  return;
                }
                setSeries([]);
                setLive(null);
                setMonitoring(true);
              }
            }}
            className={`flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50 ${
              monitoring
                ? "bg-rose-600/90 text-white hover:bg-rose-500"
                : "bg-slate-800 text-slate-200 hover:bg-slate-700"
            }`}
          >
            <MonitorPlay className="h-4 w-4" />
            {monitoring ? "Stop monitor" : "Monitor"}
          </button>
        </div>

        {monitoring && (
          <div className="mt-4 h-36 rounded-lg border border-slate-800 bg-slate-950/60 p-2">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={series} margin={{ top: 6, right: 8, left: 0, bottom: 0 }}>
                <defs>
                  <linearGradient id="pingFill" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#22d3ee" stopOpacity={0.35} />
                    <stop offset="100%" stopColor="#22d3ee" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
                <XAxis dataKey="t" stroke="#475569" fontSize={10} tickLine={false} minTickGap={48} />
                <YAxis
                  stroke="#475569"
                  fontSize={10}
                  tickLine={false}
                  width={38}
                  unit="ms"
                  domain={[0, (max: number) => Math.max(10, Math.ceil(max * 1.25))]}
                />
                <Tooltip
                  contentStyle={{ background: "#0f172a", border: "1px solid #1e293b", borderRadius: 8 }}
                  labelStyle={{ color: "#94a3b8" }}
                  formatter={(value) => [`${Number(value).toFixed(1)} ms`, "median"]}
                />
                <Area
                  type="monotone"
                  dataKey="ms"
                  name="median"
                  stroke="#22d3ee"
                  strokeWidth={2}
                  fill="url(#pingFill)"
                  isAnimationActive={false}
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        )}

        {(ping || live) && (
          <div className="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-5">
            <StatBlock label="Median" value={ms((live ?? ping)!.medianMs)} />
            <StatBlock label="Jitter" value={ms((live ?? ping)!.jitterMs)} />
            <StatBlock
              label="Min / Max"
              value={`${ms((live ?? ping)!.minMs)} / ${ms((live ?? ping)!.maxMs)}`}
              small
            />
            <StatBlock label="Loss" value={`${(live ?? ping)!.lossPercent.toFixed(0)}%`} />
            <StatBlock
              label="Replies"
              value={`${(live ?? ping)!.received}/${(live ?? ping)!.sent}`}
            />
          </div>
        )}
        <p className="mt-3 text-xs text-slate-600">
          ICMP round-trip time and jitter — the honest in-game connection signal. Monitor pings
          the gateway every 2 s: spikes here mean your local network; stable gateway but high
          external ping points at your ISP. Good targets: jitter under 5 ms, zero packet loss.
        </p>
      </Card>

      <Card
        title="TCP/IP Tweaks (experimental)"
        action={
          isWindows && (
            <div className="flex items-center gap-2">
              <button
                onClick={onApplyTweaks}
                disabled={tweakBusy}
                className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
              >
                <Zap className="h-3.5 w-3.5" />
                Apply recommended
              </button>
              <button
                onClick={onResetTweaks}
                disabled={tweakBusy}
                className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
              >
                <RotateCcw className="h-3.5 w-3.5" />
                Revert to defaults
              </button>
            </div>
          )
        }
      >
        {!isWindows ? (
          <p className="text-sm text-slate-500">
            TCP/IP registry tweaks are only available on Windows; nothing is
            listed on this platform.
          </p>
        ) : tweaks.length === 0 ? (
          <p className="text-sm text-slate-500">
            No TCP parameters exposed (Windows-only).
          </p>
        ) : (
          <ul className="grid grid-cols-1 gap-x-8 gap-y-2 sm:grid-cols-2">
            {tweaks.map((t) => (
              <li key={t.name} className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <span className="font-mono text-sm text-slate-400">{t.name}</span>
                  <p className="truncate text-xs text-slate-600" title={t.description}>
                    {t.description}
                  </p>
                </div>
                <span className="flex shrink-0 items-center gap-2">
                  {t.applied && <Badge tone="emerald">applied</Badge>}
                  <span className="tabular-nums text-sm text-slate-200">
                    {t.current === null ? "default" : t.current}
                  </span>
                </span>
              </li>
            ))}
          </ul>
        )}
        {isWindows && (
          <p className="mt-3 text-xs text-slate-600">
            These legacy registry tweaks are mostly placebo on modern Windows — real wins come
            from DNS selection and a wired connection. Everything here is snapshot-first and
            revertible in one click.
          </p>
        )}
      </Card>
    </div>
  );
}

function sortDnsResults(results: DnsBenchmarkResult[] | null): DnsBenchmarkResult[] {
  return (
    results?.slice().sort((a, b) => {
      if (a.medianMs === null) return 1;
      if (b.medianMs === null) return -1;
      return a.medianMs - b.medianMs;
    }) ?? []
  );
}

function StatBlock({ label, value, small }: { label: string; value: string; small?: boolean }) {
  return (
    <div>
      <div className="text-xs uppercase tracking-wider text-slate-500">{label}</div>
      <div className={`mt-0.5 font-semibold text-slate-100 ${small ? "text-sm" : "text-lg"}`}>
        {value}
      </div>
    </div>
  );
}

function DetailRow({ items }: { items: [string, ReactNode][] }) {
  return (
    <dl className="mt-2 grid grid-cols-2 gap-x-4 gap-y-1 text-xs sm:grid-cols-4">
      {items.map(([k, v]) => (
        <div key={k} className="min-w-0">
          <dt className="text-slate-600">{k}</dt>
          <dd className="truncate text-slate-300" title={typeof v === "string" ? v : undefined}>
            {v}
          </dd>
        </div>
      ))}
    </dl>
  );
}

function AdapterCard({ status }: { status: NetworkStatus | null }) {
  if (!status || status.inventory.length === 0) {
    return (
      <Card title="Adapters">
        <p className="text-sm text-slate-500">No adapters detected.</p>
      </Card>
    );
  }
  return (
    <Card title={`Adapters (${status.inventory.length})`}>
      <ul className="space-y-3">
        {status.inventory.map((a) => {
          const counters = a.counters;
          const hasIssues =
            counters &&
            (counters.receiveErrors > 0 ||
              counters.sendErrors > 0 ||
              counters.receiveDiscards > 0 ||
              counters.sendDiscards > 0);
          return (
            <li
              key={a.guid}
              className="rounded-lg border border-slate-800 bg-slate-950/40 p-3"
            >
              <div className="flex flex-wrap items-center gap-2">
                <span
                  className={`h-2 w-2 shrink-0 rounded-full ${
                    a.isUp ? "bg-emerald-400" : "bg-slate-600"
                  }`}
                  title={a.isUp ? "connected" : "down"}
                />
                <span className="font-medium text-slate-100">{a.name || a.guid}</span>
                <Badge tone={KIND_TONE[a.kind]}>
                  <KindIcon kind={a.kind} />
                  <span className="ml-1">{a.kind.toUpperCase()}</span>
                </Badge>
                {a.driver?.fullDuplex === false && <Badge tone="amber">half duplex</Badge>}
                {hasIssues && <Badge tone="amber">interface errors</Badge>}
                {!a.isUp && <Badge tone="slate">down</Badge>}
              </div>

              <DetailRow
                items={[
                  ["Link", `${linkSpeed(a.receiveLinkBps)} ↓ / ${linkSpeed(a.transmitLinkBps)} ↑`],
                  ["MTU", a.mtu ? String(a.mtu) : "—"],
                  ["MAC", a.macAddress ?? "—"],
                  [
                    "IP",
                    a.ipAddresses.length > 0 ? a.ipAddresses.join(", ") : "no address",
                  ],
                  ...(a.gateways.length > 0
                    ? ([["Gateway", a.gateways.join(", ")]] as [string, ReactNode][])
                    : []),
                  ...(a.dhcpEnabled !== null && !a.isVirtual
                    ? ([["DHCP", a.dhcpEnabled ? "enabled" : "static"]] as [string, ReactNode][])
                    : []),
                  ...(a.driver
                    ? ([
                        [
                          "Driver",
                          [
                            a.driver.version,
                            a.driver.date,
                            a.driver.provider,
                            a.driver.ndisVersion ? `NDIS ${a.driver.ndisVersion}` : null,
                          ]
                            .filter(Boolean)
                            .join(" · ") || "—",
                        ],
                      ] as [string, ReactNode][])
                    : []),
                ]}
              />

              {a.wifi && (
                <DetailRow
                  items={[
                    ["SSID", a.wifi.ssid ?? "—"],
                    ["Signal", a.wifi.signalPercent !== null ? `${a.wifi.signalPercent}%${a.wifi.rssiDbm !== null ? ` (~${a.wifi.rssiDbm} dBm)` : ""}` : "—"],
                    ["Channel", a.wifi.channel !== null ? String(a.wifi.channel) : "—"],
                    ["Radio", a.wifi.phyType],
                    [
                      "Rates",
                      `${a.wifi.rxRateMbps !== null ? `${Math.round(a.wifi.rxRateMbps)} Mb/s` : "—"} rx · ${
                        a.wifi.txRateMbps !== null ? `${Math.round(a.wifi.txRateMbps)} Mb/s` : "—"
                      } tx`,
                    ],
                    ["Auth", a.wifi.authentication],
                    ["Cipher", a.wifi.cipher],
                    ["BSSID", a.wifi.bssid ?? "—"],
                  ]}
                />
              )}

              {counters && (
                <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs tabular-nums text-slate-500">
                  <span>RX {formatBytes(counters.receivedBytes)}</span>
                  <span>TX {formatBytes(counters.sentBytes)}</span>
                  {counters.receiveErrors > 0 && (
                    <span className="text-amber-400">RX errors {counters.receiveErrors}</span>
                  )}
                  {counters.sendErrors > 0 && (
                    <span className="text-amber-400">TX errors {counters.sendErrors}</span>
                  )}
                  {counters.receiveDiscards > 0 && (
                    <span className="text-amber-400">RX dropped {counters.receiveDiscards}</span>
                  )}
                  {counters.sendDiscards > 0 && (
                    <span className="text-amber-400">TX dropped {counters.sendDiscards}</span>
                  )}
                </div>
              )}

              <p className="mt-2 truncate text-xs text-slate-600" title={a.description}>
                {a.description}
              </p>
            </li>
          );
        })}
      </ul>
    </Card>
  );
}
