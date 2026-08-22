import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AppWindow,
  Bug,
  CircuitBoard,
  Cpu,
  DownloadCloud,
  Gauge,
  HardDrive,
  MemoryStick,
  Monitor,
  Power,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  Thermometer,
} from "lucide-react";
import { runDiagnostics } from "../lib/api";
import type { Diagnostic, DiagnosticsReport } from "../lib/types";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";

type Severity = "critical" | "warning" | "info";

function asSeverity(value: string): Severity {
  if (value === "critical" || value === "warning") return value;
  return "info";
}

const SEVERITY_TONE: Record<Severity, "rose" | "amber" | "slate"> = {
  critical: "rose",
  warning: "amber",
  info: "slate",
};

const CATEGORY_META: Record<string, { label: string; icon: typeof Cpu }> = {
  cpu: { label: "CPU", icon: Cpu },
  gpu: { label: "GPU", icon: Monitor },
  memory: { label: "Memory", icon: MemoryStick },
  storage: { label: "Storage", icon: HardDrive },
  background: { label: "Background", icon: AppWindow },
  update: { label: "Updates", icon: DownloadCloud },
  driver: { label: "Driver", icon: CircuitBoard },
  thermal: { label: "Thermal", icon: Thermometer },
  stability: { label: "Stability", icon: Bug },
  frametime: { label: "Frametime", icon: Gauge },
  system: { label: "System", icon: Power },
};

const RING_RADIUS = 34;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

function scoreTone(score: number): { stroke: string; text: string } {
  if (score >= 90) return { stroke: "stroke-emerald-400", text: "text-emerald-400" };
  if (score >= 45) return { stroke: "stroke-amber-400", text: "text-amber-400" };
  return { stroke: "stroke-rose-500", text: "text-rose-400" };
}

export function Diagnostics() {
  const [report, setReport] = useState<DiagnosticsReport | null>(null);
  const [filter, setFilter] = useState<Severity | "all">("all");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setReport(await runDiagnostics());
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const findings = report?.findings ?? [];
  const counts = useMemo(
    () => ({
      critical: findings.filter((f) => f.severity === "critical").length,
      warning: findings.filter((f) => f.severity === "warning").length,
      info: findings.filter((f) => asSeverity(f.severity) === "info").length,
    }),
    [findings],
  );
  const visible =
    filter === "all"
      ? findings
      : findings.filter((f) => asSeverity(f.severity) === filter);

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

      {report && (
        <Card>
          <div className="flex flex-wrap items-center gap-6">
            <div className="relative h-24 w-24 shrink-0">
              <svg viewBox="0 0 80 80" className="h-full w-full -rotate-90">
                <circle
                  cx="40"
                  cy="40"
                  r={RING_RADIUS}
                  fill="none"
                  strokeWidth="7"
                  className="stroke-slate-800"
                />
                <circle
                  cx="40"
                  cy="40"
                  r={RING_RADIUS}
                  fill="none"
                  strokeWidth="7"
                  strokeLinecap="round"
                  className={scoreTone(report.score).stroke}
                  strokeDasharray={RING_CIRCUMFERENCE}
                  strokeDashoffset={RING_CIRCUMFERENCE * (1 - report.score / 100)}
                />
              </svg>
              <div className="absolute inset-0 flex flex-col items-center justify-center">
                <span className={`text-2xl font-bold ${scoreTone(report.score).text}`}>
                  {report.score}
                </span>
                <span className="text-[10px] uppercase tracking-wide text-slate-500">health</span>
              </div>
            </div>
            <div className="min-w-0 flex-1">
              <div className="text-lg font-semibold text-slate-100">{report.verdict}</div>
              <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-slate-500">
                <span>{report.checksRun} checks run</span>
                {counts.critical > 0 && <Badge tone="rose">{counts.critical} critical</Badge>}
                {counts.warning > 0 && <Badge tone="amber">{counts.warning} warnings</Badge>}
                {counts.info > 0 && <Badge tone="slate">{counts.info} notes</Badge>}
              </div>
            </div>
          </div>
        </Card>
      )}

      {findings.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {(["all", "critical", "warning", "info"] as const).map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`rounded-lg px-3 py-1.5 text-xs font-medium capitalize transition-colors ${
                filter === f
                  ? "bg-slate-700 text-slate-100"
                  : "bg-slate-900 text-slate-400 hover:text-slate-200"
              }`}
            >
              {f === "all" ? `All (${findings.length})` : `${f} (${counts[f]})`}
            </button>
          ))}
        </div>
      )}

      <Card title={`Findings (${visible.length})`}>
        {visible.length === 0 ? (
          <div className="flex flex-col items-center gap-2 py-8 text-center">
            <ShieldCheck className="h-10 w-10 text-emerald-400" />
            <p className="text-sm text-slate-400">
              {findings.length === 0
                ? `No issues found — all ${report?.checksRun ?? ""} checks passed. Your system looks healthy.`
                : `No ${filter} findings.`}
            </p>
          </div>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {visible.map((d) => (
              <FindingRow key={d.id} finding={d} />
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}

function FindingRow({ finding }: { finding: Diagnostic }) {
  const meta = CATEGORY_META[finding.category];
  const Icon = meta?.icon ?? Sparkles;
  return (
    <li className="flex items-start gap-3 py-3">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-slate-800">
        <Icon className="h-4 w-4 text-cyan-400" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-medium text-slate-200">{finding.title}</span>
          <Badge tone={SEVERITY_TONE[asSeverity(finding.severity)]}>{finding.severity}</Badge>
          <Badge tone="slate">{meta?.label ?? finding.category}</Badge>
          <Badge tone="cyan">{finding.confidence}% confidence</Badge>
        </div>
        <div className="mt-0.5 text-xs text-slate-500">{finding.detail}</div>
        <div className="text-xs text-slate-400">
          <span className="font-medium text-slate-300">Recommendation:</span>{" "}
          {finding.recommendation}
        </div>
      </div>
      <div className="w-24 shrink-0">
        <div className="h-1.5 w-full overflow-hidden rounded-full bg-slate-800">
          <div
            className="h-full rounded-full bg-gradient-to-r from-cyan-400 to-violet-500"
            style={{ width: `${Math.max(0, Math.min(100, finding.confidence))}%` }}
          />
        </div>
      </div>
    </li>
  );
}
