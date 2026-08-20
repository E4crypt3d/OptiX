import { lazy, Suspense, useState, type ComponentType, type LazyExoticComponent } from "react";
import { Activity } from "lucide-react";
import { Sidebar, type ViewId } from "./components/Sidebar";

/**
 * Each view is loaded on demand so the heavy dependencies it pulls in
 * (recharts on Dashboard/Benchmark, etc.) aren't parsed on startup — they're
 * fetched only when the user opens that page, and cached afterwards.
 */
const viewModules: Record<ViewId, LazyExoticComponent<ComponentType>> = {
  dashboard: lazy(() => import("./components/Dashboard").then((m) => ({ default: m.Dashboard }))),
  scanner: lazy(() => import("./components/Scanner").then((m) => ({ default: m.Scanner }))),
  snapshots: lazy(() => import("./components/Snapshots").then((m) => ({ default: m.Snapshots }))),
  rollback: lazy(() => import("./components/Rollback").then((m) => ({ default: m.Rollback }))),
  cleanup: lazy(() => import("./components/Cleanup").then((m) => ({ default: m.Cleanup }))),
  bloatware: lazy(() => import("./components/Bloatware").then((m) => ({ default: m.Bloatware }))),
  processes: lazy(() => import("./components/Processes").then((m) => ({ default: m.Processes }))),
  power: lazy(() => import("./components/Power").then((m) => ({ default: m.Power }))),
  startup: lazy(() => import("./components/StartupServices").then((m) => ({ default: m.StartupServices }))),
  network: lazy(() => import("./components/Network").then((m) => ({ default: m.Network }))),
  gpu: lazy(() => import("./components/Gpu").then((m) => ({ default: m.Gpu }))),
  games: lazy(() => import("./components/Games").then((m) => ({ default: m.Games }))),
  benchmarks: lazy(() => import("./components/Benchmark").then((m) => ({ default: m.Benchmark }))),
  crash: lazy(() => import("./components/CrashReports").then((m) => ({ default: m.CrashReports }))),
  diagnostics: lazy(() => import("./components/Diagnostics").then((m) => ({ default: m.Diagnostics }))),
  settings: lazy(() => import("./components/Settings").then((m) => ({ default: m.Settings }))),
};

export default function App() {
  const [view, setView] = useState<ViewId>("dashboard");
  const View = viewModules[view];

  return (
    <div className="flex h-screen w-full overflow-hidden bg-[#0a0e17] text-slate-100">
      <Sidebar active={view} onNavigate={setView} />
      <main className="flex-1 overflow-y-auto px-6 py-6">
        <div className="mx-auto max-w-6xl">
          <Suspense fallback={<ViewLoading />}>
            <View />
          </Suspense>
        </div>
      </main>
    </div>
  );
}

function ViewLoading() {
  return (
    <div className="flex items-center gap-2 py-6 text-sm text-slate-500">
      <Activity className="h-4 w-4 animate-pulse" />
      Loading…
    </div>
  );
}