import { useCallback, useEffect, useState } from "react";
import { ArrowLeftRight, Undo2 } from "lucide-react";
import {
  diffSnapshots,
  listChanges,
  listSnapshots,
  restoreSnapshot,
} from "../lib/api";
import type { ChangeRecord, Snapshot } from "../lib/types";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";

interface DiffEntry {
  path: string;
  kind: string;
  old?: unknown;
  new?: unknown;
}

function stringify(v: unknown): string {
  if (v == null) return "null";
  if (typeof v === "string") return v;
  return JSON.stringify(v);
}

export function Rollback() {
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [changes, setChanges] = useState<ChangeRecord[]>([]);
  const [diffA, setDiffA] = useState("");
  const [diffB, setDiffB] = useState("");
  const [diff, setDiff] = useState<DiffEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setSnapshots(await listSnapshots());
    } catch (e) {
      setError(errMsg(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!selectedId) {
      setChanges([]);
      return;
    }
    listChanges(selectedId)
      .then(setChanges)
      .catch((e) => setError(errMsg(e)));
  }, [selectedId]);

  async function onRestore() {
    if (!selectedId) return;
    setError(null);
    setNotice(null);
    try {
      const n = await restoreSnapshot(selectedId);
      setNotice(
        n > 0 ? `Restored ${n} change${n === 1 ? "" : "s"}.` : "No changes to roll back.",
      );
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    }
  }

  async function onDiff() {
    if (!diffA || !diffB) return;
    setError(null);
    try {
      setDiff((await diffSnapshots(diffA, diffB)) as DiffEntry[]);
    } catch (e) {
      setError(errMsg(e));
    }
  }

  return (
    <div className="space-y-4">
      <header>
        <h1 className="text-xl font-semibold text-slate-100">Rollback Center</h1>
        <p className="text-sm text-slate-500">
          Inspect and undo every tracked change.
        </p>
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

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <Card title="Choose a snapshot">
          <div className="space-y-1">
            {snapshots.length === 0 && (
              <p className="text-sm text-slate-500">No snapshots yet.</p>
            )}
            {snapshots.map((s) => (
              <button
                key={s.id}
                onClick={() => setSelectedId(s.id)}
                className={`flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition-colors ${
                  selectedId === s.id
                    ? "bg-slate-800 text-slate-100"
                    : "text-slate-400 hover:bg-slate-900 hover:text-slate-200"
                }`}
              >
                <span className="truncate">{s.name}</span>
                <Badge tone={s.status === "active" ? "emerald" : s.status === "restored" ? "violet" : "slate"}>
                  {s.status}
                </Badge>
              </button>
            ))}
          </div>
        </Card>

        <Card
          title="Changes"
          action={
            <button
              onClick={onRestore}
              disabled={!selectedId}
              className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
            >
              <Undo2 className="h-3.5 w-3.5" />
              Restore
            </button>
          }
        >
          {changes.length === 0 ? (
            <p className="text-sm text-slate-500">
              {selectedId ? "No changes recorded for this snapshot." : "Select a snapshot to inspect."}
            </p>
          ) : (
            <ul className="divide-y divide-slate-800/60">
              {changes.map((c) => (
                <li key={c.id ?? c.location} className="py-2 text-sm">
                  <div className="flex items-center gap-2">
                    <Badge tone="cyan">{c.domain}</Badge>
                    <Badge tone="slate">{c.kind}</Badge>
                    <span className="truncate font-mono text-xs text-slate-400">
                      {c.location}
                    </span>
                  </div>
                  <div className="mt-1 text-xs text-slate-500">
                    <span className="text-rose-400">{stringify(c.oldValue)}</span>
                    {" → "}
                    <span className="text-emerald-400">{stringify(c.newValue)}</span>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>

      <Card title="Compare snapshots">
        <div className="flex flex-col gap-3 sm:flex-row">
          <select
            value={diffA}
            onChange={(e) => setDiffA(e.currentTarget.value)}
            className="flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-slate-100 focus:border-cyan-500 focus:outline-none"
          >
            <option value="">Select snapshot A…</option>
            {snapshots.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
          <select
            value={diffB}
            onChange={(e) => setDiffB(e.currentTarget.value)}
            className="flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-slate-100 focus:border-cyan-500 focus:outline-none"
          >
            <option value="">Select snapshot B…</option>
            {snapshots.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
          <button
            onClick={onDiff}
            disabled={!diffA || !diffB}
            className="flex items-center justify-center gap-2 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
          >
            <ArrowLeftRight className="h-4 w-4" />
            Compare
          </button>
        </div>

        {diff.length > 0 && (
          <ul className="mt-4 divide-y divide-slate-800/60">
            {diff.map((d, i) => (
              <li key={i} className="py-2 text-sm">
                <div className="flex items-center gap-2">
                  <Badge
                    tone={d.kind === "changed" ? "amber" : d.kind === "added" ? "emerald" : "rose"}
                  >
                    {d.kind}
                  </Badge>
                  <span className="font-mono text-xs text-slate-400">{d.path}</span>
                </div>
                <div className="mt-1 text-xs text-slate-500">
                  {d.old != null && <span className="text-rose-400">{stringify(d.old)}</span>}
                  {d.old != null && d.new != null && " → "}
                  {d.new != null && <span className="text-emerald-400">{stringify(d.new)}</span>}
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
