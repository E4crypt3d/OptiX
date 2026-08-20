import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  PackageX,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { removeBloatware, scanBloatware } from "../lib/api";
import { errMsg } from "../lib/errors";
import type { AppxClassification, AppxPackage, BloatwareRemoveResult } from "../lib/types";
import { Badge, Card } from "./ui";

const CLASS_TONE: Record<AppxClassification, "emerald" | "amber" | "slate" | "cyan"> = {
  removal: "emerald",
  caution: "amber",
  protected: "slate",
  unknown: "cyan",
};

const CLASS_LABEL: Record<AppxClassification, string> = {
  removal: "removal candidate",
  caution: "caution",
  protected: "protected",
  unknown: "review only",
};

const FILTERS: { id: AppxClassification | "all"; label: string }[] = [
  { id: "all", label: "All" },
  { id: "removal", label: "Removal" },
  { id: "caution", label: "Caution" },
  { id: "unknown", label: "Review only" },
];

function canRemove(pkg: AppxPackage): boolean {
  return pkg.classification === "removal" || pkg.classification === "caution";
}

export function Bloatware() {
  const [packages, setPackages] = useState<AppxPackage[]>([]);
  const [filter, setFilter] = useState<AppxClassification | "all">("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scanning, setScanning] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<BloatwareRemoveResult | null>(null);
  const selectionInitialized = useRef(false);
  const isWindows =
    typeof navigator !== "undefined" && /windows|win32/i.test(navigator.userAgent);

  const scan = useCallback(async () => {
    setScanning(true);
    setError(null);
    try {
      const pkgs = await scanBloatware();
      setPackages(pkgs);
      setSelected((previous) => {
        const removable = new Set(pkgs.filter(canRemove).map((pkg) => pkg.fullName));
        if (!selectionInitialized.current) {
          selectionInitialized.current = true;
          return new Set(pkgs.filter((pkg) => pkg.classification === "removal").map((pkg) => pkg.fullName));
        }
        return new Set([...previous].filter((name) => removable.has(name)));
      });
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setScanning(false);
    }
  }, []);

  useEffect(() => {
    void scan();
  }, [scan]);

  const visible = useMemo(
    () => (filter === "all" ? packages : packages.filter((pkg) => pkg.classification === filter)),
    [packages, filter],
  );
  const selectedPackages = useMemo(
    () => packages.filter((pkg) => selected.has(pkg.fullName) && canRemove(pkg)),
    [packages, selected],
  );
  const selectedCaution = selectedPackages.filter((pkg) => pkg.classification === "caution");
  const removable = packages.filter(canRemove);
  const removableIds = removable.map((pkg) => pkg.fullName);
  const busy = scanning || removing;

  function toggle(pkg: AppxPackage) {
    if (!canRemove(pkg)) return;
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(pkg.fullName)) next.delete(pkg.fullName);
      else next.add(pkg.fullName);
      return next;
    });
  }

  function selectCandidates() {
    setSelected(new Set(packages.filter((pkg) => pkg.classification === "removal").map((pkg) => pkg.fullName)));
  }

  function selectAllRemovable() {
    setSelected(new Set(removableIds));
  }

  function selectVisible() {
    setSelected((previous) => {
      const next = new Set(previous);
      for (const pkg of visible) {
        if (canRemove(pkg)) next.add(pkg.fullName);
      }
      return next;
    });
  }

  function clearSelection() {
    setSelected(new Set());
  }

  async function onRemove() {
    if (selectedPackages.length === 0) {
      setError("Select at least one removal candidate first.");
      return;
    }
    const cautionWarning = selectedCaution.length
      ? `\n\nCaution packages selected: ${selectedCaution.map((pkg) => pkg.name).join(", ")}.`
      : "";
    const provisionedCount = selectedPackages.filter((pkg) => pkg.provisioned).length;
    const provisionedWarning = provisionedCount
      ? `\n\n${provisionedCount} provisioned package${provisionedCount === 1 ? "" : "s"} will also be removed to prevent reinstalling for new users.`
      : "";
    if (
      !window.confirm(
        `Remove ${selectedPackages.length} package${selectedPackages.length === 1 ? "" : "s"}?${cautionWarning}${provisionedWarning}\n\nA snapshot will be created first. Protected and unknown packages cannot be removed.`,
      )
    ) {
      return;
    }

    setRemoving(true);
    setError(null);
    setResult(null);
    try {
      const next = await removeBloatware(selectedPackages.map((pkg) => pkg.fullName));
      setResult(next);
      setSelected(new Set());
      await scan();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setRemoving(false);
    }
  }

  return (
    <div className="space-y-4">
      <header className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Bloatware</h1>
          <p className="text-sm text-slate-500">
            Review preinstalled Store apps. Protected system and Xbox packages are never removable.
          </p>
        </div>
        <button
          onClick={scan}
          disabled={busy}
          className="flex w-fit items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
        >
          <RefreshCw className={`h-4 w-4 ${scanning ? "animate-spin" : ""}`} />
          {scanning ? "Scanning…" : "Rescan"}
        </button>
      </header>

      {error && (
        <div className="flex items-start gap-2 rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <span className="min-w-0">{error}</span>
        </div>
      )}

      {result && (
        <section className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 p-4 text-sm text-emerald-200">
          <div className="flex items-start gap-3">
            <CheckCircle2 className="mt-0.5 h-5 w-5 shrink-0 text-emerald-400" />
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2 font-medium">
                Removal complete
                <Badge tone="emerald">snapshot {result.snapshotId.slice(0, 8)}</Badge>
              </div>
              <div className="mt-2 text-xs text-emerald-300">
                {result.removed.length} removed · {result.failed.length} failed
              </div>
              {result.failed.length > 0 && (
                <div className="mt-3 space-y-2 border-t border-emerald-500/20 pt-2 text-xs text-amber-300">
                  {result.failed.map((failure) => (
                    <div key={failure.fullName} className="min-w-0">
                      <div className="break-all font-medium">{failure.fullName}</div>
                      <div className="break-words text-amber-300/80">{failure.error || "Unknown removal error"}</div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </section>
      )}

      {!isWindows ? (
        <div className="flex items-start gap-2 rounded-xl border border-slate-800 bg-slate-900/30 px-4 py-3 text-sm text-slate-500">
          <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-slate-400" />
          Store app management is only available on Windows; this page is read-only on this platform.
        </div>
      ) : null}

      <Card
        title={`Packages${packages.length > 0 ? ` · ${packages.length} installed` : ""}`}
        action={
          <div className="flex flex-wrap items-center justify-end gap-1">
            {FILTERS.map((item) => (
              <button
                key={item.id}
                onClick={() => setFilter(item.id)}
                className={`rounded-md px-2 py-1 text-xs font-medium transition-colors ${
                  filter === item.id
                    ? "bg-slate-700 text-slate-100"
                    : "text-slate-400 hover:bg-slate-800 hover:text-slate-200"
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>
        }
      >
        {scanning && packages.length === 0 ? (
          <div className="space-y-3 py-2">
            {[0, 1, 2, 3].map((item) => (
              <div key={item} className="h-14 animate-pulse rounded-lg bg-slate-800/50" />
            ))}
          </div>
        ) : !isWindows ? (
          <div className="flex flex-col items-center gap-2 py-10 text-center text-slate-500">
            <PackageX className="h-8 w-8" />
            <p className="text-sm">No Store package data on this platform.</p>
          </div>
        ) : packages.length === 0 ? (
          <div className="flex flex-col items-center gap-2 py-10 text-center text-slate-500">
            <PackageX className="h-8 w-8" />
            <p className="text-sm font-medium text-slate-400">No packages found</p>
            <p className="text-xs">Try Rescan if Store packages should be present.</p>
          </div>
        ) : visible.length === 0 ? (
          <p className="py-8 text-center text-sm text-slate-500">No packages match this filter.</p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {visible.map((pkg) => {
              const selectable = canRemove(pkg);
              return (
                <li key={pkg.fullName} className="flex items-start gap-3 py-3">
                  <input
                    type="checkbox"
                    checked={selected.has(pkg.fullName)}
                    onChange={() => toggle(pkg)}
                    disabled={!selectable || busy}
                    aria-label={`Select ${pkg.name}`}
                    className="mt-1 h-4 w-4 shrink-0 accent-cyan-500 disabled:opacity-30"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="break-words font-medium text-slate-200">{pkg.name}</span>
                      <Badge tone={CLASS_TONE[pkg.classification]}>{CLASS_LABEL[pkg.classification]}</Badge>
                      {pkg.provisioned && <Badge tone="violet">provisioned</Badge>}
                    </div>
                    <div className="mt-1 truncate text-xs text-slate-500" title={pkg.fullName}>
                      {pkg.publisher || "Unknown publisher"}
                      {pkg.version ? ` · v${pkg.version}` : ""}
                      {pkg.architecture ? ` · ${pkg.architecture}` : ""}
                    </div>
                    <div className="mt-0.5 truncate font-mono text-[11px] text-slate-600" title={pkg.fullName}>
                      {pkg.fullName}
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </Card>

      <div className="flex flex-col gap-3 rounded-xl border border-slate-800 bg-slate-900/40 p-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-wrap items-center gap-2 text-xs text-slate-500">
          <button onClick={selectCandidates} disabled={busy || packages.length === 0} className="hover:text-slate-300 disabled:opacity-50">
            Select candidates
          </button>
          <span>·</span>
          <button onClick={selectAllRemovable} disabled={busy || removable.length === 0} className="hover:text-slate-300 disabled:opacity-50">
            Select all removable
          </button>
          <span>·</span>
          <button onClick={selectVisible} disabled={busy || visible.length === 0} className="hover:text-slate-300 disabled:opacity-50">
            Select visible
          </button>
          <span>·</span>
          <button onClick={clearSelection} disabled={busy || selected.size === 0} className="hover:text-slate-300 disabled:opacity-50">
            Clear
          </button>
          <span className="text-slate-600">{selectedPackages.length} selected</span>
        </div>
        <button
          onClick={onRemove}
          disabled={!isWindows || busy || selectedPackages.length === 0}
          className="flex items-center justify-center gap-2 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
        >
          <Trash2 className="h-4 w-4" />
          {removing ? "Removing…" : `Remove ${selectedPackages.length} selected`}
        </button>
      </div>
    </div>
  );
}
