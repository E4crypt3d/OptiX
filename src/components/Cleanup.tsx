import { useCallback, useEffect, useState } from "react";
import { Cpu, RefreshCw, Trash2 } from "lucide-react";
import { dismComponentCleanup, runCleanup, scanCleanup } from "../lib/api";
import { formatBytes } from "../lib/format";
import type { CleanupCategory, CleanupResult } from "../lib/types";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";

export function Cleanup() {
  const [categories, setCategories] = useState<CleanupCategory[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<CleanupResult | null>(null);
  const [dismRunning, setDismRunning] = useState(false);
  const [dismOutput, setDismOutput] = useState<string | null>(null);

  const rescan = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const cats = await scanCleanup();
      setCategories(cats);
      setSelected(new Set(cats.filter((c) => c.safety === "safe").map((c) => c.id)));
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void rescan();
  }, [rescan]);

  function toggle(id: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  async function onClean() {
    const ids = categories.filter((c) => selected.has(c.id)).map((c) => c.id);
    if (ids.length === 0) return;
    const total = categories
      .filter((c) => selected.has(c.id))
      .reduce((a, c) => a + c.sizeBytes, 0);
    if (!window.confirm(`Delete ${formatBytes(total)} across ${ids.length} categor${ids.length === 1 ? "y" : "ies"}?`)) {
      return;
    }
    setRunning(true);
    setError(null);
    setResult(null);
    try {
      setResult(await runCleanup(ids));
      await rescan();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setRunning(false);
    }
  }

  const totalSelected = categories
    .filter((c) => selected.has(c.id))
    .reduce((a, c) => a + c.sizeBytes, 0);

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Cleanup</h1>
          <p className="text-sm text-slate-500">
            Free space from temporary files and caches. A snapshot is created first.
          </p>
        </div>
        <button
          onClick={rescan}
          disabled={loading}
          className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
        >
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          Rescan
        </button>
      </header>

      {error && (
        <div className="rounded-lg border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
          {error}
        </div>
      )}

      {result && (
        <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-300">
          Freed {formatBytes(result.freedBytes)} · {result.filesRemoved} files removed ·{" "}
          {result.filesSkipped} skipped. Snapshot {result.snapshotId.slice(0, 8)} created.
        </div>
      )}

      <Card title={`Categories (${formatBytes(totalSelected)} selected)`}>
        {categories.length === 0 && !loading ? (
          <p className="text-sm text-slate-500">No cleanup locations found.</p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {categories.map((c) => (
              <li key={c.id} className="flex items-center gap-3 py-3">
                <input
                  type="checkbox"
                  checked={selected.has(c.id)}
                  onChange={() => toggle(c.id)}
                  className="h-4 w-4 shrink-0 accent-cyan-500"
                />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-slate-200">{c.name}</span>
                    <Badge tone={c.safety === "safe" ? "emerald" : "amber"}>{c.safety}</Badge>
                    {c.expectedRebuild && <Badge tone="violet">rebuilds on launch</Badge>}
                  </div>
                  <div className="mt-0.5 truncate text-xs text-slate-500">{c.description}</div>
                </div>
                <div className="shrink-0 text-right">
                  <div className="tabular-nums text-slate-200">{formatBytes(c.sizeBytes)}</div>
                  <div className="text-xs tabular-nums text-slate-500">{c.fileCount} files</div>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <div className="flex items-center justify-end gap-3">
        <span className="text-sm text-slate-500">
          {selected.size} selected · {formatBytes(totalSelected)}
        </span>
        <button
          onClick={onClean}
          disabled={running || selected.size === 0}
          className="flex items-center gap-2 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
        >
          <Trash2 className="h-4 w-4" />
          {running ? "Cleaning…" : "Clean selected"}
        </button>
      </div>

      <Card title="WinSxS component store (DISM)">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="text-sm text-slate-400">
            <p>
              Run{" "}
              <span className="font-mono text-slate-200">
                dism /online /cleanup-image /startcomponentcleanup
              </span>{" "}
              to reclaim space from the Windows component store. Microsoft-sanctioned; requires
              administrator. A snapshot and a System Restore point are created first.{" "}
              <span className="text-slate-500">Never uses `resetbase`.</span>
            </p>
          </div>
          <button
            onClick={async () => {
              if (!window.confirm("Run DISM component cleanup? It can take several minutes.")) return;
              setDismRunning(true);
              setError(null);
              setDismOutput(null);
              try {
                setDismOutput(await dismComponentCleanup());
              } catch (e) {
                setError(errMsg(e));
              } finally {
                setDismRunning(false);
              }
            }}
            disabled={dismRunning}
            className="flex shrink-0 items-center gap-2 rounded-lg bg-slate-800 px-4 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
          >
            <Cpu className={`h-4 w-4 ${dismRunning ? "animate-pulse" : ""}`} />
            {dismRunning ? "Running DISM…" : "Run DISM cleanup"}
          </button>
        </div>
        {dismOutput && (
          <pre className="mt-3 max-h-48 overflow-auto rounded-lg bg-slate-950 p-3 text-xs text-slate-400">
            {dismOutput.trim() || "DISM completed without output."}
          </pre>
        )}
      </Card>
    </div>
  );
}
