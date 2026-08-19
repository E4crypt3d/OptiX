import { useCallback, useEffect, useMemo, useState } from "react";
import { Gauge, Globe, RefreshCw, Zap } from "lucide-react";
import {
  applyDns,
  benchmarkDns,
  networkStatus,
  tcpParameters,
} from "../lib/api";
import type {
  DnsBenchmarkResult,
  DnsServer,
  NetworkStatus,
  TcpParameter,
} from "../lib/types";
import { Badge, Card } from "./ui";

function ms(v: number | null): string {
  return v === null ? "—" : `${v.toFixed(1)} ms`;
}

export function Network() {
  const [status, setStatus] = useState<NetworkStatus | null>(null);
  const [results, setResults] = useState<DnsBenchmarkResult[] | null>(null);
  const [tcp, setTcp] = useState<TcpParameter[]>([]);
  const [adapter, setAdapter] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [running, setRunning] = useState(false);
  const [applying, setApplying] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [s, t] = await Promise.all([networkStatus(), tcpParameters()]);
      setStatus(s);
      setTcp(t);
      setAdapter((prev) => {
        if (prev && s.adapters.some((a) => a.guid === prev)) return prev;
        const active = s.adapters.find((a) => a.isActive) ?? s.adapters[0];
        return active?.guid ?? "";
      });
    } catch (e) {
      setError(String(e));
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
      setError(String(e));
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
      setError(String(e));
    } finally {
      setApplying(null);
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

      <Card title="TCP/IP Parameters (experimental)">
        {tcp.length === 0 ? (
          <p className="text-sm text-slate-500">
            No TCP parameters exposed (Windows-only).
          </p>
        ) : (
          <ul className="grid grid-cols-1 gap-x-8 gap-y-2 sm:grid-cols-2">
            {tcp.map((p) => (
              <li key={p.name} className="flex items-center justify-between">
                <span className="font-mono text-sm text-slate-400">{p.name}</span>
                <span className="tabular-nums text-sm text-slate-200">
                  {p.value === null ? "default" : p.value}
                </span>
              </li>
            ))}
          </ul>
        )}
        <p className="mt-3 text-xs text-slate-600">
          These legacy tweaks are mostly placebo on modern Windows — real wins come from DNS
          selection and a wired connection. Read-only for now.
        </p>
      </Card>
    </div>
  );
}
