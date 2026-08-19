import { useEffect, useRef, useState } from "react";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  Cpu,
  HardDrive,
  MemoryStick,
  Monitor,
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
import { recordSample, scanSystem, systemStats } from "../lib/api";
import { formatBytes, formatFrequency, formatRate, formatUptime } from "../lib/format";
import type { HardwareInfo, SystemStats } from "../lib/types";
import { Card, ProgressBar, Stat } from "./ui";

interface HistoryPoint {
  time: string;
  cpu: number;
  ram: number;
}

export function Dashboard() {
  const [info, setInfo] = useState<HardwareInfo | null>(null);
  const [stats, setStats] = useState<SystemStats | null>(null);
  const [history, setHistory] = useState<HistoryPoint[]>([]);
  const historyRef = useRef<HistoryPoint[]>([]);

  useEffect(() => {
    scanSystem().then(setInfo).catch(console.error);
  }, []);

  // Persist a telemetry sample every 30s while the dashboard is open.
  useEffect(() => {
    const id = window.setInterval(() => {
      void recordSample().catch(console.error);
    }, 30_000);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function poll() {
      try {
        const next = await systemStats();
        if (cancelled) return;
        setStats(next);
        const point: HistoryPoint = {
          time: new Date(next.timestampMs).toLocaleTimeString(),
          cpu: next.cpuUsagePercent,
          ram: next.memory.usagePercent,
        };
        historyRef.current = [...historyRef.current.slice(-59), point];
        setHistory(historyRef.current);
      } catch (e) {
        console.error(e);
      }
    }

    void poll();
    const id = window.setInterval(() => void poll(), 1500);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, []);

  const down = stats?.network.reduce((a, n) => a + n.receivedBytes, 0) ?? 0;
  const up = stats?.network.reduce((a, n) => a + n.transmittedBytes, 0) ?? 0;
  const cpu = stats?.cpuUsagePercent ?? 0;
  const mem = stats?.memory ?? info?.memory;

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Dashboard</h1>
          <p className="text-sm text-slate-500">
            {info
              ? `${info.os.name} ${info.os.version} · up ${formatUptime(info.os.uptimeSeconds)}`
              : "Loading system information…"}
          </p>
        </div>
        {info && (
          <span className="text-xs text-slate-500">
            {info.os.hostName}
          </span>
        )}
      </header>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Stat
          label="CPU Usage"
          value={`${cpu.toFixed(1)}%`}
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

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
        <Card title="CPU & Memory History" className="xl:col-span-2">
          <div className="h-64">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={history} margin={{ top: 4, right: 8, left: -16, bottom: 0 }}>
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
                <YAxis domain={[0, 100]} stroke="#475569" fontSize={11} tickLine={false} />
                <Tooltip
                  contentStyle={{
                    background: "#0f172a",
                    border: "1px solid #1e293b",
                    borderRadius: 8,
                    fontSize: 12,
                  }}
                />
                <Area type="monotone" dataKey="cpu" name="CPU %" stroke="#22d3ee" strokeWidth={2} fill="url(#cpuFill)" isAnimationActive={false} />
                <Area type="monotone" dataKey="ram" name="RAM %" stroke="#a78bfa" strokeWidth={2} fill="url(#ramFill)" isAnimationActive={false} />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </Card>

        <Card title="Per-Core Usage">
          <div className="space-y-2">
            {(stats?.perCoreUsage ?? []).length === 0 && (
              <p className="text-sm text-slate-500">Collecting core data…</p>
            )}
            {(stats?.perCoreUsage ?? []).map((usage, i) => (
              <div key={i} className="flex items-center gap-3">
                <span className="w-14 shrink-0 text-xs text-slate-400">
                  Core {i}
                </span>
                <ProgressBar value={usage} tone="cyan" className="flex-1" />
                <span className="w-12 shrink-0 text-right text-xs tabular-nums text-slate-400">
                  {usage.toFixed(0)}%
                </span>
              </div>
            ))}
          </div>
        </Card>
      </div>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
        <Card title="CPU">
          {info ? (
            <div className="space-y-2 text-sm">
              <Row label="Model" value={info.cpu.brand || info.cpu.name} />
              <Row label="Cores" value={`${info.cpu.physicalCores} physical / ${info.cpu.logicalCores} logical`} />
              <Row label="Clock" value={formatFrequency(info.cpu.frequencyMhz)} />
            </div>
          ) : (
            <Loading />
          )}
        </Card>
        <Card title="GPU">
          {info && info.gpus.length > 0 ? (
            <div className="space-y-2 text-sm">
              <Row label="Adapter" value={info.gpus[0].name} />
              <Row label="Driver" value={info.gpus[0].driverVersion || "—"} />
              <Row label="Vendor" value={info.gpus[0].vendor || "—"} />
            </div>
          ) : (
            <p className="text-sm text-slate-500">
              {info ? "No discrete GPU detected" : "Loading…"}
            </p>
          )}
        </Card>
        <Card title="Display & Storage">
          {info ? (
            <div className="space-y-2 text-sm">
              <div className="flex items-center gap-2 text-slate-400">
                <Monitor className="h-4 w-4" />
                {info.displays.length > 0
                  ? `${info.displays[0].width}×${info.displays[0].height} @ ${info.displays[0].refreshRate} Hz`
                  : "Display info unavailable"}
              </div>
              <div className="flex items-center gap-2 text-slate-400">
                <HardDrive className="h-4 w-4" />
                {info.disks.length > 0
                  ? `${info.disks.length} storage device${info.disks.length === 1 ? "" : "s"}`
                  : "No storage detected"}
              </div>
              {info.disks.map((d) => (
                <div key={d.mountPoint} className="flex items-center gap-3">
                  <span className="w-24 shrink-0 truncate text-xs text-slate-400">
                    {d.mountPoint}
                  </span>
                  <ProgressBar
                    value={(d.usedBytes / Math.max(1, d.totalBytes)) * 100}
                    tone="violet"
                    className="flex-1"
                  />
                  <span className="w-20 shrink-0 text-right text-xs tabular-nums text-slate-400">
                    {formatBytes(d.totalBytes)}
                  </span>
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
