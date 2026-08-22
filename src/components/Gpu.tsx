import { useCallback, useEffect, useState } from "react";
import { Monitor, RefreshCw, Thermometer, Activity, Trash2 } from "lucide-react";
import {
  clearShaderCaches,
  getAmdShaderCache,
  listGpuAdapters,
  listGpuToggles,
  scanShaderCaches,
  setAmdShaderCache,
  setGpuToggle,
} from "../lib/api";
import type {
  AmdShaderCache,
  GamingToggle,
  GpuAdapter,
  ShaderCache,
} from "../lib/types";
import { formatBytes } from "../lib/format";
import { errMsg } from "../lib/errors";
import { Badge, Card } from "./ui";

function riskTone(risk: string): "emerald" | "amber" | "rose" {
  if (risk === "high") return "rose";
  if (risk === "medium") return "amber";
  return "emerald";
}

export function Gpu() {
  const isWindows =
    typeof navigator !== "undefined" && /windows|win32/i.test(navigator.userAgent);
  const [adapters, setAdapters] = useState<GpuAdapter[]>([]);
  const [toggles, setToggles] = useState<GamingToggle[]>([]);
  const [caches, setCaches] = useState<ShaderCache[]>([]);
  const [amd, setAmd] = useState<AmdShaderCache | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [a, t, c, amdMode] = await Promise.all([
        listGpuAdapters(),
        listGpuToggles(),
        scanShaderCaches(),
        getAmdShaderCache(),
      ]);
      setAdapters(a);
      setToggles(t);
      setCaches(c);
      setAmd(amdMode);
      setSelected(new Set(c.map((x) => x.id)));
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function onToggle(t: GamingToggle) {
    const next = !t.enabled;
    const extra =
      t.id === "memory_integrity"
        ? "\n\nMemory Integrity is a security feature. Disabling it reduces protection but is reversible and requires a restart."
        : "";
    if (!window.confirm(`${next ? "Enable" : "Disable"} "${t.name}"?${extra}`)) return;
    setBusy(t.id);
    setError(null);
    setNotice(null);
    try {
      await setGpuToggle(t.id, next);
      setNotice(`Updated ${t.name}. Reversible via Rollback Center.`);
      await load();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  async function onClearCaches() {
    const ids = caches.filter((c) => selected.has(c.id)).map((c) => c.id);
    if (ids.length === 0) return;
    const total = caches
      .filter((c) => selected.has(c.id))
      .reduce((a, c) => a + c.sizeBytes, 0);
    if (
      !window.confirm(
        `Clear ${formatBytes(total)} of shader caches? Games will briefly stutter while shaders rebuild.`,
      )
    ) {
      return;
    }
    setBusy("caches");
    setError(null);
    setNotice(null);
    try {
      const r = await clearShaderCaches(ids);
      setNotice(`Freed ${formatBytes(r.freedBytes)}. Rebuilt on next launch.`);
      await load();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  async function onAmdMode(alwaysOn: boolean) {
    const label = alwaysOn ? "Always On" : "Optimized";
    if (
      !window.confirm(
        `Set AMD shader cache to ${label}? A snapshot is created first and the change is reversible.`,
      )
    ) {
      return;
    }
    setBusy("amd");
    setError(null);
    setNotice(null);
    try {
      await setAmdShaderCache(alwaysOn);
      setNotice(alwaysOn ? "AMD shader cache set to Always On." : "AMD shader cache set to Optimized.");
      await load();
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(null);
    }
  }

  const totalSelected = caches
    .filter((c) => selected.has(c.id))
    .reduce((a, c) => a + c.sizeBytes, 0);

  return (
    <div className="space-y-4">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold text-slate-100">GPU</h1>
          <p className="text-sm text-slate-500">
            Driver toggles and shader caches. Every change is snapshot-first and reversible.
          </p>
        </div>
        <button
          onClick={load}
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

      <Card title={`Adapters (${adapters.length})`}>
        {adapters.length === 0 ? (
          <p className="text-sm text-slate-500">No display adapters detected.</p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {adapters.map((a) => (
              <li key={a.name} className="flex items-center gap-3 py-2.5">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-slate-800">
                  <Monitor className="h-4 w-4 text-cyan-400" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="font-medium text-slate-200">{a.name}</div>
                  <div className="text-xs text-slate-500">
                    {a.vendor}
                    {a.driverVersion ? ` · driver ${a.driverVersion}` : ""}
                  </div>
                </div>
                <div className="flex shrink-0 gap-4 text-right">
                  {a.usagePercent != null && (
                    <div>
                      <div className="flex items-center gap-1 tabular-nums text-slate-200">
                        <Activity className="h-3 w-3 text-cyan-400" />
                        {Math.round(a.usagePercent)}%
                      </div>
                      <div className="text-xs text-slate-500">Usage</div>
                    </div>
                  )}
                  {a.temperatureCelsius != null && (
                    <div>
                      <div className="flex items-center gap-1 tabular-nums text-slate-200">
                        <Thermometer className="h-3 w-3 text-amber-400" />
                        {Math.round(a.temperatureCelsius)}°C
                      </div>
                      <div className="text-xs text-slate-500">Temp</div>
                    </div>
                  )}
                  <div>
                    <div className="tabular-nums text-slate-200">
                      {a.memoryUsedBytes != null ? (
                        <>
                          {formatBytes(a.memoryUsedBytes)} / {formatBytes(a.memoryBytes)}
                        </>
                      ) : (
                        formatBytes(a.memoryBytes)
                      )}
                    </div>
                    <div className="text-xs text-slate-500">VRAM</div>
                  </div>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <Card title="Gaming Toggles">
        {!isWindows ? (
          <p className="text-sm text-slate-500">
            Gaming toggles are Windows registry settings; they're unavailable on
            this platform.
          </p>
        ) : (
        <ul className="divide-y divide-slate-800/60">
          {toggles.map((t) => (
            <li key={t.id} className="flex items-center gap-4 py-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-slate-200">{t.name}</span>
                  {t.enabled ? (
                    <Badge tone="emerald">on</Badge>
                  ) : (
                    <Badge tone="slate">{t.known ? "off" : "default"}</Badge>
                  )}
                  <Badge tone={riskTone(t.risk)}>{t.risk} risk</Badge>
                  {t.requiresRestart && <Badge tone="violet">restart</Badge>}
                </div>
                <div className="mt-0.5 text-xs text-slate-500">{t.description}</div>
                <div className="text-xs text-slate-600">{t.impactNote}</div>
              </div>
              <button
                onClick={() => onToggle(t)}
                disabled={busy === t.id}
                className={`rounded-lg px-3 py-1.5 text-xs font-medium text-white transition-colors disabled:opacity-50 ${
                  t.enabled ? "bg-rose-600 hover:bg-rose-500" : "bg-cyan-600 hover:bg-cyan-500"
                }`}
              >
                {busy === t.id ? "…" : t.enabled ? "Disable" : "Enable"}
              </button>
            </li>
          ))}
        </ul>
        )}
      </Card>

      {amd && amd.adapter && (
        <Card title="AMD Shader Cache">
          <div className="flex items-center gap-4">
            <div className="min-w-0 flex-1">
              <div className="font-medium text-slate-200">{amd.adapter}</div>
              <div className="text-xs text-slate-500">
                Current mode: <span className="text-slate-300">{amd.mode}</span>. "Always On" pins
                the shader cache on and can reduce mid-game recompilation stutter.
              </div>
            </div>
            <button
              onClick={() => onAmdMode(amd.mode !== "always_on")}
              disabled={busy === "amd"}
              className={`rounded-lg px-3 py-1.5 text-xs font-medium text-white transition-colors disabled:opacity-50 ${
                amd.mode === "always_on"
                  ? "bg-rose-600 hover:bg-rose-500"
                  : "bg-cyan-600 hover:bg-cyan-500"
              }`}
            >
              {busy === "amd"
                ? "…"
                : amd.mode === "always_on"
                  ? "Use Optimized"
                  : "Use Always On"}
            </button>
          </div>
        </Card>
      )}

      <Card
        title={`Shader Caches${isWindows ? ` (${formatBytes(totalSelected)} selected)` : ""}`}
        action={
          isWindows && (
            <button
              onClick={onClearCaches}
              disabled={busy === "caches" || selected.size === 0}
              className="flex items-center gap-1.5 rounded-lg bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-cyan-500 disabled:opacity-50"
            >
              <Trash2 className="h-3.5 w-3.5" />
              {busy === "caches" ? "Clearing…" : "Clear selected"}
            </button>
          )
        }
      >
        {!isWindows ? (
          <p className="text-sm text-slate-500">
            Shader cache directories are Windows-specific; nothing is listed on
            this platform.
          </p>
        ) : caches.length === 0 ? (
          <p className="text-sm text-slate-500">No shader cache directories found.</p>
        ) : (
          <ul className="divide-y divide-slate-800/60">
            {caches.map((c) => (
              <li key={c.id} className="flex items-center gap-3 py-2.5">
                <input
                  type="checkbox"
                  checked={selected.has(c.id)}
                  onChange={() =>
                    setSelected((prev) => {
                      const next = new Set(prev);
                      if (next.has(c.id)) next.delete(c.id);
                      else next.add(c.id);
                      return next;
                    })
                  }
                  className="h-4 w-4 shrink-0 accent-cyan-500"
                />
                <div className="min-w-0 flex-1">
                  <div className="font-medium text-slate-200">{c.name}</div>
                  <div className="truncate font-mono text-[11px] text-slate-600">{c.path}</div>
                </div>
                <div className="shrink-0 text-right">
                  <div className="tabular-nums text-slate-200">{formatBytes(c.sizeBytes)}</div>
                  <div className="text-xs tabular-nums text-slate-500">{c.fileCount} files</div>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
