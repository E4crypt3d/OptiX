import { useEffect, useState } from "react";
import { Database, FolderClock, ShieldCheck, Zap } from "lucide-react";
import { getAppInfo } from "../lib/api";
import { errMsg } from "../lib/errors";
import type { AppInfo } from "../lib/types";
import { Card } from "./ui";

export function Settings() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getAppInfo()
      .then(setInfo)
      .catch((e) => setError(errMsg(e)));
  }, []);

  return (
    <div className="space-y-4">
      <header>
        <h1 className="text-xl font-semibold text-slate-100">Settings</h1>
        <p className="text-sm text-slate-500">About Optix and how it keeps changes safe.</p>
      </header>

      {error && (
        <div className="rounded-lg border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
          {error}
        </div>
      )}

      <Card title="Application">
        <dl className="grid grid-cols-1 gap-x-8 gap-y-3 sm:grid-cols-2">
          <div className="flex items-center gap-3">
            <Zap className="h-4 w-4 shrink-0 text-cyan-400" />
            <div>
              <dt className="text-xs uppercase tracking-wider text-slate-500">Version</dt>
              <dd className="text-sm text-slate-200">{info?.version ?? "…"}</dd>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <FolderClock className="h-4 w-4 shrink-0 text-violet-400" />
            <div>
              <dt className="text-xs uppercase tracking-wider text-slate-500">Snapshot retention</dt>
              <dd className="text-sm text-slate-200">
                {info ? `${info.snapshotRetention} snapshots kept` : "…"}
              </dd>
            </div>
          </div>
          <div className="flex items-start gap-3 sm:col-span-2">
            <Database className="mt-0.5 h-4 w-4 shrink-0 text-slate-400" />
            <div className="min-w-0">
              <dt className="text-xs uppercase tracking-wider text-slate-500">Data directory</dt>
              <dd className="truncate font-mono text-xs text-slate-300">{info?.dataDir ?? "…"}</dd>
              <dt className="mt-2 text-xs uppercase tracking-wider text-slate-500">Snapshots</dt>
              <dd className="truncate font-mono text-xs text-slate-300">{info?.snapshotsDir ?? "…"}</dd>
            </div>
          </div>
        </dl>
      </Card>

      <Card title="Safety model">
        <div className="flex items-start gap-3">
          <ShieldCheck className="mt-0.5 h-5 w-5 shrink-0 text-emerald-400" />
          <ul className="space-y-1.5 text-sm text-slate-400">
            <li>Every change follows Detect → Snapshot → Apply → Verify → Record → Rollback.</li>
            <li>Mutations are snapshot-first and reversible from the Rollback Center.</li>
            <li>
              Optix never makes irreversible changes, never auto-deletes, and never applies REALTIME
              process priority.
            </li>
            <li>
              The current build runs as a single elevated process; a privileged-service split is the
              production plan.
            </li>
          </ul>
        </div>
      </Card>
    </div>
  );
}
