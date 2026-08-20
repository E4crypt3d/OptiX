import { Component, lazy, Suspense, useState, type ComponentType, type LazyExoticComponent, type ReactNode } from "react";
import { Activity, RefreshCw } from "lucide-react";
import { Sidebar, type ViewId } from "./components/Sidebar";
import { logEvent } from "./lib/api";
import { errMsg } from "./lib/errors";

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
          {/* Keyed per view: switching tabs resets a crashed view instead of
              trapping the user on a dead screen. */}
          <ErrorBoundary key={view} onHome={() => setView("dashboard")}>
            <Suspense fallback={<ViewLoading />}>
              <View />
            </Suspense>
          </ErrorBoundary>
        </div>
      </main>
    </div>
  );
}

interface ErrorBoundaryProps {
  children: ReactNode;
  onHome: () => void;
}

interface ErrorBoundaryState {
  error: string | null;
}

/**
 * Catches render/lifecycle errors in a view so one bad component can't blank
 * the entire app. The error is logged to the console (and, via the window
 * handler, to logs.txt) and the user gets a Retry / Go to Dashboard choice.
 */
class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(e: unknown): ErrorBoundaryState {
    return { error: errMsg(e) };
  }

  componentDidCatch(error: unknown, info: unknown) {
    console.error("view crashed:", error, info);
    void logEvent("error", `view crashed: ${errMsg(error)}`).catch(() => {});
  }

  render() {
    if (this.state.error === null) return this.props.children;
    return (
      <div
        role="alert"
        className="rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-6 text-center"
      >
        <h2 className="text-sm font-semibold text-rose-200">
          This view hit an unexpected error
        </h2>
        <p className="mx-auto mt-1 max-w-md break-words text-xs text-rose-300/80">
          {this.state.error}
        </p>
        <div className="mt-4 flex items-center justify-center gap-2">
          <button
            onClick={() => this.setState({ error: null })}
            className="flex items-center gap-2 rounded-lg bg-rose-500/20 px-4 py-2 text-sm font-medium text-rose-100 transition-colors hover:bg-rose-500/30"
          >
            <RefreshCw className="h-4 w-4" />
            Retry
          </button>
          <button
            onClick={() => {
              this.props.onHome();
              this.setState({ error: null });
            }}
            className="rounded-lg bg-slate-800 px-4 py-2 text-sm font-medium text-slate-200 transition-colors hover:bg-slate-700"
          >
            Go to Dashboard
          </button>
        </div>
      </div>
    );
  }
}

function ViewLoading() {
  return (
    <div className="flex items-center gap-2 py-6 text-sm text-slate-500">
      <Activity className="h-4 w-4 animate-pulse" />
      Loading…
    </div>
  );
}