import { useCallback, useEffect, useMemo, useState } from "react";
import { PackageX, RefreshCw, Trash2 } from "lucide-react";
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
  unknown: "unknown",
};

const FILTERS: { id: AppxClassification | "all"; label: string }[] = [
  { id: "all", label: "All" },
  { id: "removal", label: "Removal" },
  { id: "caution", label: "Caution" },
  { id: "unknown", label: "Unknown" },
];

export function Bloatware() {
  const [packages, setPackages] = useState<AppxPackage[]>([]);
  const [filter, setFilter] = useState<AppxClassification | "all">("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scanning, setScanning] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<BloatwareRemoveResult | null>(null);

  const scan = useCallback(async () => {
    setScanning(true);
    setError(null);
    try {
      const pkgs = await scanBloatware();
      setPackages(pkgs);
      // Pre-select the clear removal candidates, never protected packages.
      setSelected(new Set(pkgs.filter((p) => p.classification === "removal").map((p) => p.fullName)));
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
    () => (filter === "all" ? packages : packages.filter((p) => p.classification === filter)),
    [packages, filter],
  );

  function toggle(fullName: string, protectedPkg: boolean) {
    if (protectedPkg) return;
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(fullName)) next.delete(fullName);
      else next.add(fullName);
      return next;
    });
  }

  async function onRemove() {
    const names = packages.filter((p) => selected.has(p.fullName)).map((p) => p.fullName);
    if (names.length === 0) return;
    if (
      !window.confirm(
        `Remove ${names.length} package${names.length === 1 ? "" : "s"}? ` +
          "Provisioned copies are removed too so they don't reinstall. This can be reverted from the Rollback Center.",
      )
    ) {
      return;
    }
    setRemoving(true);
    setError(null);
    setResult(null);
    try {
      const r = await removeBloatware(names);
      setResult(r);
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
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">Bloatware</h1>
          <p className="text-sm text-slate-500">
            Review preinstalled Store apps. Removal is snapshot-first and reversible; core system and
            Xbox packages are never flagged.
          </p>
        </div>
        <button
          onClick={scan}
          disabled={scanning}
          className="flex items-center gap-2 rounded-lg bg-slate-800 px-3 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700 disabled:opacity-50"
        >
          <RefreshCw className={`h-4 w-4 ${scanning ? "animate-spin" : ""}`} />
          {scanning ? "Scanning…" : "Rescan"}
        </button>
      </header>

      {error && (
        <div className="rounded-lg border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
          {error}
        </div>
      )}

      {result && (
        <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-300">
          Removed {result.removed.length} package{result.removed.length === 1 ? "" : "s"}.
          {result.failed.length > 0 && (
            <span className="ml-1 text-amber-300">
              {result.failed.length} failed ({result.failed.map((f) => f.fullName).join(", ")}).
            </span>
          )}{" "}
          Snapshot {result.snapshotId.slice(0, 8)} created.
        </div>
      )}

      <Card
        title={`Packages (${packages.length})`}
        action={
          <div className="flex items-center gap-1">
            {FILTERS.map((f) => (
              <button
                key={f.id}
                onClick={() => setFilter(f.id)}
                className={`rounded-md px-2 py-1 text-xs font-medium transition-colors ${
                  filter === f.id
                    ? "bg-slate-700 text-slate-100"
                    : "text-slate-400 hover:bg-slate-800 hover:text-slate-200"
                }`}
              >
                {f.label}
              </button>
            ))}
          </div>
        }
      >
        {packages.length === 0 && !scanning ? (
          <div className="flex flex-col items-center gap-2 py-10 text-slate-500">
            <PackageX className="h-8 w-8" />
            <p className="text-sm">No packages found.</p>
          </div>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {visible.map((p) => {
              const protectedPkg = p.classification === "protected";
              return (
                <li key={p.fullName} className="flex items-center gap-3 py-3">
                  <input
                    type="checkbox"
                    checked={selected.has(p.fullName)}
                    onChange={() => toggle(p.fullName, protectedPkg)}
                    disabled={protectedPkg}
                    className="h-4 w-4 shrink-0 accent-cyan-500 disabled:opacity-30"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="truncate font-medium text-slate-200">{p.name}</span>
                      <Badge tone={CLASS_TONE[p.classification]}>{CLASS_LABEL[p.classification]}</Badge>
                      {p.provisioned && <Badge tone="violet">provisioned</Badge>}
                    </div>
                    <div className="mt-0.5 truncate text-xs text-slate-500">
                      {p.publisher || "Unknown publisher"}
                      {p.version ? ` · v${p.version}` : ""}
                      {p.architecture ? ` · ${p.architecture}` : ""}
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </Card>

      <div className="flex items-center justify-end gap-3">
        <span className="text-sm text-slate-500">{selected.size} selected</span>
        <button
          onClick={onRemove}
          disabled={removing || selected.size === 0}
          className="flex items-center gap-2 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
        >
          <Trash2 className="h-4 w-4" />
          {removing ? "Removing…" : `Remove ${selected.size} selected`}
        </button>
      </div>
    </div>
  );
}
