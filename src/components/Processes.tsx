import { useCallback, useEffect, useMemo, useState } from "react";
import { Gamepad2, RefreshCw, Skull, Undo2 } from "lucide-react";
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

export function Processes() {
  const [processes, setProcesses] = useState<ProcessDetail[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [filter, setFilter] = useState<ProcessClass | "all">("all");
  const [search, setSearch] = useState("");
  const [gamePids, setGamePids] = useState<Set<number>>(new Set());
  const [bgPids, setBgPids] = useState<Set<number>>(new Set());

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setProcesses(await listProcesses());
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

  async function onKill(p: ProcessDetail) {
    if (
      !window.confirm(
        `Terminate ${p.name} (PID ${p.pid})? Unsaved work may be lost.`,
      )
    ) {
      return;
    }
    setError(null);
    try {
      await killProcess(p.pid);
      setNotice(`Terminated ${p.name}.`);
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    }
  }

  async function onSetPriority(p: ProcessDetail, priority: PriorityClass) {
    setError(null);
    try {
      await setProcessPriority(p.pid, priority);
      setNotice(`${p.name} → ${priority}.`);
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    }
  }

  async function onApplyGamingMode() {
    if (gamePids.size === 0) {
      setError("Select at least one game process to boost.");
      return;
    }
    setError(null);
    setNotice(null);
    try {
      const result = await applyGamingMode([...gamePids], [...bgPids]);
      const total = result.boosted.length + result.lowered.length;
      if (total === 0) {
        setNotice(
          "Gaming mode requires Windows. No priority changes were applied.",
        );
      } else {
        setNotice(
          `Gaming mode applied: ${result.boosted.length} boosted, ${result.lowered.length} lowered.`,
        );
      }
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    }
  }

  async function onRestore() {
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
    }
  }

  function togglePid(setter: React.Dispatch<React.SetStateAction<Set<number>>>, pid: number) {
    setter((prev) => {
      const next = new Set(prev);
      if (next.has(pid)) next.delete(pid);
      else next.add(pid);
      return next;
    });
  }

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">
            Processes &amp; RAM
          </h1>
          <p className="text-sm text-slate-500">
            Classify, prioritize, and clean up running processes. Never uses
            REALTIME priority.
          </p>
        </div>
        <button
          onClick={refresh}
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

      <Card title="Gaming mode">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="text-sm text-slate-400">
            <p>
              Boost the selected game to{" "}
              <span className="text-slate-200">above-normal</span> priority and
              lower background processes to{" "}
              <span className="text-slate-200">below-normal</span>. Changes are
              restored when you leave gaming mode.
            </p>
            <p className="mt-1 text-xs text-slate-500">
              Select a process with the <Gamepad2 className="inline h-3 w-3" />{" "}
              toggle as the game; others can be marked background.
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <button
              onClick={onApplyGamingMode}
              disabled={gamePids.size === 0}
              className="flex items-center gap-2 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
            >
              <Gamepad2 className="h-4 w-4" />
              Apply gaming mode
            </button>
            <button
              onClick={onRestore}
              className="flex items-center gap-2 rounded-lg bg-slate-800 px-4 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700"
            >
              <Undo2 className="h-4 w-4" />
              Restore
            </button>
          </div>
        </div>
      </Card>

      <Card
        title={`Processes (${filtered.length})`}
        action={
          <div className="flex items-center gap-2">
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Filter…"
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-1.5 text-sm text-slate-200 placeholder-slate-500 focus:border-cyan-500 focus:outline-none"
            />
            <select
              value={filter}
              onChange={(e) => setFilter(e.target.value as ProcessClass | "all")}
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
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-800 text-left text-xs uppercase tracking-wider text-slate-500">
                <th className="py-2 pr-2 font-medium">Game</th>
                <th className="py-2 pr-2 font-medium">Background</th>
                <th className="py-2 pr-2 font-medium">Process</th>
                <th className="py-2 pr-2 text-right font-medium">CPU</th>
                <th className="py-2 pr-2 text-right font-medium">RAM</th>
                <th className="py-2 pr-2 text-right font-medium">Disk read</th>
                <th className="py-2 pr-2 font-medium">Priority</th>
                <th className="py-2 pr-2 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {filtered.slice(0, 200).map((p) => (
                <tr
                  key={p.pid}
                  className="border-b border-slate-800/50 hover:bg-slate-900/40"
                >
                  <td className="py-2 pr-2">
                    <input
                      type="checkbox"
                      checked={gamePids.has(p.pid)}
                      disabled={p.isSystem}
                      onChange={() => togglePid(setGamePids, p.pid)}
                      className="h-4 w-4 accent-cyan-500"
                    />
                  </td>
                  <td className="py-2 pr-2">
                    <input
                      type="checkbox"
                      checked={bgPids.has(p.pid)}
                      disabled={p.isSystem || p.classification === "required"}
                      onChange={() => togglePid(setBgPids, p.pid)}
                      className="h-4 w-4 accent-amber-500"
                    />
                  </td>
                  <td className="max-w-[260px] py-2 pr-2">
                    <div className="flex items-center gap-2">
                      <span className="truncate font-medium text-slate-200">
                        {p.name}
                      </span>
                      <Badge tone={CLASS_TONE[p.classification]}>
                        {p.classification}
                      </Badge>
                      {p.isSystem && <Badge tone="rose">system</Badge>}
                    </div>
                    <div className="truncate text-xs text-slate-500">
                      PID {p.pid} · {p.exe || p.status}
                    </div>
                  </td>
                  <td className="py-2 pr-2 text-right tabular-nums text-slate-300">
                    {p.cpuUsagePercent.toFixed(1)}%
                  </td>
                  <td className="py-2 pr-2 text-right tabular-nums text-slate-300">
                    {formatBytes(p.memoryBytes)}
                  </td>
                  <td className="py-2 pr-2 text-right tabular-nums text-slate-500">
                    {formatBytes(p.diskReadBytes)}
                  </td>
                  <td className="py-2 pr-2">
                    <select
                      value={p.priority ?? "unknown"}
                      disabled={p.isSystem || p.classification === "required"}
                      onChange={(e) => {
                        const v = e.target.value as PriorityClass | "unknown";
                        if (v !== "unknown") onSetPriority(p, v);
                      }}
                      className="rounded-lg border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-200 focus:border-cyan-500 focus:outline-none disabled:opacity-50"
                    >
                      {p.priority === null && (
                        <option value="unknown">n/a</option>
                      )}
                      {PRIORITIES.map((pr) => (
                        <option key={pr} value={pr}>
                          {pr.replace("_", " ")}
                        </option>
                      ))}
                      {p.priority === "realtime" && (
                        <option value="realtime">realtime</option>
                      )}
                    </select>
                  </td>
                  <td className="py-2 pr-2">
                    <button
                      onClick={() => onKill(p)}
                      disabled={p.isSystem || p.classification === "required"}
                      title={
                        p.isSystem || p.classification === "required"
                          ? "Protected process"
                          : "Terminate process"
                      }
                      className="rounded-lg p-1.5 text-slate-400 transition-colors hover:bg-rose-500/10 hover:text-rose-400 disabled:opacity-30"
                    >
                      <Skull className="h-4 w-4" />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {filtered.length === 0 && !loading && (
          <p className="py-4 text-sm text-slate-500">No processes match.</p>
        )}
      </Card>
    </div>
  );
}
