import { useState } from "react";
import { Benchmark } from "./components/Benchmark";
import { Bloatware } from "./components/Bloatware";
import { Cleanup } from "./components/Cleanup";
import { CrashReports } from "./components/CrashReports";
import { Diagnostics } from "./components/Diagnostics";
import { Dashboard } from "./components/Dashboard";
import { Games } from "./components/Games";
import { Gpu } from "./components/Gpu";
import { Network } from "./components/Network";
import { Power } from "./components/Power";
import { Processes } from "./components/Processes";
import { Rollback } from "./components/Rollback";
import { Scanner } from "./components/Scanner";
import { Settings } from "./components/Settings";
import { StartupServices } from "./components/StartupServices";
import { Sidebar, type ViewId } from "./components/Sidebar";
import { Snapshots } from "./components/Snapshots";

export default function App() {
  const [view, setView] = useState<ViewId>("dashboard");

  return (
    <div className="flex h-screen w-full overflow-hidden bg-[#0a0e17] text-slate-100">
      <Sidebar active={view} onNavigate={setView} />
      <main className="flex-1 overflow-y-auto px-6 py-6">
        <div className="mx-auto max-w-6xl">
          {view === "dashboard" && <Dashboard />}
          {view === "scanner" && <Scanner />}
          {view === "snapshots" && <Snapshots />}
          {view === "rollback" && <Rollback />}
          {view === "cleanup" && <Cleanup />}
          {view === "bloatware" && <Bloatware />}
          {view === "processes" && <Processes />}
          {view === "power" && <Power />}
          {view === "startup" && <StartupServices />}
          {view === "network" && <Network />}
          {view === "gpu" && <Gpu />}
          {view === "games" && <Games />}
          {view === "benchmarks" && <Benchmark />}
          {view === "crash" && <CrashReports />}
          {view === "diagnostics" && <Diagnostics />}
          {view === "settings" && <Settings />}
        </div>
      </main>
    </div>
  );
}
