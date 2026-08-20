import { useCallback, useEffect, useState } from "react";
import { Archive, HardDrive, Plus, RefreshCw, Trash2, Undo2 } from "lucide-react";
import {
  createSnapshot,
  createSystemRestorePoint,
  deleteSnapshot,
  listSnapshots,
  restoreSnapshot,
} from "../lib/api";
import type { Snapshot, SnapshotStatus } from "../lib/types";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";

function statusTone(status: SnapshotStatus): "emerald" | "violet" | "slate" {
  switch (status) {
    case "active":
      return "emerald";
    case "restored":
      return "violet";
    default:
      return "slate";
  }
}

export function Snapshots() {
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [name, setName] = useState("");
  const [reason, setReason] = useState("");
  const [loading, setLoading] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [restorePointBusy, setRestorePointBusy] = useState(false);
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

  async function onCreate() {
    if (!name.trim()) return;
    setLoading(true);
    setError(null);
    setNotice(null);
    try {
      await createSnapshot(name.trim(), reason.trim() || null);
      setName("");
      setReason("");
      setNotice("Snapshot created.");
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }

  async function onRestore(s: Snapshot) {
    setBusyId(s.id);
    setError(null);
    setNotice(null);
    try {
      const n = await restoreSnapshot(s.id);
      setNotice(
        n > 0 ? `Restored ${n} change${n === 1 ? "" : "s"}.` : "No changes to roll back.",
      );
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyId(null);
    }
  }

  async function onCreateRestorePoint() {
    setRestorePointBusy(true);
    setError(null);
    setNotice(null);
    try {
      await createSystemRestorePoint("Optix manual snapshot");
      setNotice("System Restore point created.");
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setRestorePointBusy(false);
    }
  }

  async function onDelete(s: Snapshot) {
    if (!window.confirm(`Delete snapshot "${s.name}"? This cannot be undone.`)) return;
    setBusyId(s.id);
    setError(null);
    try {
      await deleteSnapshot(s.id);
      setNotice("Snapshot deleted.");
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyId(null);
    }
  }

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Snapshots</h1>
          <p className="text-sm text-slate-500">
            Capture the state of your system before making changes.
          </p>
        </div>
        <button
          onClick={onCreateRestorePoint}
          disabled={restorePointBusy}
          title="Create a Windows System Restore point as an extra safety net (requires System Protection to be enabled)"
          className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
        >
          <HardDrive className={`h-4 w-4 ${restorePointBusy ? "animate-pulse" : ""}`} />
          {restorePointBusy ? "Creating…" : "Create Restore point"}
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

      <Card title="New Snapshot">
        <div className="flex flex-col gap-3 sm:flex-row">
          <input
            value={name}
            onChange={(e) => setName(e.currentTarget.value)}
            placeholder="Snapshot name"
            className="flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-slate-100 placeholder:text-slate-600 focus:border-cyan-500 focus:outline-none"
          />
          <input
            value={reason}
            onChange={(e) => setReason(e.currentTarget.value)}
            placeholder="Reason (optional)"
            className="flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-slate-100 placeholder:text-slate-600 focus:border-cyan-500 focus:outline-none"
          />
          <button
            onClick={onCreate}
            disabled={loading || !name.trim()}
            className="flex items-center justify-center gap-2 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
          >
            <Plus className="h-4 w-4" />
            Create
          </button>
        </div>
      </Card>

      <Card title={`Snapshots (${snapshots.length})`}>
        {snapshots.length === 0 ? (
          <div className="flex flex-col items-center gap-2 py-10 text-slate-500">
            <Archive className="h-8 w-8" />
            <p className="text-sm">No snapshots yet.</p>
          </div>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {snapshots.map((s) => (
              <li key={s.id} className="flex items-center gap-4 py-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate font-medium text-slate-200">{s.name}</span>
                    <Badge tone={statusTone(s.status)}>{s.status}</Badge>
                  </div>
                  <div className="mt-0.5 truncate text-xs text-slate-500">
                    {new Date(s.createdAtMs).toLocaleString()}
                    {s.reason ? ` · ${s.reason}` : ""}
                  </div>
                </div>
                <button
                  onClick={() => onRestore(s)}
                  disabled={busyId === s.id}
                  className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
                >
                  <Undo2 className="h-3.5 w-3.5" />
                  Restore
                </button>
                <button
                  onClick={() => onDelete(s)}
                  disabled={busyId === s.id}
                  className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-medium text-rose-300 transition-colors hover:bg-rose-500/20 disabled:opacity-50"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                  Delete
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="mt-2 flex justify-end">
          <button
            onClick={refresh}
            className="flex items-center gap-1.5 text-xs text-slate-500 hover:text-slate-300"
          >
            <RefreshCw className="h-3.5 w-3.5" />
            Refresh
          </button>
        </div>
      </Card>
    </div>
  );
}
