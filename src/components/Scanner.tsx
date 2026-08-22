import { useCallback, useEffect, useState } from "react";
import {
  AlertTriangle,
  CircuitBoard,
  Cpu,
  Download,
  Gpu,
  HardDrive,
  MemoryStick,
  Monitor,
  Network,
  RefreshCw,
  Thermometer,
} from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { exportSystemReport, scanSystem } from "../lib/api";
import { formatBytes, formatFrequency } from "../lib/format";
import type { HardwareInfo, ProcessInfo } from "../lib/types";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";

type Tone = "slate" | "emerald" | "amber" | "violet" | "cyan" | "rose";

function healthTone(status: string): Tone {
  switch (status) {
    case "Healthy":
      return "emerald";
    case "Warning":
      return "amber";
    case "Unhealthy":
      return "rose";
    default:
      return "slate";
  }
}

function mediaTone(media: string): Tone {
  switch (media) {
    case "SSD":
      return "cyan";
    case "HDD":
      return "amber";
    case "SCM":
      return "violet";
    default:
      return "slate";
  }
}

export function Scanner() {
  const [info, setInfo] = useState<HardwareInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportedPath, setExportedPath] = useState<string | null>(null);

  const rescan = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setInfo(await scanSystem());
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void rescan();
  }, [rescan]);

  async function onExport(format: "html" | "json") {
    const ext = format === "html" ? "html" : "json";
    const stamp = new Date().toISOString().slice(0, 10);
    const selected = await save({
      title: "Export system report",
      defaultPath: `optix-system-report-${stamp}.${ext}`,
      filters: [{ name: format.toUpperCase(), extensions: [ext] }],
    });
    if (!selected) return;
    setExporting(true);
    setError(null);
    setExportedPath(null);
    try {
      const result = await exportSystemReport(selected, format);
      setExportedPath(result.path);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setExporting(false);
    }
  }

  const processes = [...(info?.processes ?? [])]
    .sort((a, b) => b.cpuUsagePercent - a.cpuUsagePercent)
    .slice(0, 20);

  const showWin10Banner =
    info != null && info.os.buildNumber != null && !info.os.isWindows11;

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">System Scanner</h1>
          <p className="text-sm text-slate-500">
            {info
              ? `Last scan ${new Date(info.scannedAtMs).toLocaleString()}`
              : "Scanning hardware and software…"}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <button
            onClick={() => void onExport("json")}
            disabled={loading || exporting || info == null}
            className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
          >
            <Download className="h-4 w-4" />
            {exporting ? "Exporting…" : "Export JSON"}
          </button>
          <button
            onClick={() => void onExport("html")}
            disabled={loading || exporting || info == null}
            className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
          >
            <Download className="h-4 w-4" />
            Export HTML
          </button>
          <button
            onClick={rescan}
            disabled={loading || exporting}
            className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
          >
            <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
            Rescan
          </button>
        </div>
      </header>

      {exportedPath && (
        <div className="flex items-start gap-2 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-200">
          <Download className="mt-0.5 h-4 w-4 shrink-0 text-emerald-400" />
          <span className="min-w-0">
            Report exported to <span className="break-all font-mono text-xs">{exportedPath}</span>
          </span>
        </div>
      )}

      {error && (
        <div className="rounded-lg border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
          {error}
        </div>
      )}

      {loading && !info && (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <div
              key={i}
              className="animate-pulse rounded-lg border border-slate-800 bg-slate-900/50 p-4"
            >
              <div className="h-4 w-24 rounded bg-slate-800" />
              <div className="mt-3 h-4 w-3/4 rounded bg-slate-800/70" />
              <div className="mt-2 h-4 w-1/2 rounded bg-slate-800/50" />
            </div>
          ))}
        </div>
      )}

      {showWin10Banner && (
        <div className="flex items-start gap-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3 text-sm text-amber-200">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <div>
            Windows 10 reached end of support. Optix works, but Windows 11 is
            recommended for gaming — it matches or beats Windows 10 once the
            defaults are fixed.
          </div>
        </div>
      )}

      {info && (
        <>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
            <Card title="CPU">
              <div className="flex items-start gap-3">
                <Cpu className="mt-0.5 h-5 w-5 shrink-0 text-cyan-400" />
                <div className="min-w-0 text-sm">
                  <div className="truncate font-medium text-slate-200">
                    {info.cpu.brand || info.cpu.name}
                  </div>
                  <div className="mt-1 space-y-1 text-slate-500">
                    <KV k="Cores" v={`${info.cpu.physicalCores} physical / ${info.cpu.logicalCores} logical`} />
                    <KV k="Clock" v={formatFrequency(info.cpu.frequencyMhz)} />
                    <KV k="Vendor" v={info.cpu.vendor} />
                    <KV k="Usage" v={`${info.cpu.usagePercent.toFixed(1)}%`} />
                  </div>
                </div>
              </div>
            </Card>

            <Card title="GPU">
              {info.gpus.length > 0 ? (
                <div className="space-y-3">
                  {info.gpus.map((g, i) => (
                    <div key={i} className="flex items-start gap-3">
                      <Gpu className="mt-0.5 h-5 w-5 shrink-0 text-violet-400" />
                      <div className="min-w-0 flex-1 text-sm">
                        <div className="truncate font-medium text-slate-200">
                          {g.name}
                        </div>
                        <div className="mt-1 space-y-1 text-slate-500">
                          <KV k="Driver" v={g.driverVersion || "—"} />
                          <KV k="VRAM" v={g.memoryBytes > 0 ? formatBytes(g.memoryBytes) : "—"} />
                          <KV k="Vendor" v={g.vendor || "—"} />
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="text-sm text-slate-500">No graphics adapters detected</p>
              )}
            </Card>

            <Card title="Memory">
              <div className="flex items-start gap-3">
                <MemoryStick className="mt-0.5 h-5 w-5 shrink-0 text-emerald-400" />
                <div className="text-sm">
                  <div className="font-medium text-slate-200">
                    {formatBytes(info.memory.totalBytes)}
                  </div>
                  <div className="mt-1 space-y-1 text-slate-500">
                    <KV k="Used" v={formatBytes(info.memory.usedBytes)} />
                    <KV k="Available" v={formatBytes(info.memory.availableBytes)} />
                    <KV k="Usage" v={`${info.memory.usagePercent.toFixed(0)}%`} />
                  </div>
                </div>
              </div>
            </Card>

            <Card title="Storage">
              <div className="space-y-3">
                {info.disks.map((d) => (
                  <div key={d.mountPoint} className="flex items-start gap-3">
                    <HardDrive className="mt-0.5 h-5 w-5 shrink-0 text-amber-400" />
                    <div className="min-w-0 flex-1 text-sm">
                      <div className="flex items-center justify-between gap-2">
                        <span className="truncate font-medium text-slate-200">
                          {d.name || d.mountPoint}
                        </span>
                        <Badge tone={d.kind === "SSD" ? "cyan" : d.kind === "HDD" ? "amber" : "slate"}>
                          {d.kind}
                        </Badge>
                      </div>
                      <div className="mt-1 space-y-1 text-slate-500">
                        <KV k="Capacity" v={formatBytes(d.totalBytes)} />
                        <KV k="Free" v={formatBytes(d.availableBytes)} />
                        <KV k="FS" v={`${d.fileSystem} · ${d.mountPoint}`} />
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </Card>

            <Card title="Displays">
              {info.displays.length > 0 ? (
                <div className="space-y-3">
                  {info.displays.map((d, i) => (
                    <div key={i} className="flex items-start gap-3">
                      <Monitor className="mt-0.5 h-5 w-5 shrink-0 text-cyan-400" />
                      <div className="text-sm">
                        <div className="font-medium text-slate-200">
                          {d.name || `Display ${i + 1}`}
                        </div>
                        <div className="mt-1 text-slate-500">
                          {d.width}×{d.height} · {d.refreshRate > 0 ? `${d.refreshRate} Hz` : "refresh rate unavailable"}
                          {d.isPrimary ? " · Primary" : ""}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="text-sm text-slate-500">Display info unavailable</p>
              )}
            </Card>

            <Card title="Network">
              {info.network.length > 0 ? (
                <div className="space-y-3">
                  {info.network.map((n) => (
                    <div key={n.name} className="flex items-start gap-3">
                      <Network className="mt-0.5 h-5 w-5 shrink-0 text-emerald-400" />
                      <div className="min-w-0 flex-1 text-sm">
                        <div className="truncate font-medium text-slate-200">{n.name}</div>
                        <div className="mt-1 space-y-1 text-slate-500">
                          <KV k="Received" v={formatBytes(n.totalReceivedBytes)} />
                          <KV k="Sent" v={formatBytes(n.totalTransmittedBytes)} />
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="text-sm text-slate-500">No network interfaces detected</p>
              )}
            </Card>

            <Card title="Temperatures">
              {info.temperatures.length > 0 ? (
                <div className="space-y-3">
                  {info.temperatures.map((t, i) => (
                    <div key={i} className="flex items-start gap-3">
                      <Thermometer className="mt-0.5 h-5 w-5 shrink-0 text-rose-400" />
                      <div className="min-w-0 flex-1 text-sm">
                        <div className="flex min-w-0 items-center justify-between gap-2">
                          <span
                            className="min-w-0 flex-1 truncate text-slate-200"
                            title={t.label}
                          >
                            {t.label}
                          </span>
                          <span className="shrink-0 tabular-nums text-slate-400">
                            {t.celsius != null ? `${t.celsius.toFixed(0)}°C` : "—"}
                          </span>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="text-sm text-slate-500">No sensors available</p>
              )}
            </Card>
          </div>

          {info.physicalDisks.length > 0 && (
            <Card title="Physical Disks">
              <div className="space-y-3">
                {info.physicalDisks.map((d, i) => (
                  <div key={i} className="flex items-start gap-3">
                    <HardDrive className="mt-0.5 h-5 w-5 shrink-0 text-amber-400" />
                    <div className="min-w-0 flex-1 text-sm">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="truncate font-medium text-slate-200">
                          {d.friendlyName || d.busType}
                        </span>
                        <Badge tone={healthTone(d.healthStatus)}>{d.healthStatus}</Badge>
                        <Badge tone={mediaTone(d.mediaType)}>{d.mediaType}</Badge>
                        <Badge tone="slate">{d.busType}</Badge>
                      </div>
                      <div className="mt-1 space-y-1 text-slate-500">
                        <KV k="Capacity" v={formatBytes(d.sizeBytes)} />
                        {d.firmwareVersion && <KV k="Firmware" v={d.firmwareVersion} />}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </Card>
          )}

          <Card title="Motherboard & BIOS">
            {info.motherboard || info.bios ? (
              <div className="flex items-start gap-3">
                <CircuitBoard className="mt-0.5 h-5 w-5 shrink-0 text-slate-400" />
                <div className="flex-1 space-y-1 text-sm">
                  {info.motherboard ? (
                    <KV
                      k="Board"
                      v={`${info.motherboard.manufacturer} ${info.motherboard.product}`.trim()}
                    />
                  ) : (
                    <KV k="Board" v="—" />
                  )}
                  {info.bios ? (
                    <KV k="BIOS" v={`${info.bios.vendor} ${info.bios.version}`.trim()} />
                  ) : (
                    <KV k="BIOS" v="—" />
                  )}
                </div>
              </div>
            ) : (
              <p className="text-sm text-slate-500">
                Unavailable — board/DMI information could not be read on this system.
              </p>
            )}
          </Card>

          <Card title="Operating System">
            <div className="grid grid-cols-1 gap-x-6 gap-y-1 text-sm sm:grid-cols-2 lg:grid-cols-4">
              <KV k="OS" v={info.os.edition || info.os.name} />
              <KV k="Version" v={info.os.version} />
              <KV
                k="Build"
                v={info.os.buildNumber != null ? String(info.os.buildNumber) : "—"}
              />
              <KV k="Host" v={info.os.hostName} />
            </div>
          </Card>

          <Card title="Top Processes by CPU">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-800 text-left text-xs uppercase tracking-wider text-slate-500">
                  <th className="py-2 pr-2 font-medium">Name</th>
                  <th className="py-2 pr-2 font-medium">PID</th>
                  <th className="py-2 pr-2 font-medium">CPU</th>
                  <th className="py-2 font-medium">Memory</th>
                </tr>
              </thead>
              <tbody>
                {processes.map((p: ProcessInfo) => (
                  <tr key={p.pid} className="border-b border-slate-800/60 last:border-0">
                    <td className="max-w-0 truncate py-2 pr-2 text-slate-200">{p.name}</td>
                    <td className="py-2 pr-2 tabular-nums text-slate-400">{p.pid}</td>
                    <td className="py-2 pr-2 tabular-nums text-cyan-400">
                      {p.cpuUsagePercent.toFixed(1)}%
                    </td>
                    <td className="py-2 tabular-nums text-slate-400">
                      {formatBytes(p.memoryBytes)}
                    </td>
                  </tr>
                ))}
                {processes.length === 0 && (
                  <tr>
                    <td colSpan={4} className="py-4 text-slate-500">
                      No process data available
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </Card>

          <Card title={`Startup Applications (${info.startupApps.length})`}>
            {info.startupApps.length > 0 ? (
              <ul className="divide-y divide-slate-800/60">
                {info.startupApps.map((a) => (
                  <li key={`${a.location}-${a.name}`} className="flex items-center justify-between gap-4 py-2 text-sm">
                    <div className="min-w-0">
                      <div className="truncate text-slate-200">{a.name}</div>
                      <div className="truncate text-xs text-slate-500">{a.command}</div>
                    </div>
                    <Badge tone="slate">{a.location}</Badge>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-sm text-slate-500">No startup entries found.</p>
            )}
          </Card>
        </>
      )}
    </div>
  );
}

function KV({ k, v }: { k: string; v: string }) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <span className="text-slate-500">{k}</span>
      <span className="truncate text-right text-slate-300">{v}</span>
    </div>
  );
}
