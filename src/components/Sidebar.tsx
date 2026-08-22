import {
  AlertTriangle,
  Archive,
  BatteryCharging,
  Cpu,
  Gauge,
  Gamepad2,
  LayoutDashboard,
  Monitor,
  Network,
  PackageX,
  Rocket,
  ScanSearch,
  Settings,
  Sparkles,
  Trash2,
  Undo2,
} from "lucide-react";

export type ViewId =
  | "dashboard"
  | "scanner"
  | "snapshots"
  | "rollback"
  | "cleanup"
  | "bloatware"
  | "processes"
  | "power"
  | "startup"
  | "network"
  | "gpu"
  | "games"
  | "benchmarks"
  | "crash"
  | "diagnostics"
  | "settings";

export const NAV: { id: ViewId; label: string; icon: typeof LayoutDashboard }[] = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "scanner", label: "System Scanner", icon: ScanSearch },
  { id: "snapshots", label: "Snapshots", icon: Archive },
  { id: "rollback", label: "Rollback Center", icon: Undo2 },
  { id: "cleanup", label: "Cleanup", icon: Trash2 },
  { id: "bloatware", label: "Bloatware", icon: PackageX },
  { id: "processes", label: "Processes & RAM", icon: Cpu },
  { id: "power", label: "Power", icon: BatteryCharging },
  { id: "startup", label: "Startup & Services", icon: Rocket },
  { id: "network", label: "Network", icon: Network },
  { id: "gpu", label: "GPU", icon: Monitor },
  { id: "games", label: "Game Profiles", icon: Gamepad2 },
  { id: "benchmarks", label: "Benchmarks", icon: Gauge },
  { id: "crash", label: "Crash Reports", icon: AlertTriangle },
  { id: "diagnostics", label: "Diagnostics", icon: Sparkles },
  { id: "settings", label: "Settings", icon: Settings },
];

export function Sidebar({
  active,
  onNavigate,
}: {
  active: ViewId;
  onNavigate: (view: ViewId) => void;
}) {
  return (
    <aside className="flex h-full w-60 shrink-0 flex-col border-r border-slate-800 bg-slate-950/60">
      <div className="flex items-center gap-2 px-5 py-5">
        <img src="/logo-no-bg.png" alt="Optix" className="h-11 w-11 shrink-0" />
        <div>
          <div className="text-lg font-bold tracking-tight text-slate-100">
            Optix
          </div>
          <div className="text-[11px] leading-tight text-slate-500">
            Performance &amp; Recovery
          </div>
        </div>
      </div>

      <nav className="flex-1 space-y-1 px-3 py-2">
        {NAV.map(({ id, label, icon: Icon }, index) => {
          const isActive = id === active;
          // First nine views are reachable with Ctrl/Cmd+1..9; show the hint
          // on hover and expose it to assistive tech via aria-keyshortcuts.
          const shortcut = index < 9 ? `Control+${index + 1}` : undefined;
          return (
            <button
              key={id}
              onClick={() => onNavigate(id)}
              aria-current={isActive ? "page" : undefined}
              aria-keyshortcuts={shortcut}
              title={shortcut ? `${label} (${shortcut.replace("Control", "Ctrl")})` : undefined}
              className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
                isActive
                  ? "bg-slate-800/80 text-slate-100"
                  : "text-slate-400 hover:bg-slate-900 hover:text-slate-200"
              }`}
            >
              <Icon className="h-4 w-4" />
              <span className="flex-1 truncate text-left">{label}</span>
              {shortcut && (
                <kbd className="text-[10px] font-normal text-slate-600">
                  {shortcut.replace("Control", "Ctrl")}
                </kbd>
              )}
            </button>
          );
        })}
      </nav>

      <div className="px-5 py-4 text-[11px] text-slate-600">
        Every change is tracked.
        <br />
        Every change can be undone.
      </div>
    </aside>
  );
}
