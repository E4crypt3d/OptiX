import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Gauge, Play, RefreshCw, Trash2 } from "lucide-react";
import {
  benchmarkFrameTimes,
  deleteBenchmark,
  listBenchmarks,
  listGames,
  runFpsBenchmark,
  runStressBenchmark,
} from "../lib/api";
import type { BenchmarkResult, Game } from "../lib/types";
import { decimateFrameTimes } from "../lib/decimate";
import { formatBytes } from "../lib/format";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

const fps = (v: number | null) => (v == null ? "—" : v.toFixed(1));
const ms = (v: number | null) => (v == null ? "—" : v.toFixed(2));

function when(ts: number): string {
  return new Date(ts).toLocaleString();
}

export function Benchmark() {
  const [runs, setRuns] = useState<BenchmarkResult[]>([]);
  const [games, setGames] = useState<Game[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [gameId, setGameId] = useState<number | "">("");
  const [exeName, setExeName] = useState("");
  const [duration, setDuration] = useState(30);
  const [chartId, setChartId] = useState<number | null>(null);
  const [chartData, setChartData] = useState<{ frame: number; ms: number }[]>([]);
  const chartToken = useRef(0);
  const [compare, setCompare] = useState<Set<number>>(new Set());

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [b, g] = await Promise.all([listBenchmarks(), listGames()]);
      setRuns(b);
      setGames(g);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const selectedGame = useMemo(
    () => games.find((g) => g.id === gameId),
    [games, gameId],
  );

  async function onRunFps() {
    const exe = selectedGame ? selectedGame.exeName : exeName.trim();
    if (!exe) {
      setError("Pick a game or enter an executable name.");
      return;
    }
    setBusy("fps");
    setError(null);
    setNotice(null);
    try {
      const r = await runFpsBenchmark(
        selectedGame ? selectedGame.id : null,
        selectedGame ? selectedGame.name : exe,
        exe,
        duration,
      );
      setNotice(
        `Captured ${r.frameCount} frames · avg ${fps(r.avgFps)} FPS · 1% low ${fps(r.p1Fps)} FPS.`,
      );
      await refresh();
      if (r.id != null) await onChart(r.id);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  async function onRunStress() {
    setBusy("stress");
    setError(null);
    setNotice(null);
    try {
      const r = await runStressBenchmark(duration);
      setNotice(
        `Stress run done · CPU ${r.cpuAvg?.toFixed(1) ?? "—"}% · RAM ${Math.round(r.ramAvgMb ?? 0)} MB.`,
      );
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  async function onChart(id: number) {
    const token = ++chartToken.current;
    setChartId(id);
    try {
      const times = await benchmarkFrameTimes(id);
      // A full capture can hold tens of thousands of frames; decimate before
      // rendering so the chart stays responsive.
      const points = times.map((ms, i) => ({ frame: i + 1, ms: Number(ms.toFixed(2)) }));
      if (chartToken.current !== token) return; // a newer chart was requested
      setChartData(decimateFrameTimes(points, 1500));
    } catch {
      if (chartToken.current === token) setChartData([]);
    }
  }

  async function onDelete(r: BenchmarkResult) {
    if (r.id == null) return;
    if (!window.confirm("Delete this benchmark run?")) return;
    setBusy(`delete:${r.id}`);
    setError(null);
    try {
      await deleteBenchmark(r.id);
      if (chartId === r.id) {
        setChartId(null);
        setChartData([]);
      }
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  function toggleCompare(id: number) {
    setCompare((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else if (next.size < 2) next.add(id);
      else next.clear(), next.add(id);
      return next;
    });
  }

  const compared = runs.filter((r) => r.id != null && compare.has(r.id));

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Benchmarks</h1>
          <p className="text-sm text-slate-500">
            Measure FPS and frame pacing (PresentMon) or system stress. Compare before/after runs.
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

      <Card title="Run a Benchmark">
        <div className="flex flex-wrap items-end gap-3">
          <label className="text-xs text-slate-400">
            Game
            <select
              value={gameId}
              onChange={(e) => setGameId(e.currentTarget.value === "" ? "" : Number(e.currentTarget.value))}
              className="mt-1 block w-56 rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 text-xs text-slate-200 focus:border-cyan-500 focus:outline-none"
            >
              <option value="">Manual executable…</option>
              {games.map((g) => (
                <option key={g.id} value={g.id}>
                  {g.name}
                </option>
              ))}
            </select>
          </label>

          {!selectedGame && (
            <label className="text-xs text-slate-400">
              Process name
              <input
                value={exeName}
                onChange={(e) => setExeName(e.currentTarget.value)}
                placeholder="cs2.exe"
                className="mt-1 block w-52 rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 font-mono text-xs text-slate-200 placeholder:text-slate-600 focus:border-cyan-500 focus:outline-none"
              />
            </label>
          )}

          <label className="text-xs text-slate-400">
            Duration (s)
            <input
              type="number"
              min={5}
              max={300}
              value={duration}
              onChange={(e) => setDuration(Number(e.currentTarget.value) || 30)}
              className="mt-1 block w-24 rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 text-xs text-slate-200 focus:border-cyan-500 focus:outline-none"
            />
          </label>

          <button
            onClick={onRunFps}
            disabled={busy === "fps" || busy === "stress"}
            className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-2 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
          >
            <Play className="h-3.5 w-3.5" />
            {busy === "fps" ? "Capturing…" : "Start FPS capture"}
          </button>
          <button
            onClick={onRunStress}
            disabled={busy === "fps" || busy === "stress"}
            className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-3 py-2 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
          >
            <Gauge className="h-3.5 w-3.5" />
            {busy === "stress" ? "Sampling…" : "Stress test"}
          </button>
        </div>
        <p className="mt-3 text-xs text-slate-600">
          FPS capture requires PresentMon64.exe (place it next to the Optix executable or on PATH).
          Stress test measures CPU/RAM without PresentMon.
        </p>
      </Card>

      {compared.length === 2 && (
        <Card title="Comparison">
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-5">
            {[
              ["Avg FPS", compared[0].avgFps, compared[1].avgFps],
              ["1% low", compared[0].p1Fps, compared[1].p1Fps],
              ["0.1% low", compared[0].p01Fps, compared[1].p01Fps],
              ["Frame time", compared[0].avgFrameTimeMs, compared[1].avgFrameTimeMs],
              ["CPU %", compared[0].cpuAvg, compared[1].cpuAvg],
            ].map(([label, a, b]) => {
              const av = a as number | null;
              const bv = b as number | null;
              const unit = label === "Frame time" ? "ms" : label === "CPU %" ? "%" : "fps";
              return (
                <div key={label as string} className="rounded-lg border border-slate-800 bg-slate-950/60 p-3">
                  <div className="text-xs text-slate-500">{label as string}</div>
                  <div className="mt-1 flex items-baseline gap-2">
                    <span className="tabular-nums text-slate-200">{av == null ? "—" : av.toFixed(1)}</span>
                    <span className="text-xs text-slate-600">{unit}</span>
                  </div>
                  <div className="tabular-nums text-slate-400">{bv == null ? "—" : bv.toFixed(1)}</div>
                </div>
              );
            })}
          </div>
        </Card>
      )}

      {chartData.length > 0 && (
        <Card title={`Frame Times (run #${chartId})`}>
          <div className="h-64">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={chartData} margin={{ top: 4, right: 8, left: -16, bottom: 0 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
                <XAxis dataKey="frame" stroke="#475569" fontSize={11} tickLine={false} minTickGap={60} />
                <YAxis stroke="#475569" fontSize={11} tickLine={false} domain={[0, "auto"]} />
                <Tooltip
                  contentStyle={{ background: "#0f172a", border: "1px solid #1e293b", borderRadius: 8, fontSize: 12 }}
                />
                <Line
                  type="monotone"
                  dataKey="ms"
                  name="Frame time (ms)"
                  stroke="#22d3ee"
                  strokeWidth={1.5}
                  dot={false}
                  isAnimationActive={false}
                />
              </LineChart>
            </ResponsiveContainer>
          </div>
        </Card>
      )}

      <Card title={`History (${runs.length})`}>
        {runs.length === 0 ? (
          <p className="text-sm text-slate-500">No benchmark runs yet.</p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {runs.map((r) => (
              <li key={r.id} className="flex items-center gap-3 py-2.5">
                <input
                  type="checkbox"
                  checked={r.id != null && compare.has(r.id)}
                  onChange={() => r.id != null && toggleCompare(r.id)}
                  className="h-4 w-4 shrink-0 accent-cyan-500"
                />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium text-slate-200">{r.gameName ?? "System stress"}</span>
                    {r.avgFps != null ? (
                      <Badge tone="cyan">FPS</Badge>
                    ) : (
                      <Badge tone="slate">stress</Badge>
                    )}
                    <span className="text-xs text-slate-600">{when(r.startedAt)}</span>
                  </div>
                  <div className="mt-0.5 flex flex-wrap gap-x-4 gap-y-0.5 text-xs tabular-nums text-slate-500">
                    {r.avgFps != null && (
                      <>
                        <span>avg {fps(r.avgFps)}</span>
                        <span>1% low {fps(r.p1Fps)}</span>
                        <span>0.1% low {fps(r.p01Fps)}</span>
                        <span>p95 {ms(r.p95FrameTimeMs)} ms</span>
                        <span>{r.frameCount} frames</span>
                      </>
                    )}
                    <span>CPU {r.cpuAvg?.toFixed(1) ?? "—"}%</span>
                    <span>RAM {formatBytes((r.ramAvgMb ?? 0) * 1024 * 1024)}</span>
                  </div>
                </div>

                {r.avgFps != null && (
                  <button
                    onClick={() => r.id != null && onChart(r.id)}
                    className={`rounded-lg px-2.5 py-1.5 text-xs font-medium transition-colors ${
                      chartId === r.id
                        ? "bg-cyan-600 text-white"
                        : "bg-slate-800 text-slate-200 hover:bg-slate-700"
                    }`}
                  >
                    Chart
                  </button>
                )}
                <button
                  onClick={() => onDelete(r)}
                  disabled={busy === `delete:${r.id}`}
                  className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-2.5 py-1.5 text-xs font-medium text-rose-300 transition-colors hover:bg-slate-700 disabled:opacity-50"
                >
                  <Trash2 className="h-3 w-3" />
                </button>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
