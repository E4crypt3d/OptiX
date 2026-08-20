import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { useInterval } from "../lib/useInterval";
import {
  FolderOpen,
  Gamepad2,
  Play,
  Plus,
  RefreshCw,
  ScanSearch,
  Settings2,
  Trash2,
  Undo2,
  X,
} from "lucide-react";
import {
  addGame,
  addManualGame,
  applyGameProfile,
  detectGames,
  getGameProfile,
  listGames,
  removeGame,
  restoreGameProfile,
  saveGameProfile,
} from "../lib/api";
import type { DetectedGame, Game, GameProfile } from "../lib/types";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";

const LAUNCHER_TONE: Record<string, "cyan" | "violet" | "emerald" | "amber" | "rose" | "slate"> = {
  steam: "cyan",
  epic: "violet",
  riot: "rose",
  battlenet: "amber",
  manual: "slate",
};

const CPU_PRIORITIES = [
  ["normal", "Normal"],
  ["above_normal", "Above normal"],
  ["high", "High"],
] as const;

const POWER_PROFILES = [
  ["none", "None"],
  ["balanced_gaming", "Balanced Gaming"],
  ["competitive_gaming", "Competitive Gaming"],
  ["maximum_performance", "Maximum Performance"],
] as const;

const NETWORK_PROFILES = [
  ["none", "None"],
  ["dns", "DNS"],
  ["tcp_experimental", "TCP (experimental)"],
] as const;

export function Games() {
  const [games, setGames] = useState<Game[]>([]);
  const [detected, setDetected] = useState<DetectedGame[]>([]);
  const [scanning, setScanning] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [draft, setDraft] = useState<GameProfile | null>(null);
  const [manualName, setManualName] = useState("");
  const [manualExe, setManualExe] = useState("");

  const refresh = useCallback(async () => {
    try {
      const g = await listGames();
      setGames(g);
    } catch (e) {
      setError(errMsg(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Light polling so running/boosted badges track the background watcher.
  // Paused while the window is hidden to avoid needless process enumeration.
  useInterval(() => void refresh(), 3000);

  async function onScan() {
    setScanning(true);
    setError(null);
    setNotice(null);
    try {
      const d = await detectGames();
      setDetected(d);
      setNotice(
        d.length > 0
          ? `Found ${d.length} game${d.length === 1 ? "" : "s"}. Add the ones you want to optimize.`
          : "No games found. Add one manually below.",
      );
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setScanning(false);
    }
  }

  async function onAddDetected(d: DetectedGame) {
    setBusy(d.name);
    setError(null);
    try {
      await addGame(d.launcher, d.appId, d.name, d.installPath, d.executable);
      setDetected((prev) => prev.filter((x) => x !== d));
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  async function onAddManual() {
    if (!manualName.trim() || !manualExe.trim()) return;
    setBusy("manual");
    setError(null);
    try {
      await addManualGame(manualName.trim(), manualExe.trim());
      setManualName("");
      setManualExe("");
      setNotice("Game added.");
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  async function onBrowseExe() {
    const selected = await open({
      multiple: false,
      directory: false,
      title: "Select game executable",
      filters: [{ name: "Executables", extensions: ["exe", "cmd", "bat"] }],
    });
    if (typeof selected === "string") {
      setManualExe(selected);
      setError(null);
    }
  }

  async function onRemove(g: Game) {
    if (!window.confirm(`Remove "${g.name}" from the library?`)) return;
    setBusy(`remove:${g.id}`);
    setError(null);
    try {
      await removeGame(g.id);
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  async function onApply(g: Game) {
    setBusy(`apply:${g.id}`);
    setError(null);
    setNotice(null);
    try {
      const r = await applyGameProfile(g.id);
      const parts: string[] = [];
      if (r.powerApplied) parts.push(`power "${r.powerApplied}"`);
      if (r.boosted.length) parts.push(`${r.boosted.length} boosted`);
      if (r.lowered.length) parts.push(`${r.lowered.length} background lowered`);
      if (r.affinityApplied.length) parts.push("affinity set");
      if (r.gpuProfile) parts.push(`NVIDIA profile "${r.gpuProfile}"`);
      setNotice(
        parts.length
          ? `Applied profile: ${parts.join(", ")}.`
          : "Profile applied (no running process to adjust).",
      );
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  async function onRestore(g: Game) {
    setBusy(`restore:${g.id}`);
    setError(null);
    try {
      await restoreGameProfile(g.id);
      setNotice(`Restored "${g.name}" to its previous state.`);
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  async function onEdit(g: Game) {
    if (editingId === g.id) {
      setEditingId(null);
      setDraft(null);
      return;
    }
    setError(null);
    try {
      const p = await getGameProfile(g.id);
      setDraft(p);
      setEditingId(g.id);
    } catch (e) {
      setError(errMsg(e));
    }
  }

  async function onSaveProfile() {
    if (!draft) return;
    setBusy(`save:${draft.gameId}`);
    setError(null);
    try {
      await saveGameProfile(draft);
      setNotice("Profile saved. The watcher applies it on launch.");
      setEditingId(null);
      setDraft(null);
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Game Profiles</h1>
          <p className="text-sm text-slate-500">
            Detected games and per-game optimization. The watcher auto-applies profiles on launch.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={onScan}
            disabled={scanning}
            className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
          >
            <ScanSearch className={`h-4 w-4 ${scanning ? "animate-pulse" : ""}`} />
            {scanning ? "Scanning…" : "Scan for games"}
          </button>
          <button
            onClick={refresh}
            className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700"
          >
            <RefreshCw className="h-4 w-4" />
            Refresh
          </button>
        </div>
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

      {detected.length > 0 && (
        <Card
          title={`Detected Games (${detected.length})`}
          action={
            <button
              onClick={async () => {
                for (const d of [...detected]) await onAddDetected(d);
              }}
              className="rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500"
            >
              Add all
            </button>
          }
        >
          <ul className="divide-y divide-slate-800/60">
            {detected.map((d) => (
              <li key={`${d.launcher}:${d.name}`} className="flex items-center gap-3 py-2.5">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-slate-200">{d.name}</span>
                    <Badge tone={LAUNCHER_TONE[d.launcher] ?? "slate"}>{d.launcher}</Badge>
                    {d.executable === "" && <Badge tone="amber">no exe detected</Badge>}
                  </div>
                  <div className="truncate text-xs text-slate-500">{d.installPath}</div>
                  {d.executable && (
                    <div className="truncate font-mono text-[11px] text-slate-600">{d.executable}</div>
                  )}
                </div>
                <button
                  onClick={() => onAddDetected(d)}
                  disabled={busy === d.name}
                  className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
                >
                  <Plus className="h-3.5 w-3.5" />
                  {busy === d.name ? "Adding…" : "Add"}
                </button>
              </li>
            ))}
          </ul>
        </Card>
      )}

      <Card title="Add Manually">
        <div className="flex flex-wrap items-center gap-2">
          <input
            value={manualName}
            onChange={(e) => setManualName(e.currentTarget.value)}
            placeholder="Game name (e.g. Apex Legends)"
            className="w-56 rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 text-xs text-slate-100 placeholder:text-slate-600 focus:border-cyan-500 focus:outline-none"
          />
          <input
            value={manualExe}
            onChange={(e) => setManualExe(e.currentTarget.value)}
            placeholder="Executable path (e.g. C:\\Games\\r5apex.exe)"
            className="flex-1 min-w-64 rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 font-mono text-xs text-slate-100 placeholder:text-slate-600 focus:border-cyan-500 focus:outline-none"
          />
          <button
            onClick={onBrowseExe}
            title="Browse for the game executable"
            className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700"
          >
            <FolderOpen className="h-3.5 w-3.5" />
            Browse…
          </button>
          <button
            onClick={onAddManual}
            disabled={busy === "manual" || !manualName.trim() || !manualExe.trim()}
            className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
          >
            <Plus className="h-3.5 w-3.5" />
            {busy === "manual" ? "Adding…" : "Add"}
          </button>
        </div>
      </Card>

      <Card title={`Library (${games.length})`}>
        {games.length === 0 ? (
          <p className="text-sm text-slate-500">
            No games yet. Scan for games or add one manually.
          </p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {games.map((g) => (
              <li key={g.id}>
                <div className="flex items-center gap-3 py-2.5">
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-slate-800">
                    <Gamepad2 className="h-4 w-4 text-cyan-400" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-medium text-slate-200">{g.name}</span>
                      <Badge tone={LAUNCHER_TONE[g.launcher] ?? "slate"}>{g.launcher}</Badge>
                      {g.running && <Badge tone="emerald">running</Badge>}
                      {g.boosted && <Badge tone="violet">boosted</Badge>}
                      {g.exeName === "" && <Badge tone="amber">no exe</Badge>}
                    </div>
                    <div className="truncate font-mono text-[11px] text-slate-600">
                      {g.executable || g.installPath}
                    </div>
                  </div>

                  <button
                    onClick={() => onEdit(g)}
                    className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-2.5 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700"
                  >
                    <Settings2 className="h-3 w-3" />
                    Profile
                  </button>
                  <button
                    onClick={() => onApply(g)}
                    disabled={busy === `apply:${g.id}`}
                    className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-2.5 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
                  >
                    <Play className="h-3 w-3" />
                    {busy === `apply:${g.id}` ? "…" : "Apply"}
                  </button>
                  {g.boosted && (
                    <button
                      onClick={() => onRestore(g)}
                      disabled={busy === `restore:${g.id}`}
                      className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-2.5 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
                    >
                      <Undo2 className="h-3 w-3" />
                      Restore
                    </button>
                  )}
                  <button
                    onClick={() => onRemove(g)}
                    disabled={busy === `remove:${g.id}`}
                    className="flex items-center gap-1.5 rounded-lg bg-slate-800 px-2.5 py-1.5 text-xs font-medium text-rose-300 transition-colors hover:bg-slate-700 disabled:opacity-50"
                  >
                    <Trash2 className="h-3 w-3" />
                  </button>
                </div>

                {editingId === g.id && draft && (
                  <div className="mb-3 rounded-lg border border-slate-800 bg-slate-950/60 p-3">
                    <div className="mb-2 flex items-center justify-between">
                      <span className="text-xs font-semibold uppercase tracking-wider text-slate-400">
                        Profile — {g.name}
                      </span>
                      <button
                        onClick={() => onEdit(g)}
                        className="text-slate-500 hover:text-slate-300"
                      >
                        <X className="h-4 w-4" />
                      </button>
                    </div>
                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
                      <label className="text-xs text-slate-400">
                        CPU priority
                        <select
                          value={draft.cpuPriority}
                          onChange={(e) =>
                            setDraft({ ...draft, cpuPriority: e.currentTarget.value })
                          }
                          className="mt-1 w-full rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 text-xs text-slate-200 focus:border-cyan-500 focus:outline-none"
                        >
                          {CPU_PRIORITIES.map(([v, label]) => (
                            <option key={v} value={v}>
                              {label}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label className="text-xs text-slate-400">
                        CPU affinity (hex mask, optional)
                        <input
                          value={draft.affinityMask ?? ""}
                          onChange={(e) =>
                            setDraft({
                              ...draft,
                              affinityMask: e.currentTarget.value || null,
                            })
                          }
                          placeholder="e.g. 0x5555"
                          className="mt-1 w-full rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 font-mono text-xs text-slate-200 placeholder:text-slate-600 focus:border-cyan-500 focus:outline-none"
                        />
                      </label>
                      <label className="text-xs text-slate-400">
                        Power profile
                        <select
                          value={draft.powerProfile}
                          onChange={(e) =>
                            setDraft({ ...draft, powerProfile: e.currentTarget.value })
                          }
                          className="mt-1 w-full rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 text-xs text-slate-200 focus:border-cyan-500 focus:outline-none"
                        >
                          {POWER_PROFILES.map(([v, label]) => (
                            <option key={v} value={v}>
                              {label}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label className="text-xs text-slate-400">
                        Network profile
                        <select
                          value={draft.networkProfile}
                          onChange={(e) =>
                            setDraft({ ...draft, networkProfile: e.currentTarget.value })
                          }
                          className="mt-1 w-full rounded-lg border border-slate-700 bg-slate-950 px-2 py-1.5 text-xs text-slate-200 focus:border-cyan-500 focus:outline-none"
                        >
                          {NETWORK_PROFILES.map(([v, label]) => (
                            <option key={v} value={v}>
                              {label}
                            </option>
                          ))}
                        </select>
                      </label>
                    </div>
                    <div className="mt-3 flex flex-wrap items-center gap-4">
                      <label className="flex items-center gap-2 text-xs text-slate-300">
                        <input
                          type="checkbox"
                          checked={draft.cleanupBg}
                          onChange={(e) => setDraft({ ...draft, cleanupBg: e.currentTarget.checked })}
                          className="h-4 w-4 accent-cyan-500"
                        />
                        Lower background apps while playing
                      </label>
                      <label className="flex items-center gap-2 text-xs text-slate-300">
                        <input
                          type="checkbox"
                          checked={draft.enabled}
                          onChange={(e) => setDraft({ ...draft, enabled: e.currentTarget.checked })}
                          className="h-4 w-4 accent-cyan-500"
                        />
                        Auto-apply on launch
                      </label>
                      <button
                        onClick={onSaveProfile}
                        disabled={busy === `save:${draft.gameId}`}
                        className="ml-auto rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
                      >
                        {busy === `save:${draft.gameId}` ? "Saving…" : "Save profile"}
                      </button>
                    </div>
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
