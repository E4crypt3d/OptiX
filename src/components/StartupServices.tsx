import { useCallback, useEffect, useMemo, useState } from "react";
import { Play, RefreshCw, Search, Square } from "lucide-react";
import {
  getWsearch,
  listServices,
  listStartup,
  setServiceStartType,
  setStartupEnabled,
  setWsearch,
  startService,
  stopService,
} from "../lib/api";
import type {
  ServiceClass,
  ServiceInfo,
  StartupEntry,
  WSearchStatus,
} from "../lib/types";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";

type ClassFilter = "all" | ServiceClass;

function classTone(c: ServiceClass): "rose" | "emerald" | "slate" {
  switch (c) {
    case "required":
      return "rose";
    case "safe":
      return "emerald";
    default:
      return "slate";
  }
}

function stateTone(state: string): "emerald" | "slate" | "amber" {
  if (state === "running") return "emerald";
  if (state === "stopped") return "slate";
  return "amber";
}

export function StartupServices() {
  const [services, setServices] = useState<ServiceInfo[]>([]);
  const [startup, setStartup] = useState<StartupEntry[]>([]);
  const [wsearch, setWsearchStatus] = useState<WSearchStatus | null>(null);
  const [query, setQuery] = useState("");
  const [classFilter, setClassFilter] = useState<ClassFilter>("all");
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [s, st, w] = await Promise.all([
        listServices(),
        listStartup(),
        getWsearch(),
      ]);
      setServices(s);
      setStartup(st);
      setWsearchStatus(w);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return services.filter((s) => {
      if (classFilter !== "all" && s.classification !== classFilter) return false;
      if (!q) return true;
      return (
        s.name.toLowerCase().includes(q) ||
        s.displayName.toLowerCase().includes(q) ||
        s.binaryPath.toLowerCase().includes(q)
      );
    });
  }, [services, query, classFilter]);

  async function run(
    key: string,
    action: () => Promise<{ snapshotId: string; changes: number }>,
    msg: string,
  ) {
    setBusy(key);
    setError(null);
    setNotice(null);
    try {
      const r = await action();
      setNotice(r.changes > 0 ? msg : "No change was needed.");
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  async function onToggleWsearch() {
    if (!wsearch) return;
    const next = !wsearch.enabled;
    if (
      !window.confirm(
        next
          ? "Enable Windows Search? Search indexing will restart."
          : "Disable Windows Search? Start-menu and Explorer search will become slower. A snapshot is created first.",
      )
    ) {
      return;
    }
    await run("wsearch", () => setWsearch(next), next ? "Windows Search enabled." : "Windows Search disabled.");
  }

  async function onToggleStartup(entry: StartupEntry) {
    if (!entry.toggleable) return;
    const next = !entry.enabled;
    await run(
      entry.id,
      () => setStartupEnabled(entry.location, next, entry.command),
      next ? `Enabled startup "${entry.name}".` : `Disabled startup "${entry.name}".`,
    );
  }

  const settableStartTypes = ["auto", "manual", "disabled"];

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Startup &amp; Services</h1>
          <p className="text-sm text-slate-500">
            Review what runs at boot and in the background. Changes are snapshot-first and reversible.
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

      <Card title="Windows Search Index">
        {wsearch && (
          <div className="flex items-center gap-4">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-slate-800">
              <Search className="h-4 w-4 text-cyan-400" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="font-medium text-slate-200">Search Indexer (WSearch)</span>
                {wsearch.running ? (
                  <Badge tone="emerald">running</Badge>
                ) : (
                  <Badge tone="slate">stopped</Badge>
                )}
              </div>
              <p className="mt-0.5 text-xs text-slate-500">
                Disabling frees RAM and background disk activity, but slows Start-menu and
                Explorer search. Start type: {wsearch.startType}.
              </p>
            </div>
            <button
              onClick={onToggleWsearch}
              disabled={busy === "wsearch"}
              className={`rounded-lg px-3 py-1.5 text-xs font-medium text-white transition-colors disabled:opacity-50 ${
                wsearch.enabled
                  ? "bg-rose-600 hover:bg-rose-500"
                  : "bg-cyan-600 hover:bg-cyan-500"
              }`}
            >
              {busy === "wsearch" ? "…" : wsearch.enabled ? "Disable" : "Enable"}
            </button>
          </div>
        )}
      </Card>

      <Card title="Startup Apps">
        {startup.length === 0 ? (
          <p className="text-sm text-slate-500">No startup entries found.</p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {startup.map((e) => (
              <li key={e.id} className="flex items-center gap-3 py-2.5">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-slate-200">{e.name}</span>
                    <Badge tone={e.source === "registry" ? "cyan" : "slate"}>{e.source}</Badge>
                    {e.enabled ? (
                      <Badge tone="emerald">enabled</Badge>
                    ) : (
                      <Badge tone="amber">disabled</Badge>
                    )}
                  </div>
                  <div className="truncate font-mono text-[11px] text-slate-600">{e.command}</div>
                </div>
                <button
                  onClick={() => onToggleStartup(e)}
                  disabled={!e.toggleable || busy === e.id}
                  className={`rounded-lg px-3 py-1.5 text-xs font-medium text-white transition-colors disabled:opacity-40 ${
                    e.enabled ? "bg-rose-600 hover:bg-rose-500" : "bg-cyan-600 hover:bg-cyan-500"
                  }`}
                >
                  {busy === e.id ? "…" : e.enabled ? "Disable" : "Enable"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <Card
        title={`Services (${visible.length} of ${services.length})`}
        action={
          <div className="flex items-center gap-2">
            <div className="relative">
              <Search className="absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-slate-500" />
              <input
                value={query}
                onChange={(e) => setQuery(e.currentTarget.value)}
                placeholder="Filter services…"
                className="w-52 rounded-lg border border-slate-700 bg-slate-950 py-1.5 pl-7 pr-2 text-xs text-slate-100 placeholder:text-slate-600 focus:border-cyan-500 focus:outline-none"
              />
            </div>
            <select
              value={classFilter}
              onChange={(e) => setClassFilter(e.currentTarget.value as ClassFilter)}
              className="rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 text-xs text-slate-200 focus:border-cyan-500 focus:outline-none"
            >
              <option value="all">All</option>
              <option value="required">Required</option>
              <option value="safe">Safe</option>
              <option value="unknown">Unknown</option>
            </select>
          </div>
        }
      >
        <ul className="max-h-[32rem] divide-y divide-slate-800/60 overflow-y-auto">
          {visible.map((s) => (
            <li key={s.name} className="cv-row flex items-center gap-3 py-2.5">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-slate-200">{s.displayName || s.name}</span>
                  <Badge tone={stateTone(s.state)}>{s.state}</Badge>
                  <Badge tone={classTone(s.classification)}>{s.classification}</Badge>
                  {s.delayedAutoStart && <Badge tone="violet">delayed</Badge>}
                </div>
                <div className="truncate text-xs text-slate-500">
                  {s.description && <span>{s.description} · </span>}
                  <span className="font-mono">{s.binaryPath}</span>
                </div>
              </div>

              <select
                value={s.startType}
                disabled={s.isDriver || s.classification === "required" || busy === s.name}
                onChange={(e) =>
                  run(
                    s.name,
                    () => setServiceStartType(s.name, e.currentTarget.value),
                    `Set ${s.name} start type to ${e.currentTarget.value}.`,
                  )
                }
                className="rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 text-xs text-slate-200 focus:border-cyan-500 focus:outline-none disabled:opacity-40"
              >
                {!settableStartTypes.includes(s.startType) && (
                  <option value={s.startType}>{s.startType}</option>
                )}
                {settableStartTypes.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>

              {!s.isDriver && (
                <button
                  onClick={() =>
                    s.state === "running"
                      ? run(s.name, () => stopService(s.name), `Stopped ${s.name}.`)
                      : run(s.name, () => startService(s.name), `Started ${s.name}.`)
                  }
                  disabled={busy === s.name || (s.state === "running" && s.classification === "required")}
                  className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-2.5 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-40"
                >
                  {s.state === "running" ? (
                    <>
                      <Square className="h-3 w-3" /> Stop
                    </>
                  ) : (
                    <>
                      <Play className="h-3 w-3" /> Start
                    </>
                  )}
                </button>
              )}
            </li>
          ))}
        </ul>
      </Card>
    </div>
  );
}
