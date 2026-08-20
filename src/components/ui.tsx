import type { ReactNode } from "react";

export function Card({
  title,
  children,
  className = "",
  action,
}: {
  title?: string;
  children: ReactNode;
  className?: string;
  action?: ReactNode;
}) {
  return (
    <section
      className={`cv rounded-xl border border-slate-800 bg-slate-900/60 p-4 shadow-sm ${className}`}
    >
      {title && (
        <header className="mb-3 flex items-center justify-between">
          <h2 className="text-sm font-semibold tracking-wide text-slate-300">
            {title}
          </h2>
          {action}
        </header>
      )}
      {children}
    </section>
  );
}

export function Stat({
  label,
  value,
  sub,
  icon,
}: {
  label: string;
  value: string;
  sub?: string;
  icon?: ReactNode;
}) {
  return (
    <div className="rounded-xl border border-slate-800 bg-slate-900/60 p-4">
      <div className="flex items-center gap-2 text-slate-400">
        {icon}
        <span className="text-xs font-medium uppercase tracking-wider">
          {label}
        </span>
      </div>
      <div className="mt-2 text-2xl font-semibold text-slate-100">{value}</div>
      {sub && <div className="mt-1 text-xs text-slate-500">{sub}</div>}
    </div>
  );
}

export function ProgressBar({
  value,
  className = "",
  tone,
}: {
  value: number;
  className?: string;
  tone?: "cyan" | "violet" | "amber" | "emerald";
}) {
  const clamped = Math.max(0, Math.min(100, value));
  const tones: Record<string, string> = {
    cyan: "from-cyan-400 to-blue-500",
    violet: "from-violet-400 to-fuchsia-500",
    amber: "from-amber-400 to-orange-500",
    emerald: "from-emerald-400 to-teal-500",
  };
  const gradient = tones[tone ?? "cyan"] ?? tones.cyan;
  return (
    <div
      className={`h-2 w-full overflow-hidden rounded-full bg-slate-800 ${className}`}
    >
      <div
        className={`h-full rounded-full bg-gradient-to-r ${gradient} transition-all duration-500`}
        style={{ width: `${clamped}%` }}
      />
    </div>
  );
}

export function Badge({
  children,
  tone = "slate",
}: {
  children: ReactNode;
  tone?: "slate" | "emerald" | "amber" | "violet" | "cyan" | "rose";
}) {
  const tones: Record<string, string> = {
    slate: "bg-slate-800 text-slate-300",
    emerald: "bg-emerald-500/15 text-emerald-400",
    amber: "bg-amber-500/15 text-amber-400",
    violet: "bg-violet-500/15 text-violet-400",
    cyan: "bg-cyan-500/15 text-cyan-400",
    rose: "bg-rose-500/15 text-rose-400",
  };
  return (
    <span
      className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${tones[tone]}`}
    >
      {children}
    </span>
  );
}
