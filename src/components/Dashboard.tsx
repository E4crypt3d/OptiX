import { useEffect, useState } from "react";
import { useInterval } from "../lib/useInterval";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  Cpu,
  Gpu,
  MemoryStick,
  Monitor,
  RefreshCw,
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
import { recordSample, recentSamples, scanSystem, systemStats } from "../lib/api";
import { formatBytes, formatFrequency, formatRate, formatUptime } from "../lib/format";
import type { HardwareInfo, HardwareSample, SystemStats } from "../lib/types";
import { Badge, Card, ProgressBar, Stat } from "./ui";

interface HistoryPoint {
  time: string;
  cpu: number;
  ram: number;
}

const SCAN_TTL_MS = 60_000;
const HISTORY_LENGTH = 60;

// Module-level so the chart and the system scan survive tab switches: returning
// to the Dashboard doesn't re-run the expensive WMI scan or start with an empty
// chart.
let cachedInfo: HardwareInfo | null = null;
let cachedAtMs = 0;
let sharedHistory: HistoryPoint[] = [];

function loadInfo(force = false): Promise<HardwareInfo> {
  if (!force && cachedInfo && Date.now() - cachedAtMs < SCAN_TTL_MS) {
    return Promise.resolve(cachedInfo);
  }
  return scanSystem().then((info) => {
    cachedInfo = info;
    cachedAtMs = Date.now();
    return info;
  });
}

function sampleToPoint(s: HardwareSample): HistoryPoint {
  const ramPct =
    s.ramUsedMb != null && s.ramTotalMb && s.ramTotalMb > 0
      ? (s.ramUsedMb / s.ramTotalMb) * 100
      : 0;
  return {
    time: new Date(s.tsMs).toLocaleTimeString(),
    cpu: s.cpuUsage ?? 0,
    ram: ramPct,
  };
}

export function Dashboard() {
  const [info, setInfo] = useState<HardwareInfo | null>(null);
  const [scanError, setScanError] = useState<string | null>(null);
  const [scanLoading, setScanLoading] = useState(false);
  const [stats, setStats] = useState<SystemStats | null>(null);
  const [history, setHistory] = useState<HistoryPoint[]>(sharedHistory);

  useEffect(() => {
    let cancelled = false;

    // Poll once immediately so the dashboard isn't blank for the first tick.
    void poll();

    loadInfo()
      .then((i) => {
        if (!cancelled) setInfo(i);
      })
      .catch((e) => {
        console.error(e);
        if (!cancelled) setScanError(String(e));
      });

    // Backfill the chart from persisted samples when there's no live history
    // yet (first visit of the session, after an app restart).
    if (sharedHistory.length === 0) {
      recentSamples()
        .then((samples) => {
          if (cancelled || sharedHistory.length > 0) return;
          const points = [...samples]
            .reverse()
            .slice(-HISTORY_LENGTH)
            .map(sampleToPoint);
          if (points.length > 0) {
            sharedHistory = points;
            setHistory(points);
          }
        })
        .catch(console.error);
    }

    return () => {
      cancelled = true;
    };
  }, []);

  // Telemetry polling is paused while the window is hidden, so the dashboard
  // idles at zero cost when minimized or covered by other windows.
  useInterval(() => {
    void recordSample().catch(console.error);
  }, 30_000);

  async function poll() {
    try {
      const next = await systemStats();
      setStats(next);
      const point: HistoryPoint = {
        time: new Date(next.timestampMs).toLocaleTimeString(),
        cpu: next.cpuUsagePercent,
        ram: next.memory.usagePercent,
      };
      sharedHistory = [...sharedHistory.slice(-(HISTORY_LENGTH - 1)), point];
      setHistory(sharedHistory);
    } catch (e) {
      console.error(e);
    }
  }

  useInterval(() => void poll(), 1500);

  async function rescanInfo() {
    setScanLoading(true);
    setScanError(null);
    try {
      setInfo(await loadInfo(true));
    } catch (e) {
      console.error(e);
      setScanError(String(e));
    } finally {
      setScanLoading(false);
    }
  }

  const cpu = stats?.cpuUsagePercent;
  const mem = stats?.memory ?? info?.memory;
  // Rates are computed in the backend from refresh-window byte deltas.
  const down = stats?.network.reduce((a, n) => a + n.receivedBytesPerSec, 0) ?? 0;
  const up = stats?.network.reduce((a, n) => a + n.transmittedBytesPerSec, 0) ?? 0;
  const uptimeSeconds = info
    ? info.os.uptimeSeconds + Math.max(0, (Date.now() - info.scannedAtMs) / 1000)
    : 0;

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Dashboard</h1>
          <p className="text-sm text-slate-500">
            {info
              ? `${info.os.name} ${info.os.version} · up ${formatUptime(uptimeSeconds)}`
              : "Loading system information…"}
          </p>
        </div>
        <div className="flex items-center gap-3">
          {info && <span className="text-xs text-slate-500">{info.os.hostName}</span>}
          <button
            onClick={rescanInfo}
            disabled={scanLoading}
            className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${scanLoading ? "animate-spin" : ""}`} />
            Rescan
          </button>
        </div>
      </header>

      {scanError && (
        <div className="flex items-center justify-between gap-3 rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
          <span className="truncate">Couldn't scan your system: {scanError}</span>
          <button
            onClick={() => {
              setScanError(null);
              loadInfo(true)
                .then(setInfo)
                .catch((e) => {
                  console.error(e);
                  setScanError(String(e));
                });
            }}
            className="shrink-0 rounded-lg bg-rose-500/20 px-3 py-1.5 text-xs font-medium text-rose-200 hover:bg-rose-500/30"
          >
            Retry
          </button>
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Stat
          label="CPU Usage"
          value={cpu !== undefined ? `${cpu.toFixed(1)}%` : "—"}
          sub={
            info
              ? `${info.cpu.name} · ${info.cpu.logicalCores} threads`
              : undefined
          }
          icon={<Cpu className="h-4 w-4 text-cyan-400" />}
        />
        <Stat
          label="Memory"
          value={mem ? `${mem.usagePercent.toFixed(1)}%` : "—"}
          sub={
            mem
              ? `${formatBytes(mem.usedBytes)} / ${formatBytes(mem.totalBytes)}`
              : undefined
          }
          icon={<MemoryStick className="h-4 w-4 text-violet-400" />}
        />
        <Stat
          label="Download"
          value={formatRate(down)}
          sub="across all interfaces"
          icon={<ArrowDown className="h-4 w-4 text-emerald-400" />}
        />
        <Stat
          label="Upload"
          value={formatRate(up)}
          sub="across all interfaces"
          icon={<ArrowUp className="h-4 w-4 text-amber-400" />}
        />
      </div>

      <div className="space-y-4">
        <Card title="CPU & Memory History" action={<span className="text-xs text-slate-500">Last 90 seconds</span>}>
          <div
            className="h-64"
            role="img"
            aria-label="CPU and memory usage history chart"
          >
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={history} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
                <defs>
                  <linearGradient id="cpuFill" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#22d3ee" stopOpacity={0.35} />
                    <stop offset="100%" stopColor="#22d3ee" stopOpacity={0} />
                  </linearGradient>
                  <linearGradient id="ramFill" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#a78bfa" stopOpacity={0.35} />
                    <stop offset="100%" stopColor="#a78bfa" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
                <XAxis dataKey="time" stroke="#475569" fontSize={11} tickLine={false} minTickGap={40} />
                <YAxis domain={[0, 100]} width={34} stroke="#475569" fontSize={11} tickLine={false} />
                <Tooltip
                  contentStyle={{
                    background: "#0f172a",
                    border: "1px solid #1e293b",
                    borderRadius: 8,
                    fontSize: 12,
                  }}
                  formatter={(value, name) => [
                    `${Number(value).toFixed(1)}%`,
                    String(name),
                  ]}
                />
                <Area type="monotone" dataKey="cpu" name="CPU %" stroke="#22d3ee" strokeWidth={2} fill="url(#cpuFill)" isAnimationActive={false} />
                <Area type="monotone" dataKey="ram" name="RAM %" stroke="#a78bfa" strokeWidth={2} fill="url(#ramFill)" isAnimationActive={false} />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </Card>

        <Card
          title="Per-Core Usage"
          action={
            (stats?.perCoreUsage ?? []).length > 0 ? (
              <span className="text-xs text-slate-500">
                {(stats?.perCoreUsage ?? []).length} logical processors
              </span>
            ) : undefined
          }
        >
          {(stats?.perCoreUsage ?? []).length === 0 ? (
            <p className="text-sm text-slate-500">Collecting core data…</p>
          ) : (
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-6">
              {(stats?.perCoreUsage ?? []).map((usage, i) => (
                <div
                  key={i}
                  className="rounded-lg border border-slate-800/80 bg-slate-950/40 p-3"
                >
                  <div className="mb-2 flex items-center justify-between gap-2">
                    <span className="text-xs font-medium text-slate-400">Core {i}</span>
                    <span className="text-sm font-semibold tabular-nums text-slate-200">
                      {usage.toFixed(0)}%
                    </span>
                  </div>
                  <ProgressBar value={usage} tone="cyan" />
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Card title="CPU">
          {info ? (
            <div className="space-y-2 text-sm">
              <Row label="Model" value={info.cpu.brand || info.cpu.name} />
              <Row label="Cores" value={`${info.cpu.physicalCores} physical / ${info.cpu.logicalCores} logical`} />
              <Row label="Clock" value={formatFrequency(info.cpu.frequencyMhz)} />
              <Row label="Vendor" value={info.cpu.vendor || "—"} />
            </div>
          ) : (
            <Loading />
          )}
        </Card>

        <Card title="Graphics adapters">
          {info && info.gpus.length > 0 ? (
            <div className="space-y-3 text-sm">
              {info.gpus.map((g, i) => (
                <div key={`${g.name}-${i}`} className="space-y-1.5">
                  <div className="flex items-start gap-2">
                    <Gpu className="mt-0.5 h-4 w-4 shrink-0 text-violet-400" />
                    <div className="min-w-0 flex-1">
                      <div className="font-medium leading-5 text-slate-200">{g.name}</div>
                      <div className="mt-1 text-xs text-slate-500">
                        {[g.vendor, g.driverVersion ? `Driver ${g.driverVersion}` : ""]
                          .filter(Boolean)
                          .join(" · ") || "Driver information unavailable"}
                      </div>
                    </div>
                    {info.gpus.length > 1 && <Badge tone="slate">#{i + 1}</Badge>}
                  </div>
                  <div className="pl-6 text-xs text-slate-500">
                    VRAM {g.memoryBytes > 0 ? formatBytes(g.memoryBytes) : "shared / system managed"}
                  </div>
                </div>
              ))}
            </div>
          ) : info ? (
            <p className="text-sm text-slate-500">No graphics adapters detected</p>
          ) : (
            <Loading />
          )}
        </Card>

        <Card title="Displays">
          {info && info.displays.length > 0 ? (
            <div className="space-y-3 text-sm">
              {info.displays.map((d, i) => (
                <div key={`${d.name}-${i}`} className="flex items-start gap-2">
                  <Monitor className="mt-0.5 h-4 w-4 shrink-0 text-cyan-400" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate font-medium text-slate-200">
                        {d.name || `Display ${i + 1}`}
                      </span>
                      {d.isPrimary && <Badge tone="cyan">Primary</Badge>}
                    </div>
                    <div className="mt-1 text-slate-500">
                      {d.width}×{d.height} · {d.refreshRate > 0 ? `${d.refreshRate} Hz` : "refresh rate unavailable"}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : info ? (
            <p className="text-sm text-slate-500">No connected displays detected</p>
          ) : (
            <Loading />
          )}
        </Card>

        <Card title="Storage">
          {info ? (
            <div className="space-y-3 text-sm">
              {info.disks.length === 0 && (
                <p className="text-slate-500">No storage detected</p>
              )}
              {info.disks.map((d) => (
                <div key={d.mountPoint} className="space-y-1.5">
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate text-slate-300">{d.mountPoint}</span>
                    <Badge tone={d.kind === "SSD" ? "cyan" : d.kind === "HDD" ? "amber" : "slate"}>
                      {d.kind}
                    </Badge>
                  </div>
                  <ProgressBar
                    value={(d.usedBytes / Math.max(1, d.totalBytes)) * 100}
                    tone="violet"
                  />
                  <div className="flex justify-between gap-2 text-xs tabular-nums text-slate-500">
                    <span>{formatBytes(d.usedBytes)} used</span>
                    <span>{formatBytes(d.totalBytes)} total</span>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <Loading />
          )}
        </Card>
      </div>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <span className="text-slate-500">{label}</span>
      <span className="truncate text-right text-slate-200">{value}</span>
    </div>
  );
}

function Loading() {
  return (
    <div className="flex items-center gap-2 text-sm text-slate-500">
      <Activity className="h-4 w-4 animate-pulse" />
      Loading…
    </div>
  );
}
