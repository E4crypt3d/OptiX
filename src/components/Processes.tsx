import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Crosshair,
  Gamepad2,
  Pause,
  Play,
  RefreshCw,
  ShieldCheck,
  Skull,
  Undo2,
} from "lucide-react";
import {
  applyGamingMode,
  foregroundPid,
  getProcessAffinity,
  killProcess,
  listProcesses,
  memoryState,
  resumeProcess,
  restoreGamingMode,
  setProcessAffinity,
  setProcessPriority,
  suspendProcess,
} from "../lib/api";
import { formatBytes } from "../lib/format";
import { useInterval } from "../lib/useInterval";
import type {
  AffinityInfo,
  MemoryState,
  PriorityClass,
  ProcessClass,
  ProcessDetail,
} from "../lib/types";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";

const CLASS_TONE: Record<ProcessClass, "rose" | "emerald" | "slate"> = {
  required: "rose",
  safe: "emerald",
  unknown: "slate",
};

const PRIORITIES: PriorityClass[] = [
  "idle",
  "below_normal",
  "normal",
  "above_normal",
  "high",
];
const MAX_VISIBLE_PROCESSES = 200;

type SortKey = "name" | "cpu" | "ram" | "disk" | "threads";

function pressureTone(pressure: MemoryState["pressure"]): "emerald" | "amber" | "rose" {
  return pressure === "normal" ? "emerald" : pressure === "elevated" ? "amber" : "rose";
}

function statusBadge(status: string) {
  switch (status) {
    case "stop":
      return <Badge tone="violet">suspended</Badge>;
    case "zombie":
      return <Badge tone="rose">zombie</Badge>;
    case "dead":
      return <Badge tone="rose">dead</Badge>;
    case "idle":
      return <Badge tone="slate">idle</Badge>;
    default:
      return null;
  }
}

function running(status: string): boolean {
  return status !== "dead" && status !== "zombie";
}

export function Processes() {
  const [processes, setProcesses] = useState<ProcessDetail[]>([]);
  const [memory, setMemory] = useState<MemoryState | null>(null);
  const [loading, setLoading] = useState(false);
  const [busyPid, setBusyPid] = useState<number | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [filter, setFilter] = useState<ProcessClass | "all">("all");
  const [search, setSearch] = useState("");
  const [sortKey, setSortKey] = useState<SortKey | null>(null);
  const [sortDir, setSortDir] = useState<"asc" | "desc">("desc");
  const [gamePids, setGamePids] = useState<Set<number>>(new Set());
  const [bgPids, setBgPids] = useState<Set<number>>(new Set());
  const [affinity, setAffinity] = useState<{ pid: number; info: AffinityInfo; draft: number } | null>(
    null,
  );
  const [detectBusy, setDetectBusy] = useState(false);
  const isWindows =
    typeof navigator !== "undefined" && /windows|win32/i.test(navigator.userAgent);

  const refresh = useCallback(async (quiet = false) => {
    if (!quiet) setLoading(true);
    try {
      const [next, mem] = await Promise.all([listProcesses(), memoryState()]);
      const available = new Set(next.map((process) => process.pid));
      setProcesses(next);
      setMemory(mem);
      setGamePids((previous) => new Set([...previous].filter((pid) => available.has(pid))));
      setBgPids((previous) => new Set([...previous].filter((pid) => available.has(pid))));
      setAffinity((previous) =>
        previous && available.has(previous.pid) ? previous : null,
      );
    } catch (e) {
      if (!quiet) setError(errMsg(e));
    } finally {
      if (!quiet) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Live monitoring: refresh every 5 s while the page is visible; the hook
  // pauses entirely when the window is hidden/minimized.
  useInterval(() => void refresh(true), 5000);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    return processes.filter((p) => {
      if (filter !== "all" && p.classification !== filter) return false;
      if (q && !p.name.toLowerCase().includes(q) && !p.exe.toLowerCase().includes(q)) {
        return false;
      }
      return true;
    });
  }, [processes, filter, search]);

  const sorted = useMemo(() => {
    if (!sortKey) return filtered;
    const dir = sortDir === "asc" ? 1 : -1;
    return [...filtered].sort((a, b) => {
      let cmp = 0;
      switch (sortKey) {
        case "name":
          cmp = a.name.localeCompare(b.name);
          break;
        case "cpu":
          cmp = a.cpuUsagePercent - b.cpuUsagePercent;
          break;
        case "ram":
          cmp = a.memoryBytes - b.memoryBytes;
          break;
        case "disk":
          cmp = a.diskReadBytes - b.diskReadBytes;
          break;
        case "threads":
          cmp = a.threads - b.threads;
          break;
      }
      return cmp * dir;
    });
  }, [filtered, sortKey, sortDir]);

  const visible = sorted.slice(0, MAX_VISIBLE_PROCESSES);
  const busy = loading || busyPid !== null || busyAction !== null || detectBusy;

  function toggleSort(key: SortKey) {
    if (sortKey === key) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDir(key === "name" ? "asc" : "desc");
    }
  }

  const canControl = (p: ProcessDetail) =>
    isWindows && !p.isSystem && p.classification !== "required" && running(p.status);
  const canSuspend = (p: ProcessDetail) =>
    !p.isSystem && p.classification !== "required" && running(p.status);

  async function onKill(p: ProcessDetail) {
    if (!canControl(p)) return;
    if (!window.confirm(`Terminate ${p.name} (PID ${p.pid})? Unsaved work may be lost.`)) return;
    setBusyPid(p.pid);
    setError(null);
    setNotice(null);
    try {
      await killProcess(p.pid);
      setNotice(`Terminated ${p.name}.`);
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyPid(null);
    }
  }

  async function onSuspendToggle(p: ProcessDetail) {
    if (!canSuspend(p)) return;
    const suspended = p.status === "stop";
    if (
      !suspended &&
      !window.confirm(`Suspend ${p.name} (PID ${p.pid})? It will freeze in place until resumed.`)
    )
      return;
    setBusyPid(p.pid);
    setError(null);
    setNotice(null);
    try {
      if (suspended) {
        await resumeProcess(p.pid);
        setNotice(`Resumed ${p.name}.`);
      } else {
        await suspendProcess(p.pid);
        setNotice(`Suspended ${p.name} — it will not run until resumed.`);
      }
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyPid(null);
    }
  }

  async function onSetPriority(p: ProcessDetail, priority: PriorityClass) {
    if (!canControl(p) || priority === "realtime") return;
    setBusyPid(p.pid);
    setError(null);
    setNotice(null);
    try {
      await setProcessPriority(p.pid, priority);
      setNotice(`${p.name} → ${priority.replace("_", " ")}.`);
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyPid(null);
    }
  }

  async function onOpenAffinity(p: ProcessDetail) {
    setError(null);
    try {
      const info = await getProcessAffinity(p.pid);
      if (!info) {
        setError("CPU affinity is unavailable for this process (Windows only).");
        return;
      }
      setAffinity({ pid: p.pid, info, draft: info.processMask });
    } catch (e) {
      setError(errMsg(e));
    }
  }

  async function onApplyAffinity() {
    if (!affinity) return;
    setBusyPid(affinity.pid);
    setError(null);
    setNotice(null);
    try {
      await setProcessAffinity(affinity.pid, affinity.draft);
      const p = processes.find((x) => x.pid === affinity.pid);
      setNotice(`Pinned ${p?.name ?? `PID ${affinity.pid}`} to ${countBits(affinity.draft)} core(s).`);
      setAffinity(null);
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyPid(null);
    }
  }

  async function onApplyGamingMode() {
    if (!isWindows) {
      setError("Gaming mode priority changes are only available on Windows.");
      return;
    }
    if (gamePids.size === 0) {
      setError("Select at least one game process to boost.");
      return;
    }
    if ([...gamePids].some((pid) => bgPids.has(pid))) {
      setError("A process cannot be both game and background.");
      return;
    }
    setBusyAction("gaming");
    setError(null);
    setNotice(null);
    try {
      const result = await applyGamingMode([...gamePids], [...bgPids]);
      const total = result.boosted.length + result.lowered.length;
      setNotice(
        total === 0
          ? "No priority changes were applied. Processes may have exited or access was denied."
          : `Gaming mode applied: ${result.boosted.length} boosted, ${result.lowered.length} lowered.`,
      );
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyAction(null);
    }
  }

  async function onRestore() {
    if (!isWindows) {
      setError("Gaming mode priority changes are only available on Windows.");
      return;
    }
    setBusyAction("restore");
    setError(null);
    setNotice(null);
    try {
      const n = await restoreGamingMode();
      setNotice(n > 0 ? `Restored ${n} processes.` : "Nothing to restore.");
      setGamePids(new Set());
      setBgPids(new Set());
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyAction(null);
    }
  }

  async function onDetectGame() {
    setDetectBusy(true);
    setError(null);
    setNotice(null);
    try {
      const pid = await foregroundPid();
      const p = pid != null ? processes.find((x) => x.pid === pid) : undefined;
      if (!p) {
        setNotice(
          "No game window detected in the foreground — focus the game window and click again.",
        );
        return;
      }
      if (p.isSystem || p.classification === "required") {
        setNotice(`The foreground process (${p.name}) is a protected system process.`);
        return;
      }
      toggleGamePid(p.pid);
      setNotice(`Marked ${p.name} as the game. Add background apps, then Apply.`);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setDetectBusy(false);
    }
  }

  function toggleGamePid(pid: number) {
    setGamePids((previous) => {
      const next = new Set(previous);
      if (next.has(pid)) next.delete(pid);
      else next.add(pid);
      return next;
    });
    setBgPids((previous) => {
      const next = new Set(previous);
      next.delete(pid);
      return next;
    });
  }

  function toggleBackgroundPid(pid: number) {
    setBgPids((previous) => {
      const next = new Set(previous);
      if (next.has(pid)) next.delete(pid);
      else next.add(pid);
      return next;
    });
    setGamePids((previous) => {
      const next = new Set(previous);
      next.delete(pid);
      return next;
    });
  }

  return (
    <div className="space-y-4">
      <header className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Processes &amp; RAM</h1>
          <p className="text-sm text-slate-500">
            Classify, prioritize, suspend, and clean up running processes. REALTIME priority is never applied.
          </p>
        </div>
        <button
          onClick={() => void refresh()}
          disabled={busy}
          className="flex w-fit items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
        >
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          {loading ? "Refreshing…" : "Refresh"}
        </button>
      </header>

      {!isWindows && (
        <div className="flex items-start gap-2 rounded-xl border border-slate-800 bg-slate-900/30 px-4 py-3 text-sm text-slate-500">
          <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-slate-400" />
          Process priority changes, termination, and affinity are only available on Windows.
          Suspend/resume works on this platform too; the rest is read-only.
        </div>
      )}

      {error && (
        <div className="rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
          {error}
        </div>
      )}
      {notice && (
        <div className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-300">
          {notice}
        </div>
      )}

      <Card title="Memory">
        {memory ? (
          <div className="space-y-3">
            <div className="flex items-center justify-between gap-3">
              <span className="text-lg font-medium text-slate-100">
                {formatBytes(memory.totalBytes)}
              </span>
              <Badge tone={pressureTone(memory.pressure)}>{memory.pressure} pressure</Badge>
            </div>
            <div className="h-2 w-full overflow-hidden rounded-full bg-slate-800">
              <div
                className={`h-full rounded-full ${
                  memory.pressure === "critical"
                    ? "bg-rose-500"
                    : memory.pressure === "elevated"
                      ? "bg-amber-500"
                      : "bg-emerald-500"
                }`}
                style={{ width: `${Math.min(100, memory.usagePercent)}%` }}
              />
            </div>
            <div className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm sm:grid-cols-3">
              <KV k="Used" v={formatBytes(memory.usedBytes)} />
              <KV k="Available" v={formatBytes(memory.availableBytes)} />
              <KV k="Used %" v={`${memory.usagePercent.toFixed(0)}%`} />
              {memory.cachedBytes !== null && <KV k="Cached" v={formatBytes(memory.cachedBytes)} />}
              {memory.committedBytes !== null && memory.committedLimitBytes !== null && (
                <KV
                  k="Committed"
                  v={`${formatBytes(memory.committedBytes)} / ${formatBytes(memory.committedLimitBytes)}`}
                />
              )}
              {memory.swapTotalBytes > 0 && (
                <KV
                  k="Swap"
                  v={`${formatBytes(memory.swapUsedBytes)} / ${formatBytes(memory.swapTotalBytes)}`}
                />
              )}
            </div>
            <p className="text-xs leading-5 text-slate-600">
              Available memory already includes reclaimable cache. Cached/standby memory is
              released automatically when an app needs it — RAM "cleaners" that force it out
              provide no real benefit and only add churn.
            </p>
          </div>
        ) : loading ? (
          <div className="h-24 animate-pulse rounded-lg bg-slate-800/50" />
        ) : (
          <p className="text-sm text-slate-500">Reading memory…</p>
        )}
      </Card>

      <Card title="Gaming mode">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="min-w-0 text-sm text-slate-400">
            <p>
              Boost the selected game to <span className="text-slate-200">above-normal</span> and lower background processes to <span className="text-slate-200">below-normal</span>. Changes are recorded for restore.
            </p>
            <p className="mt-1 text-xs text-slate-500">
              Focus the game window and use <span className="text-slate-300">Detect active game</span>, or pick processes manually. A process can only belong to one group. System and required processes are protected.
            </p>
          </div>
          <div className="flex shrink-0 flex-wrap items-center gap-2">
            <button
              onClick={onDetectGame}
              disabled={!isWindows || busy}
              className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
            >
              <Crosshair className="h-4 w-4" />
              {detectBusy ? "Detecting…" : "Detect active game"}
            </button>
            <button
              onClick={onApplyGamingMode}
              disabled={!isWindows || busy || gamePids.size === 0}
              className="flex items-center gap-2 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
            >
              <Gamepad2 className="h-4 w-4" />
              {busyAction === "gaming" ? "Applying…" : "Apply"}
            </button>
            <button
              onClick={onRestore}
              disabled={!isWindows || busy}
              className="flex items-center gap-2 rounded-lg bg-slate-800 px-4 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
            >
              <Undo2 className="h-4 w-4" />
              {busyAction === "restore" ? "Restoring…" : "Restore"}
            </button>
          </div>
        </div>
      </Card>

      <Card
        title={`Processes (${filtered.length})`}
        action={
          <div className="flex flex-wrap items-center gap-2">
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Filter name or path…"
              aria-label="Filter processes"
              className="min-w-0 rounded-lg border border-slate-700 bg-slate-900 px-3 py-1.5 text-sm text-slate-200 placeholder-slate-500 focus:border-cyan-500 focus:outline-none"
            />
            <select
              value={filter}
              onChange={(e) => setFilter(e.target.value as ProcessClass | "all")}
              aria-label="Filter process classification"
              className="rounded-lg border border-slate-700 bg-slate-900 px-2 py-1.5 text-sm text-slate-200 focus:border-cyan-500 focus:outline-none"
            >
              <option value="all">All</option>
              <option value="required">Required</option>
              <option value="safe">Safe</option>
              <option value="unknown">Unknown</option>
            </select>
          </div>
        }
      >
        {loading && processes.length === 0 ? (
          <div className="space-y-3 py-2">
            {[0, 1, 2, 3].map((item) => (
              <div key={item} className="h-12 animate-pulse rounded-lg bg-slate-800/50" />
            ))}
          </div>
        ) : filtered.length === 0 ? (
          <p className="py-8 text-center text-sm text-slate-500">
            {processes.length === 0 ? "No process data available." : "No processes match the current filters."}
          </p>
        ) : (
          <>
            <div className="overflow-x-auto">
              <table className="min-w-[1180px] w-full text-sm">
                <thead>
                  <tr className="border-b border-slate-800 text-left text-xs uppercase tracking-wider text-slate-500">
                    <th className="py-2 pr-2 font-medium">Game</th>
                    <th className="py-2 pr-2 font-medium">Background</th>
                    <th className="py-2 pr-2 font-medium">
                      <SortButton label="Process" k="name" sortKey={sortKey} dir={sortDir} onSort={toggleSort} />
                    </th>
                    <th className="py-2 pr-2 text-right font-medium">
                      <SortButton label="CPU" k="cpu" sortKey={sortKey} dir={sortDir} onSort={toggleSort} />
                    </th>
                    <th className="py-2 pr-2 text-right font-medium">GPU</th>
                    <th className="py-2 pr-2 text-right font-medium">
                      <SortButton label="RAM" k="ram" sortKey={sortKey} dir={sortDir} onSort={toggleSort} />
                    </th>
                    <th className="py-2 pr-2 text-right font-medium">
                      <SortButton label="Disk read" k="disk" sortKey={sortKey} dir={sortDir} onSort={toggleSort} />
                    </th>
                    <th className="py-2 pr-2 text-right font-medium">
                      <SortButton label="Threads" k="threads" sortKey={sortKey} dir={sortDir} onSort={toggleSort} />
                    </th>
                    <th className="py-2 pr-2 font-medium">Priority</th>
                    <th className="py-2 font-medium">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {visible.map((p) => {
                    const control = canControl(p);
                    const suspend = canSuspend(p);
                    const suspended = p.status === "stop";
                    const priority = p.priority && p.priority !== "realtime" ? p.priority : "unknown";
                    return (
                      <tr key={p.pid} className="border-b border-slate-800/50 hover:bg-slate-900/40">
                        <td className="py-2 pr-2">
                          <input
                            type="checkbox"
                            checked={gamePids.has(p.pid)}
                            disabled={!control || busy}
                            onChange={() => toggleGamePid(p.pid)}
                            aria-label={`Mark ${p.name} as game process`}
                            className="h-4 w-4 accent-cyan-500 disabled:opacity-30"
                          />
                        </td>
                        <td className="py-2 pr-2">
                          <input
                            type="checkbox"
                            checked={bgPids.has(p.pid)}
                            disabled={!control || busy}
                            onChange={() => toggleBackgroundPid(p.pid)}
                            aria-label={`Mark ${p.name} as background process`}
                            className="h-4 w-4 accent-amber-500 disabled:opacity-30"
                          />
                        </td>
                        <td className="max-w-[240px] py-2 pr-2">
                          <div className="flex items-center gap-2">
                            <span className="truncate font-medium text-slate-200" title={p.name}>{p.name}</span>
                            <Badge tone={CLASS_TONE[p.classification]}>{p.classification}</Badge>
                            {p.isSystem && <Badge tone="rose">system</Badge>}
                            {p.userId === 0 && !p.isSystem && <Badge tone="amber">root</Badge>}
                            {statusBadge(p.status)}
                          </div>
                          <div className="truncate text-xs text-slate-500" title={p.exe}>
                            PID {p.pid} · {p.exe || p.status}
                          </div>
                        </td>
                        <td className="py-2 pr-2 text-right tabular-nums text-slate-300">{p.cpuUsagePercent.toFixed(1)}%</td>
                        <td className="py-2 pr-2 text-right tabular-nums text-slate-300">
                          {p.gpuUsagePercent > 0 ? `${p.gpuUsagePercent.toFixed(1)}%` : "—"}
                        </td>
                        <td className="py-2 pr-2 text-right tabular-nums text-slate-300">{formatBytes(p.memoryBytes)}</td>
                        <td className="py-2 pr-2 text-right tabular-nums text-slate-500">{formatBytes(p.diskReadBytes)}</td>
                        <td className="py-2 pr-2 text-right tabular-nums text-slate-500">
                          {p.threads > 0 ? p.threads : "—"}
                        </td>
                        <td className="py-2 pr-2">
                          {p.priority === "realtime" ? (
                            <span className="text-xs text-amber-300" title="REALTIME was set outside Optix and cannot be applied by Optix">
                              realtime (read-only)
                            </span>
                          ) : (
                            <select
                              value={priority}
                              disabled={!control || busy}
                              onChange={(e) => onSetPriority(p, e.target.value as PriorityClass)}
                              className="rounded-lg border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-200 focus:border-cyan-500 focus:outline-none disabled:opacity-50"
                            >
                              {p.priority === null && <option value="unknown">unavailable</option>}
                              {PRIORITIES.map((pr) => (
                                <option key={pr} value={pr}>{pr.replace("_", " ")}</option>
                              ))}
                            </select>
                          )}
                        </td>
                        <td className="py-2 pr-2">
                          <div className="flex items-center justify-end gap-1">
                            {isWindows && (
                              <button
                                onClick={() => onOpenAffinity(p)}
                                disabled={!control || busy}
                                title={control ? "Pin to CPU cores" : "Unavailable for this process"}
                                className="rounded-lg p-1.5 text-slate-400 transition-colors hover:bg-cyan-500/10 hover:text-cyan-300 disabled:opacity-30"
                              >
                                <Crosshair className="h-4 w-4" />
                              </button>
                            )}
                            <button
                              onClick={() => onSuspendToggle(p)}
                              disabled={!suspend || busy}
                              title={
                                suspend
                                  ? suspended
                                    ? "Resume process"
                                    : "Suspend process"
                                  : "Protected or unavailable on this platform"
                              }
                              className="rounded-lg p-1.5 text-slate-400 transition-colors hover:bg-violet-500/10 hover:text-violet-300 disabled:opacity-30"
                            >
                              {suspended ? <Play className="h-4 w-4" /> : <Pause className="h-4 w-4" />}
                            </button>
                            <button
                              onClick={() => onKill(p)}
                              disabled={!control || busy}
                              title={control ? "Terminate process" : "Protected or unavailable on this platform"}
                              className="rounded-lg p-1.5 text-slate-400 transition-colors hover:bg-rose-500/10 hover:text-rose-400 disabled:opacity-30"
                            >
                              <Skull className="h-4 w-4" />
                            </button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>

            {affinity && (
              <div className="mt-3 rounded-lg border border-cyan-500/30 bg-cyan-500/5 p-4">
                <div className="flex items-center justify-between gap-3">
                  <span className="text-sm font-medium text-slate-200">
                    Pin {processes.find((x) => x.pid === affinity.pid)?.name ?? `PID ${affinity.pid}`} to cores
                  </span>
                  <button onClick={() => setAffinity(null)} className="text-xs text-slate-500 hover:text-slate-300">
                    Cancel
                  </button>
                </div>
                <div className="mt-3 flex flex-wrap gap-2">
                  {coresOf(affinity.info.systemMask).map((core) => {
                    const on = (affinity.draft & (1 << core)) !== 0;
                    return (
                      <button
                        key={core}
                        onClick={() =>
                          setAffinity((a) =>
                            a ? { ...a, draft: a.draft ^ (1 << core) } : a,
                          )
                        }
                        className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
                          on
                            ? "bg-cyan-600 text-white"
                            : "bg-slate-800 text-slate-400 hover:bg-slate-700"
                        }`}
                      >
                        {core}
                      </button>
                    );
                  })}
                </div>
                <div className="mt-3 flex justify-end gap-2">
                  <button
                    onClick={() =>
                      setAffinity((a) => (a ? { ...a, draft: a.info.systemMask } : a))
                    }
                    className="rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-medium text-slate-200 hover:bg-slate-700"
                  >
                    All cores
                  </button>
                  <button
                    onClick={onApplyAffinity}
                    disabled={affinity.draft === 0 || busyPid !== null}
                    className="rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-cyan-500 disabled:opacity-50"
                  >
                    {busyPid === affinity.pid ? "Applying…" : "Apply"}
                  </button>
                </div>
              </div>
            )}

            {filtered.length > MAX_VISIBLE_PROCESSES && (
              <p className="mt-3 text-xs text-slate-500">
                Showing the first {MAX_VISIBLE_PROCESSES} of {filtered.length}. Refine the filter or sort to see more.
              </p>
            )}
          </>
        )}
      </Card>
    </div>
  );
}

function SortButton({
  label,
  k,
  sortKey,
  dir,
  onSort,
}: {
  label: string;
  k: SortKey;
  sortKey: SortKey | null;
  dir: "asc" | "desc";
  onSort: (k: SortKey) => void;
}) {
  const active = sortKey === k;
  return (
    <button
      onClick={() => onSort(k)}
      className={`uppercase tracking-wider transition-colors hover:text-slate-300 ${
        active ? "text-slate-300" : ""
      }`}
    >
      {label}
      {active ? (dir === "asc" ? " ↑" : " ↓") : ""}
    </button>
  );
}

function coresOf(mask: number): number[] {
  const cores: number[] = [];
  for (let i = 0; i < 64; i += 1) {
    if ((mask & (1 << i)) !== 0) cores.push(i);
  }
  return cores;
}

function countBits(mask: number): number {
  let n = 0;
  for (let i = 0; i < 64; i += 1) if ((mask & (1 << i)) !== 0) n += 1;
  return n;
}

function KV({ k, v }: { k: string; v: string }) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <span className="text-slate-500">{k}</span>
      <span className="truncate text-right text-slate-300">{v}</span>
    </div>
  );
}
