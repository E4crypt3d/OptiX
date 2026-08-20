import { useCallback, useEffect, useMemo, useState } from "react";
import { Gamepad2, RefreshCw, ShieldCheck, Skull, Undo2 } from "lucide-react";
import {
  applyGamingMode,
  killProcess,
  listProcesses,
  restoreGamingMode,
  setProcessPriority,
} from "../lib/api";
import { formatBytes } from "../lib/format";
import type { PriorityClass, ProcessClass, ProcessDetail } from "../lib/types";
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

function isActionable(p: ProcessDetail, isWindows: boolean): boolean {
  return isWindows && !p.isSystem && p.classification !== "required";
}

export function Processes() {
  const [processes, setProcesses] = useState<ProcessDetail[]>([]);
  const [loading, setLoading] = useState(false);
  const [busyPid, setBusyPid] = useState<number | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [filter, setFilter] = useState<ProcessClass | "all">("all");
  const [search, setSearch] = useState("");
  const [gamePids, setGamePids] = useState<Set<number>>(new Set());
  const [bgPids, setBgPids] = useState<Set<number>>(new Set());
  const isWindows =
    typeof navigator !== "undefined" && /windows|win32/i.test(navigator.userAgent);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const next = await listProcesses();
      const available = new Set(next.map((process) => process.pid));
      setProcesses(next);
      setGamePids((previous) => new Set([...previous].filter((pid) => available.has(pid))));
      setBgPids((previous) => new Set([...previous].filter((pid) => available.has(pid))));
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

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

  const visible = filtered.slice(0, MAX_VISIBLE_PROCESSES);
  const busy = loading || busyPid !== null || busyAction !== null;

  async function onKill(p: ProcessDetail) {
    if (!isActionable(p, isWindows)) return;
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

  async function onSetPriority(p: ProcessDetail, priority: PriorityClass) {
    if (!isActionable(p, isWindows) || priority === "realtime") return;
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
            Classify, prioritize, and clean up running processes. REALTIME priority is never applied.
          </p>
        </div>
        <button
          onClick={refresh}
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
          Process priority changes and termination are only available on Windows. Process data remains read-only here.
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

      <Card title="Gaming mode">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="min-w-0 text-sm text-slate-400">
            <p>
              Boost selected game processes to <span className="text-slate-200">above-normal</span> and lower selected background processes to <span className="text-slate-200">below-normal</span>. Changes are recorded for restore.
            </p>
            <p className="mt-1 text-xs text-slate-500">
              A process can only belong to one group. System and required processes are protected.
            </p>
          </div>
          <div className="flex shrink-0 flex-wrap items-center gap-2">
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
              <table className="min-w-[980px] w-full text-sm">
                <thead>
                  <tr className="border-b border-slate-800 text-left text-xs uppercase tracking-wider text-slate-500">
                    <th className="py-2 pr-2 font-medium">Game</th>
                    <th className="py-2 pr-2 font-medium">Background</th>
                    <th className="py-2 pr-2 font-medium">Process</th>
                    <th className="py-2 pr-2 text-right font-medium">CPU</th>
                    <th className="py-2 pr-2 text-right font-medium">GPU</th>
                    <th className="py-2 pr-2 text-right font-medium">RAM</th>
                    <th className="py-2 pr-2 text-right font-medium">Disk read</th>
                    <th className="py-2 pr-2 font-medium">Priority</th>
                    <th className="py-2 pr-2 font-medium">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {visible.map((p) => {
                    const actionable = isActionable(p, isWindows);
                    const priority = p.priority && p.priority !== "realtime" ? p.priority : "unknown";
                    return (
                      <tr key={p.pid} className="border-b border-slate-800/50 hover:bg-slate-900/40">
                        <td className="py-2 pr-2">
                          <input
                            type="checkbox"
                            checked={gamePids.has(p.pid)}
                            disabled={!actionable || busy}
                            onChange={() => toggleGamePid(p.pid)}
                            aria-label={`Mark ${p.name} as game process`}
                            className="h-4 w-4 accent-cyan-500 disabled:opacity-30"
                          />
                        </td>
                        <td className="py-2 pr-2">
                          <input
                            type="checkbox"
                            checked={bgPids.has(p.pid)}
                            disabled={!actionable || busy}
                            onChange={() => toggleBackgroundPid(p.pid)}
                            aria-label={`Mark ${p.name} as background process`}
                            className="h-4 w-4 accent-amber-500 disabled:opacity-30"
                          />
                        </td>
                        <td className="max-w-[260px] py-2 pr-2">
                          <div className="flex items-center gap-2">
                            <span className="truncate font-medium text-slate-200" title={p.name}>{p.name}</span>
                            <Badge tone={CLASS_TONE[p.classification]}>{p.classification}</Badge>
                            {p.isSystem && <Badge tone="rose">system</Badge>}
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
                        <td className="py-2 pr-2">
                          {p.priority === "realtime" ? (
                            <span className="text-xs text-amber-300" title="REALTIME was set outside Optix and cannot be applied by Optix">
                              realtime (read-only)
                            </span>
                          ) : (
                            <select
                              value={priority}
                              disabled={!actionable || busy}
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
                          <button
                            onClick={() => onKill(p)}
                            disabled={!actionable || busy}
                            title={actionable ? "Terminate process" : "Protected or unavailable on this platform"}
                            className="rounded-lg p-1.5 text-slate-400 transition-colors hover:bg-rose-500/10 hover:text-rose-400 disabled:opacity-30"
                          >
                            <Skull className="h-4 w-4" />
                          </button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            {filtered.length > MAX_VISIBLE_PROCESSES && (
              <p className="mt-3 text-xs text-slate-500">
                Showing the first {MAX_VISIBLE_PROCESSES} of {filtered.length}. Refine the filter to see more.
              </p>
            )}
          </>
        )}
      </Card>
    </div>
  );
}
