import { useCallback, useEffect, useState } from "react";
import { BatteryCharging, Network, RefreshCw, Zap } from "lucide-react";
import {
  applyPowerProfile,
  disableNicPowerSaving,
  listNicAdapters,
  listPowerProfiles,
  listPowerSchemes,
} from "../lib/api";
import type {
  NicAdapter,
  PowerApplyResult,
  PowerProfile,
  PowerScheme,
} from "../lib/types";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";

function nicFeatureBadges(adapter: NicAdapter) {
  const features: string[] = [];
  if (adapter.eee === 1) features.push("EEE");
  if (adapter.greenEthernet === 1) features.push("Green Ethernet");
  if (adapter.powerManagement === 1) features.push("Power Mgmt");
  if (adapter.pnpCapabilities !== null && adapter.pnpCapabilities !== 24) {
    features.push("Allow power off");
  }
  return features;
}

export function Power() {
  const [schemes, setSchemes] = useState<PowerScheme[]>([]);
  const [profiles, setProfiles] = useState<PowerProfile[]>([]);
  const [adapters, setAdapters] = useState<NicAdapter[]>([]);
  const [loading, setLoading] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [nicBusy, setNicBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [s, p, a] = await Promise.all([
        listPowerSchemes(),
        listPowerProfiles(),
        listNicAdapters(),
      ]);
      setSchemes(s);
      setProfiles(p);
      setAdapters(a);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function onApply(profile: PowerProfile) {
    if (!window.confirm(`Apply "${profile.name}" power profile? A snapshot is created first.`)) {
      return;
    }
    setBusyId(profile.id);
    setError(null);
    setNotice(null);
    try {
      const result: PowerApplyResult = await applyPowerProfile(profile.id);
      setNotice(
        `Applied ${result.schemeName}. Snapshot ${result.snapshotId.slice(0, 8)} created — undo from Rollback Center.`,
      );
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyId(null);
    }
  }

  async function onDisableNic() {
    if (!window.confirm("Disable power saving on all network adapters? A snapshot is created first.")) {
      return;
    }
    setNicBusy(true);
    setError(null);
    setNotice(null);
    try {
      const result = await disableNicPowerSaving();
      setNotice(
        result.changes > 0
          ? `Disabled power saving on ${result.adaptersChanged} adapter${result.adaptersChanged === 1 ? "" : "s"} (${result.changes} value${result.changes === 1 ? "" : "s"}). Reversible via Rollback Center.`
          : "No adapter power-saving settings needed changing.",
      );
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setNicBusy(false);
    }
  }

  const adaptersWithSaving = adapters.filter((a) => nicFeatureBadges(a).length > 0);

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Power</h1>
          <p className="text-sm text-slate-500">
            Power plans and network power saving. Every change is snapshot-first and reversible.
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

      <Card title="Optix Power Profiles">
        <ul className="divide-y divide-slate-800/60">
          {profiles.map((p) => (
            <li key={p.id} className="flex items-center gap-4 py-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-slate-800">
                <BatteryCharging className="h-4 w-4 text-cyan-400" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="font-medium text-slate-200">{p.name}</div>
                <div className="text-xs text-slate-500">{p.description}</div>
                <div className="mt-0.5 text-xs text-slate-600">{p.note}</div>
              </div>
              <button
                onClick={() => onApply(p)}
                disabled={busyId === p.id}
                className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
              >
                <Zap className="h-3.5 w-3.5" />
                {busyId === p.id ? "Applying…" : "Apply"}
              </button>
            </li>
          ))}
        </ul>
      </Card>

      <Card title={`Power Schemes (${schemes.length})`}>
        {schemes.length === 0 ? (
          <p className="text-sm text-slate-500">
            No schemes reported (power management is Windows-only).
          </p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {schemes.map((s) => (
              <li key={s.guid} className="flex items-center gap-3 py-2.5">
                <div className="min-w-0 flex-1">
                  <span className="font-medium text-slate-200">{s.name}</span>
                  <div className="truncate font-mono text-[11px] text-slate-600">{s.guid}</div>
                </div>
                {s.isActive && <Badge tone="emerald">active</Badge>}
              </li>
            ))}
          </ul>
        )}
      </Card>

      <Card
        title={`Network Adapter Power Saving (${adaptersWithSaving.length} to fix)`}
        action={
          <button
            onClick={onDisableNic}
            disabled={nicBusy || adaptersWithSaving.length === 0}
            className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
          >
            <Network className="h-3.5 w-3.5" />
            {nicBusy ? "Applying…" : "Disable power saving"}
          </button>
        }
      >
        {adapters.length === 0 ? (
          <p className="text-sm text-slate-500">
            No network adapters detected (Windows-only).
          </p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {adapters.map((a) => {
              const features = nicFeatureBadges(a);
              return (
                <li key={a.key} className="flex items-center gap-3 py-2.5">
                  <div className="min-w-0 flex-1">
                    <span className="font-medium text-slate-200">{a.name}</span>
                    <div className="mt-0.5 flex flex-wrap gap-1.5">
                      {features.length === 0 ? (
                        <Badge tone="slate">power saving off</Badge>
                      ) : (
                        features.map((f) => (
                          <Badge key={f} tone="amber">
                            {f}
                          </Badge>
                        ))
                      )}
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
        <p className="mt-3 text-xs text-slate-600">
          Energy Efficient Ethernet and device power management can add latency spikes and
          packet drops during gaming. Impact is modest on wired connections — measure with a
          benchmark to confirm.
        </p>
      </Card>
    </div>
  );
}
