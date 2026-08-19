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
  Rocket,
  ScanSearch,
  Settings,
  Trash2,
  Undo2,
  Zap,
} from "lucide-react";

export type ViewId =
  | "dashboard"
  | "scanner"
  | "snapshots"
  | "rollback"
  | "cleanup"
  | "processes"
  | "power"
  | "startup"
  | "network"
  | "gpu"
  | "games"
  | "benchmarks"
  | "crash"
  | "settings";

const NAV: { id: ViewId; label: string; icon: typeof LayoutDashboard }[] = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "scanner", label: "System Scanner", icon: ScanSearch },
  { id: "snapshots", label: "Snapshots", icon: Archive },
  { id: "rollback", label: "Rollback Center", icon: Undo2 },
  { id: "cleanup", label: "Cleanup", icon: Trash2 },
  { id: "processes", label: "Processes & RAM", icon: Cpu },
  { id: "power", label: "Power", icon: BatteryCharging },
  { id: "startup", label: "Startup & Services", icon: Rocket },
  { id: "network", label: "Network", icon: Network },
  { id: "gpu", label: "GPU", icon: Monitor },
  { id: "games", label: "Game Profiles", icon: Gamepad2 },
  { id: "benchmarks", label: "Benchmarks", icon: Gauge },
  { id: "crash", label: "Crash Reports", icon: AlertTriangle },
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
        <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-gradient-to-br from-cyan-400 to-violet-500">
          <Zap className="h-5 w-5 text-white" strokeWidth={2.5} />
        </div>
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
        {NAV.map(({ id, label, icon: Icon }) => {
          const isActive = id === active;
          return (
            <button
              key={id}
              onClick={() => onNavigate(id)}
              className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
                isActive
                  ? "bg-slate-800/80 text-slate-100"
                  : "text-slate-400 hover:bg-slate-900 hover:text-slate-200"
              }`}
            >
              <Icon className="h-4 w-4" />
              {label}
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
