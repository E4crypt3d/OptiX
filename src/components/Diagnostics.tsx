import { useCallback, useEffect, useState } from "react";
import { RefreshCw, ShieldCheck, Sparkles } from "lucide-react";
import { runDiagnostics } from "../lib/api";
import type { Diagnostic } from "../lib/types";
import { Badge, Card } from "./ui";

function severityTone(severity: string): "rose" | "amber" | "slate" {
  if (severity === "critical") return "rose";
  if (severity === "warning") return "amber";
  return "slate";
}

const CATEGORY_LABEL: Record<string, string> = {
  cpu: "CPU",
  gpu: "GPU",
  memory: "Memory",
  storage: "Storage",
  background: "Background",
  update: "Updates",
  driver: "Driver",
  thermal: "Thermal",
};

export function Diagnostics() {
  const [findings, setFindings] = useState<Diagnostic[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setFindings(await runDiagnostics());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Diagnostics</h1>
          <p className="text-sm text-slate-500">
            Rule-based analysis of telemetry, benchmarks, and crashes. Nothing is changed
            automatically.
          </p>
        </div>
        <button
          onClick={refresh}
          disabled={loading}
          className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
        >
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          {loading ? "Analyzing…" : "Run analysis"}
        </button>
      </header>

      {error && (
        <div className="rounded-lg border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
          {error}
        </div>
      )}

      <Card title={`Findings (${findings.length})`}>
        {findings.length === 0 ? (
          <div className="flex flex-col items-center gap-2 py-8 text-center">
            <ShieldCheck className="h-10 w-10 text-emerald-400" />
            <p className="text-sm text-slate-400">No issues found. Your system looks healthy.</p>
          </div>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {findings.map((d) => (
              <li key={d.id} className="flex items-start gap-3 py-3">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-slate-800">
                  <Sparkles className="h-4 w-4 text-cyan-400" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium text-slate-200">{d.title}</span>
                    <Badge tone={severityTone(d.severity)}>{d.severity}</Badge>
                    <Badge tone="slate">{CATEGORY_LABEL[d.category] ?? d.category}</Badge>
                    <Badge tone="cyan">{d.confidence}% confidence</Badge>
                  </div>
                  <div className="mt-0.5 text-xs text-slate-500">{d.detail}</div>
                  <div className="text-xs text-slate-400">
                    <span className="font-medium text-slate-300">Recommendation:</span>{" "}
                    {d.recommendation}
                  </div>
                </div>
                <div className="w-24 shrink-0">
                  <div className="h-1.5 w-full overflow-hidden rounded-full bg-slate-800">
                    <div
                      className="h-full rounded-full bg-gradient-to-r from-cyan-400 to-violet-500"
                      style={{ width: `${Math.max(0, Math.min(100, d.confidence))}%` }}
                    />
                  </div>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
