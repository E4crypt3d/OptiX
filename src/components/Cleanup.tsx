import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Cpu,
  HardDrive,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
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
  const selectionInitialized = useRef(false);
  const isWindows =
    typeof navigator !== "undefined" && /windows|win32/i.test(navigator.userAgent);

  const rescan = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const cats = await scanCleanup();
      setCategories(cats);
      setSelected((previous) => {
        const available = new Set(cats.map((category) => category.id));
        if (!selectionInitialized.current) {
          selectionInitialized.current = true;
          return new Set(cats.filter((category) => category.safety === "safe").map((category) => category.id));
        }
        return new Set([...previous].filter((id) => available.has(id)));
      });
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void rescan();
  }, [rescan]);

  const selectedCategories = useMemo(
    () => categories.filter((category) => selected.has(category.id)),
    [categories, selected],
  );
  const totalSelected = selectedCategories.reduce((total, category) => total + category.sizeBytes, 0);
  const cautionSelected = selectedCategories.filter((category) => category.safety === "caution");
  const rebuildSelected = selectedCategories.some((category) => category.expectedRebuild);
  const safeIds = categories
    .filter((category) => category.safety === "safe")
    .map((category) => category.id);

  function toggle(id: string) {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function selectSafe() {
    setSelected(new Set(safeIds));
  }

  function selectAll() {
    setSelected(new Set(categories.map((category) => category.id)));
  }

  function clearSelection() {
    setSelected(new Set());
  }

  async function onClean() {
    if (selectedCategories.length === 0) {
      setError("Select at least one cleanup category first.");
      return;
    }

    const cautionWarning = cautionSelected.length
      ? `\n\nCaution categories selected: ${cautionSelected.map((category) => category.name).join(", ")}.`
      : "";
    const rebuildWarning = rebuildSelected
      ? "\n\nGPU shader caches will be rebuilt the next time a game starts."
      : "";
    const confirmed = window.confirm(
      `Remove approximately ${formatBytes(totalSelected)} from ${selectedCategories.length} categor${selectedCategories.length === 1 ? "y" : "ies"}?${cautionWarning}${rebuildWarning}\n\nA snapshot will be created first. Locked files will be skipped.`,
    );
    if (!confirmed) return;

    setRunning(true);
    setError(null);
    setResult(null);
    try {
      const cleanupResult = await runCleanup(selectedCategories.map((category) => category.id));
      setResult(cleanupResult);
      await rescan();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setRunning(false);
    }
  }

  async function onDismCleanup() {
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
  }

  const busy = loading || running || dismRunning;

  return (
    <div className="space-y-4">
      <header className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Cleanup</h1>
          <p className="text-sm text-slate-500">
            Remove temporary files and caches without deleting directory roots. A snapshot is created first.
          </p>
        </div>
        <button
          onClick={rescan}
          disabled={busy}
          className="flex w-fit items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
        >
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          {loading ? "Scanning…" : "Rescan"}
        </button>
      </header>

      {error && (
        <div className="flex items-start gap-2 rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <span className="min-w-0">{error}</span>
        </div>
      )}

      {result && (
        <section className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 p-4 text-sm text-emerald-200">
          <div className="flex items-start gap-3">
            <CheckCircle2 className="mt-0.5 h-5 w-5 shrink-0 text-emerald-400" />
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2 font-medium">
                Cleanup complete
                <Badge tone="emerald">snapshot {result.snapshotId.slice(0, 8)}</Badge>
              </div>
              <div className="mt-2 grid grid-cols-1 gap-2 text-xs text-emerald-300 sm:grid-cols-3">
                <span>{formatBytes(result.freedBytes)} freed</span>
                <span>{result.filesRemoved} files removed</span>
                <span>{result.filesSkipped} skipped or locked</span>
              </div>
              {result.categories.length > 0 && (
                <div className="mt-3 space-y-1 border-t border-emerald-500/20 pt-2 text-xs text-emerald-300/80">
                  {result.categories.map((category) => (
                    <div key={category.id} className="flex flex-wrap justify-between gap-x-3 gap-y-1">
                      <span>{category.id}</span>
                      <span>
                        {formatBytes(category.freedBytes)} freed · {category.filesRemoved} removed
                        {category.filesSkipped > 0 ? ` · ${category.filesSkipped} skipped` : ""}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </section>
      )}

      <Card
        title={`Cleanup candidates${categories.length > 0 ? ` · ${formatBytes(totalSelected)} selected` : ""}`}
        action={
          <span className="text-xs text-slate-500">
            {loading ? "Scanning…" : `${categories.length} available`}
          </span>
        }
      >
        {loading && categories.length === 0 ? (
          <div className="space-y-3 py-2">
            {[0, 1, 2].map((item) => (
              <div key={item} className="h-14 animate-pulse rounded-lg bg-slate-800/50" />
            ))}
          </div>
        ) : categories.length === 0 ? (
          <div className="flex flex-col items-center gap-2 py-10 text-center text-slate-500">
            <HardDrive className="h-8 w-8" />
            <p className="text-sm font-medium text-slate-400">No cleanup candidates found</p>
            <p className="max-w-md text-xs">
              Temporary and cache locations are either empty, unavailable, or currently protected by the operating system.
            </p>
          </div>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {categories.map((category) => (
              <li key={category.id} className="flex items-start gap-3 py-3">
                <input
                  type="checkbox"
                  checked={selected.has(category.id)}
                  onChange={() => toggle(category.id)}
                  disabled={busy}
                  aria-label={`Select ${category.name}`}
                  className="mt-1 h-4 w-4 shrink-0 accent-cyan-500"
                />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium text-slate-200">{category.name}</span>
                    <Badge tone={category.safety === "safe" ? "emerald" : "amber"}>
                      {category.safety}
                    </Badge>
                    {category.expectedRebuild && <Badge tone="violet">rebuilds later</Badge>}
                  </div>
                  <div className="mt-1 text-xs leading-5 text-slate-500">{category.description}</div>
                </div>
                <div className="shrink-0 text-right">
                  <div className="tabular-nums text-slate-200">{formatBytes(category.sizeBytes)}</div>
                  <div className="text-xs tabular-nums text-slate-500">
                    {category.fileCount} {category.fileCount === 1 ? "file" : "files"}
                  </div>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <div className="flex flex-col gap-3 rounded-xl border border-slate-800 bg-slate-900/40 p-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-wrap items-center gap-2 text-xs text-slate-500">
          <button onClick={selectSafe} disabled={busy || safeIds.length === 0} className="hover:text-slate-300 disabled:opacity-50">
            Select safe
          </button>
          <span>·</span>
          <button onClick={selectAll} disabled={busy || categories.length === 0} className="hover:text-slate-300 disabled:opacity-50">
            Select all
          </button>
          <span>·</span>
          <button onClick={clearSelection} disabled={busy || selected.size === 0} className="hover:text-slate-300 disabled:opacity-50">
            Clear
          </button>
          <span className="text-slate-600">{selectedCategories.length} selected</span>
        </div>
        <button
          onClick={onClean}
          disabled={busy || selectedCategories.length === 0}
          className="flex items-center justify-center gap-2 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
        >
          <Trash2 className="h-4 w-4" />
          {running ? "Cleaning…" : `Clean ${formatBytes(totalSelected)}`}
        </button>
      </div>

      {isWindows ? (
        <Card title="WinSxS component store (DISM)">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0 text-sm text-slate-400">
              <p>
                Run <span className="font-mono text-slate-200">dism /online /cleanup-image /startcomponentcleanup</span> to reclaim space from the Windows component store. It requires administrator privileges and never uses <span className="font-mono text-slate-300">resetbase</span>.
              </p>
            </div>
            <button
              onClick={onDismCleanup}
              disabled={busy}
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
      ) : (
        <div className="flex items-start gap-2 rounded-xl border border-slate-800 bg-slate-900/30 px-4 py-3 text-xs text-slate-500">
          <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-slate-400" />
          WinSxS/DISM cleanup is only available on Windows; it is hidden on this platform.
        </div>
      )}
    </div>
  );
}
