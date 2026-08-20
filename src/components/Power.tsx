import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  BatteryCharging,
  CheckCircle2,
  Network,
  RefreshCw,
  ShieldCheck,
  Zap,
} from "lucide-react";
import {
  activePowerState,
  applyPowerProfile,
  disableNicPowerSaving,
  listNicAdapters,
  listPowerProfiles,
  listPowerSchemes,
  previewPowerProfile,
} from "../lib/api";
import type {
  ActivePowerState,
  NicAdapter,
  PowerApplyResult,
  PowerPreview,
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

function formatSetting(label: string, value: number): string {
  if (label.includes("state")) return `${value}%`;
  if (label.includes("PCI Express")) {
    return value === 0 ? "Off" : value === 1 ? "Moderate" : value === 2 ? "Max power saving" : String(value);
  }
  if (label.includes("USB")) return value === 0 ? "Disabled" : "Enabled";
  return String(value);
}

export function Power() {
  const [schemes, setSchemes] = useState<PowerScheme[]>([]);
  const [profiles, setProfiles] = useState<PowerProfile[]>([]);
  const [adapters, setAdapters] = useState<NicAdapter[]>([]);
  const [activeState, setActiveState] = useState<ActivePowerState | null>(null);
  const [preview, setPreview] = useState<{ profile: PowerProfile; data: PowerPreview } | null>(null);
  const [previewLoading, setPreviewLoading] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [nicBusy, setNicBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const isWindows =
    typeof navigator !== "undefined" && /windows|win32/i.test(navigator.userAgent);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [nextSchemes, nextProfiles, nextAdapters, nextActive] = await Promise.all([
        listPowerSchemes(),
        listPowerProfiles(),
        listNicAdapters(),
        activePowerState(),
      ]);
      setSchemes(nextSchemes);
      setProfiles(nextProfiles);
      setAdapters(nextAdapters);
      setActiveState(nextActive);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const schemeGuids = useMemo(
    () => new Set(schemes.map((scheme) => scheme.guid.toLowerCase())),
    [schemes],
  );
  const adaptersWithSaving = adapters.filter((adapter) => nicFeatureBadges(adapter).length > 0);
  const busy = loading || busyId !== null || nicBusy;

  async function onPreview(profile: PowerProfile) {
    if (!isWindows) {
      setError("Power plan changes are only available on Windows.");
      return;
    }
    if (!schemeGuids.has(profile.baseGuid.toLowerCase())) {
      setError(`${profile.name} is unavailable because its base power scheme is not installed.`);
      return;
    }
    setError(null);
    setNotice(null);
    setPreviewLoading(profile.id);
    try {
      const data = await previewPowerProfile(profile.id);
      setPreview({ profile, data });
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setPreviewLoading(null);
    }
  }

  async function onApplyConfirm() {
    if (!preview) return;
    setBusyId(preview.profile.id);
    setError(null);
    setNotice(null);
    try {
      const result: PowerApplyResult = await applyPowerProfile(preview.profile.id);
      if (result.alreadyApplied) {
        setNotice(`${result.schemeName} is already applied — no changes made.`);
      } else {
        setNotice(
          `Applied ${result.schemeName}. Snapshot ${result.snapshotId.slice(0, 8)} created — undo from Rollback Center.`,
        );
      }
      setPreview(null);
      await refresh();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusyId(null);
    }
  }

  async function onDisableNic() {
    if (!isWindows) {
      setError("Network adapter power saving changes are only available on Windows.");
      return;
    }
    if (adaptersWithSaving.length === 0) {
      setNotice("No network adapter power-saving settings need changing.");
      return;
    }
    if (!window.confirm("Disable power saving on all detected network adapters? A snapshot is created first.")) return;
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

  return (
    <div className="space-y-4">
      <header className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Power</h1>
          <p className="text-sm text-slate-500">
            Power plans and network power saving. Every change is snapshot-first and reversible.
          </p>
        </div>
        <button
          onClick={refresh}
          disabled={busy}
          className="flex w-fit items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
        >
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          {loading ? "Refreshing…" : "Refresh"}
        </button>
      </header>

      {!isWindows && (
        <div className="flex items-start gap-2 rounded-xl border border-slate-800 bg-slate-900/30 px-4 py-3 text-sm text-slate-500">
          <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-slate-400" />
          Windows power plans and NIC registry controls are unavailable on this platform. The page is read-only.
        </div>
      )}

      <Card title="Current Power State">
        {activeState ? (
          <div className="space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-medium text-slate-200">{activeState.schemeName}</span>
              <Badge tone={activeState.onAc === false ? "amber" : activeState.onAc === true ? "emerald" : "slate"}>
                {activeState.onAc === false
                  ? "On battery"
                  : activeState.onAc === true
                    ? "On AC power"
                    : "Power source unknown"}
              </Badge>
            </div>
            {activeState.onAc === false && (
              <p className="text-xs leading-5 text-amber-300">
                On battery — Optix profile changes apply to AC only; battery settings are left untouched.
              </p>
            )}
            {activeState.settings.length > 0 ? (
              <ul className="divide-y divide-slate-800/60">
                {activeState.settings.map((s) => {
                  const matches = s.current === s.optixTarget;
                  return (
                    <li key={s.label} className="flex items-center justify-between gap-3 py-2 text-sm">
                      <span className="text-slate-300">{s.label}</span>
                      <span className="flex items-baseline gap-2 text-right">
                        <span className={`tabular-nums ${matches ? "text-emerald-300" : "text-amber-300"}`}>
                          {formatSetting(s.label, s.current)}
                        </span>
                        {!matches && (
                          <span className="text-[11px] text-slate-500">→ Optix: {formatSetting(s.label, s.optixTarget)}</span>
                        )}
                      </span>
                    </li>
                  );
                })}
              </ul>
            ) : (
              <p className="text-sm text-slate-500">No tracked settings readable on this system.</p>
            )}
          </div>
        ) : (
          <p className="text-sm text-slate-500">
            {isWindows ? "Reading power state…" : "Power state is only available on Windows."}
          </p>
        )}
      </Card>

      {error && (
        <div className="flex items-start gap-2 rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <span className="min-w-0">{error}</span>
        </div>
      )}
      {notice && (
        <div className="flex items-start gap-2 rounded-xl border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-300">
          <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0" />
          <span className="min-w-0">{notice}</span>
        </div>
      )}

      <Card title="Optix Power Profiles" action={<span className="text-xs text-slate-500">{profiles.length} profiles</span>}>
        {loading && profiles.length === 0 ? (
          <div className="space-y-3 py-2">
            {[0, 1, 2].map((item) => <div key={item} className="h-16 animate-pulse rounded-lg bg-slate-800/50" />)}
          </div>
        ) : profiles.length === 0 ? (
          <p className="py-8 text-center text-sm text-slate-500">No Optix profiles available.</p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {profiles.map((profile) => {
              const available = isWindows && schemeGuids.has(profile.baseGuid.toLowerCase());
              return (
                <li key={profile.id} className="flex items-start gap-3 py-3">
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-slate-800">
                    <BatteryCharging className="h-4 w-4 text-cyan-400" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-medium text-slate-200">{profile.name}</span>
                      {available ? <Badge tone="emerald">available</Badge> : <Badge tone="slate">unavailable</Badge>}
                    </div>
                    <div className="mt-1 text-xs leading-5 text-slate-500">{profile.description}</div>
                    <div className="mt-0.5 text-xs text-slate-600">{profile.note}</div>
                    {!available && isWindows && (
                      <div className="mt-1 text-xs text-amber-400">Base scheme is not installed on this PC.</div>
                    )}
                  </div>
                  <button
                    onClick={() => onPreview(profile)}
                    disabled={!available || busy}
                    className="flex shrink-0 items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
                  >
                    <Zap className="h-3.5 w-3.5" />
                    {previewLoading === profile.id ? "Checking…" : "Apply"}
                  </button>
                </li>
              );
            })}
          </ul>
        )}

        {preview && (
          <div className="mt-4 rounded-lg border border-cyan-500/30 bg-cyan-500/5 p-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="text-sm font-medium text-slate-100">
                  Apply "{preview.data.profileName}"?
                </div>
                <div className="mt-0.5 text-xs leading-5 text-slate-500">
                  Clones {preview.data.baseSchemeName}, writes AC settings, then sets it active.
                  Currently active: {preview.data.currentSchemeName}.
                </div>
              </div>
              <button
                onClick={() => setPreview(null)}
                className="shrink-0 text-xs text-slate-500 hover:text-slate-300"
              >
                Cancel
              </button>
            </div>

            {preview.data.alreadyApplied ? (
              <p className="mt-3 text-sm text-emerald-300">
                This profile is already applied — no changes needed.
              </p>
            ) : preview.data.changes.length === 0 ? (
              <p className="mt-3 text-sm text-slate-400">
                No tracked settings will change; applying would only re-activate the scheme.
              </p>
            ) : (
              <ul className="mt-3 space-y-1.5">
                {preview.data.changes.map((c) => (
                  <li key={c.label} className="flex items-center justify-between gap-3 text-sm">
                    <span className="text-slate-300">{c.label}</span>
                    <span className="tabular-nums text-slate-400">
                      {formatSetting(c.label, c.current)}{" "}
                      <span className="text-slate-600">→</span>{" "}
                      <span className="text-emerald-300">{formatSetting(c.label, c.optixTarget)}</span>
                    </span>
                  </li>
                ))}
              </ul>
            )}

            <div className="mt-4 flex justify-end gap-2">
              <button
                onClick={() => setPreview(null)}
                className="rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:bg-slate-700"
              >
                Cancel
              </button>
              <button
                onClick={onApplyConfirm}
                disabled={busyId !== null || preview.data.alreadyApplied}
                className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
              >
                <Zap className="h-3.5 w-3.5" />
                {busyId !== null ? "Applying…" : "Apply profile"}
              </button>
            </div>
          </div>
        )}
      </Card>

      <Card title={`Power Schemes${schemes.length > 0 ? ` · ${schemes.length} detected` : ""}`}>
        {loading && schemes.length === 0 ? (
          <div className="space-y-3 py-2">
            {[0, 1].map((item) => <div key={item} className="h-12 animate-pulse rounded-lg bg-slate-800/50" />)}
          </div>
        ) : schemes.length === 0 ? (
          <p className="py-6 text-center text-sm text-slate-500">No Windows power schemes reported.</p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {schemes.map((scheme) => (
              <li key={scheme.guid} className="flex items-center gap-3 py-2.5">
                <div className="min-w-0 flex-1">
                  <span className="font-medium text-slate-200">{scheme.name}</span>
                  <div className="truncate font-mono text-[11px] text-slate-600" title={scheme.guid}>{scheme.guid}</div>
                </div>
                {scheme.isActive && <Badge tone="emerald">active</Badge>}
              </li>
            ))}
          </ul>
        )}
      </Card>

      <Card
        title={`Network Adapter Power Saving${adapters.length > 0 ? ` · ${adaptersWithSaving.length} to fix` : ""}`}
        action={
          <button
            onClick={onDisableNic}
            disabled={!isWindows || busy || adaptersWithSaving.length === 0}
            className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
          >
            <Network className="h-3.5 w-3.5" />
            {nicBusy ? "Applying…" : "Disable power saving"}
          </button>
        }
      >
        {loading && adapters.length === 0 ? (
          <div className="space-y-3 py-2">
            {[0, 1].map((item) => <div key={item} className="h-12 animate-pulse rounded-lg bg-slate-800/50" />)}
          </div>
        ) : adapters.length === 0 ? (
          <p className="py-6 text-center text-sm text-slate-500">No network adapters detected.</p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {adapters.map((adapter) => {
              const features = nicFeatureBadges(adapter);
              return (
                <li key={adapter.key} className="flex items-start gap-3 py-2.5">
                  <Network className="mt-0.5 h-4 w-4 shrink-0 text-cyan-400" />
                  <div className="min-w-0 flex-1">
                    <span className="break-words font-medium text-slate-200">{adapter.name}</span>
                    <div className="mt-1 flex flex-wrap gap-1.5">
                      {features.length === 0 ? (
                        <Badge tone="slate">power saving off</Badge>
                      ) : (
                        features.map((feature) => <Badge key={feature} tone="amber">{feature}</Badge>)
                      )}
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
        <p className="mt-3 text-xs leading-5 text-slate-600">
          Energy Efficient Ethernet and device power management can add latency spikes and packet drops during gaming. Impact is modest on wired connections; measure before and after changing it.
        </p>
      </Card>
    </div>
  );
}
