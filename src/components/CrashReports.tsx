import { useCallback, useEffect, useMemo, useState } from "react";
import { AlertTriangle, Bug, FileArchive, RefreshCw } from "lucide-react";
import { generateCrashReport, scanCrashes } from "../lib/api";
import type { CrashReport } from "../lib/types";
import { Badge, Card } from "./ui";

function severityTone(severity: string): "rose" | "amber" | "slate" {
  if (severity === "high") return "rose";
  if (severity === "medium") return "amber";
  return "slate";
}

function sourceTone(source: string): "cyan" | "violet" | "slate" {
  if (source === "event_log") return "cyan";
  if (source === "wer") return "violet";
  return "slate";
}

function when(ts: number): string {
  if (!ts) return "unknown time";
  return new Date(ts).toLocaleString();
}

export function CrashReports() {
  const [crashes, setCrashes] = useState<CrashReport[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setCrashes(await scanCrashes());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const perApp = useMemo(() => {
    const map = new Map<string, number>();
    for (const c of crashes) {
      map.set(c.app, (map.get(c.app) ?? 0) + 1);
    }
    return [...map.entries()].sort((a, b) => b[1] - a[1]);
  }, [crashes]);

  async function onGenerate(c: CrashReport) {
    const key = `${c.app}:${c.detectedAt}`;
    setBusy(key);
    setError(null);
    setNotice(null);
    try {
      const path = await generateCrashReport(c);
      setNotice(`Crash report saved to ${path}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Crash Reports</h1>
          <p className="text-sm text-slate-500">
            Application and driver crashes from the event log, WER, and minidumps.
          </p>
        </div>
        <button
          onClick={refresh}
          disabled={loading}
          className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
        >
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          Scan now
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

      {perApp.length > 0 && (
        <Card title={`Crash Summary (${crashes.length} total)`}>
          <ul className="flex flex-wrap gap-2">
            {perApp.map(([app, count]) => (
              <li
                key={app}
                className="rounded-lg border border-slate-800 bg-slate-950/60 px-3 py-2 text-xs text-slate-300"
              >
                <span className="font-mono">{app}</span>
                <span className="ml-2 rounded-full bg-slate-800 px-2 py-0.5 text-slate-400">
                  {count}
                </span>
              </li>
            ))}
          </ul>
        </Card>
      )}

      <Card title={`Timeline (${crashes.length})`}>
        {crashes.length === 0 ? (
          <p className="text-sm text-slate-500">No crashes detected. Scan to check the event log.</p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {crashes.map((c) => (
              <li key={`${c.app}:${c.detectedAt}:${c.source}`} className="flex items-start gap-3 py-3">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-slate-800">
                  {c.severity === "high" ? (
                    <AlertTriangle className="h-4 w-4 text-rose-400" />
                  ) : (
                    <Bug className="h-4 w-4 text-cyan-400" />
                  )}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-mono text-slate-200">{c.app}</span>
                    <Badge tone={severityTone(c.severity)}>{c.severity}</Badge>
                    <Badge tone={sourceTone(c.source)}>{c.source}</Badge>
                    {c.eventId != null && <Badge tone="slate">event {c.eventId}</Badge>}
                    <span className="text-xs text-slate-600">{when(c.detectedAt)}</span>
                  </div>
                  <div className="mt-0.5 text-xs text-slate-400">
                    {c.exceptionName && <span>{c.exceptionName}</span>}
                    {c.exceptionCode && (
                      <span className="font-mono text-slate-500"> ({c.exceptionCode})</span>
                    )}
                    {c.module && (
                      <span className="font-mono text-slate-500"> · {c.module}</span>
                    )}
                  </div>
                  <div className="text-xs text-slate-500">{c.recommendation}</div>
                </div>
                <button
                  onClick={() => onGenerate(c)}
                  disabled={busy === `${c.app}:${c.detectedAt}`}
                  className="flex shrink-0 items-center gap-1.5 rounded-lg bg-slate-800 px-2.5 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
                >
                  <FileArchive className="h-3 w-3" />
                  {busy === `${c.app}:${c.detectedAt}` ? "Zipping…" : "Report"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
