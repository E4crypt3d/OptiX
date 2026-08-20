import { useCallback, useEffect, useMemo, useState } from "react";
import { Activity, Gauge, Globe, RefreshCw, RotateCcw, Zap } from "lucide-react";
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
  DnsBenchmarkResult,
  DnsServer,
  NetworkStatus,
  PingResult,
  TcpTweak,
} from "../lib/types";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";

function ms(v: number | null): string {
  return v === null ? "—" : `${v.toFixed(1)} ms`;
}

export function Network() {
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
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

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

  const sortedResults = useMemo(() => {
    if (!results) return [];
    return [...results].sort((a, b) => {
      if (a.medianMs === null) return 1;
      if (b.medianMs === null) return -1;
      return a.medianMs - b.medianMs;
    });
  }, [results]);

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Network</h1>
          <p className="text-sm text-slate-500">
            Benchmark DNS resolvers and apply the fastest. DNS affects lookups and CDN
            selection — not in-game ping after a connection is established.
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

      <Card title="Ping test">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
          <input
            value={pingHost}
            onChange={(e) => setPingHost(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && void onPing()}
            placeholder="gateway, game server, or 1.1.1.1"
            className="flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-slate-100 placeholder-slate-500 focus:border-cyan-500 focus:outline-none"
          />
          <button
            onClick={onPing}
            disabled={pinging}
            className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
          >
            <Activity className="h-4 w-4" />
            {pinging ? "Pinging…" : "Ping"}
          </button>
        </div>
        {ping && (
          <div className="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-5">
            <div>
              <div className="text-xs uppercase tracking-wider text-slate-500">Median</div>
              <div className="mt-0.5 text-lg font-semibold text-slate-100">{ms(ping.medianMs)}</div>
            </div>
            <div>
              <div className="text-xs uppercase tracking-wider text-slate-500">Jitter</div>
              <div className="mt-0.5 text-lg font-semibold text-slate-100">{ms(ping.jitterMs)}</div>
            </div>
            <div>
              <div className="text-xs uppercase tracking-wider text-slate-500">Min / Max</div>
              <div className="mt-0.5 text-sm text-slate-200">
                {ms(ping.minMs)} / {ms(ping.maxMs)}
              </div>
            </div>
            <div>
              <div className="text-xs uppercase tracking-wider text-slate-500">Loss</div>
              <div className="mt-0.5 text-lg font-semibold text-slate-100">
                {ping.lossPercent.toFixed(0)}%
              </div>
            </div>
            <div>
              <div className="text-xs uppercase tracking-wider text-slate-500">Replies</div>
              <div className="mt-0.5 text-lg font-semibold text-slate-100">
                {ping.received}/{ping.sent}
              </div>
            </div>
          </div>
        )}
        <p className="mt-3 text-xs text-slate-600">
          ICMP round-trip time and jitter — the honest in-game connection signal (unlike DNS,
          which only affects lookups).
        </p>
      </Card>

      <Card
        title="TCP/IP Tweaks (experimental)"
        action={
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
        }
      >
        {tweaks.length === 0 ? (
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
        <p className="mt-3 text-xs text-slate-600">
          These legacy registry tweaks are mostly placebo on modern Windows — real wins come
          from DNS selection and a wired connection. Everything here is snapshot-first and
          revertible in one click.
        </p>
      </Card>
    </div>
  );
}
