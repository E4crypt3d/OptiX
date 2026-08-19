import { invoke } from "@tauri-apps/api/core";
import type {
  ChangeRecord,
  CleanupCategory,
  CleanupResult,
  GamingModeResult,
  HardwareInfo,
  HardwareSample,
  NicAdapter,
  NicPowerResult,
  PowerApplyResult,
  PowerProfile,
  PowerScheme,
  PriorityClass,
  ProcessDetail,
  Snapshot,
  SystemStats,
} from "./types";

export function scanSystem(): Promise<HardwareInfo> {
  return invoke<HardwareInfo>("scan_system");
}

export function systemStats(): Promise<SystemStats> {
  return invoke<SystemStats>("system_stats");
}

export function recordSample(): Promise<void> {
  return invoke<void>("record_sample");
}

export function recentSamples(): Promise<HardwareSample[]> {
  return invoke<HardwareSample[]>("recent_samples");
}

export function createSnapshot(name: string, reason: string | null): Promise<Snapshot> {
  return invoke<Snapshot>("create_snapshot", { name, reason });
}

export function listSnapshots(): Promise<Snapshot[]> {
  return invoke<Snapshot[]>("list_snapshots");
}

export function listChanges(snapshotId: string): Promise<ChangeRecord[]> {
  return invoke<ChangeRecord[]>("list_changes", { snapshotId });
}

export function deleteSnapshot(id: string): Promise<void> {
  return invoke<void>("delete_snapshot", { id });
}

export function restoreSnapshot(id: string): Promise<number> {
  return invoke<number>("restore_snapshot", { id });
}

export function diffSnapshots(a: string, b: string): Promise<unknown> {
  return invoke<unknown>("diff_snapshots", { a, b });
}

export function scanCleanup(): Promise<CleanupCategory[]> {
  return invoke<CleanupCategory[]>("scan_cleanup");
}

export function runCleanup(ids: string[]): Promise<CleanupResult> {
  return invoke<CleanupResult>("run_cleanup", { ids });
}

export function listProcesses(): Promise<ProcessDetail[]> {
  return invoke<ProcessDetail[]>("list_processes");
}

export function killProcess(pid: number): Promise<void> {
  return invoke<void>("kill_process", { pid });
}

export function setProcessPriority(pid: number, priority: PriorityClass): Promise<void> {
  return invoke<void>("set_process_priority", { pid, priority });
}

export function applyGamingMode(
  gamePids: number[],
  backgroundPids: number[],
): Promise<GamingModeResult> {
  return invoke<GamingModeResult>("apply_gaming_mode", {
    gamePids,
    backgroundPids,
  });
}

export function restoreGamingMode(): Promise<number> {
  return invoke<number>("restore_gaming_mode");
}

export function listPowerSchemes(): Promise<PowerScheme[]> {
  return invoke<PowerScheme[]>("list_power_schemes");
}

export function listPowerProfiles(): Promise<PowerProfile[]> {
  return invoke<PowerProfile[]>("list_power_profiles");
}

export function applyPowerProfile(id: string): Promise<PowerApplyResult> {
  return invoke<PowerApplyResult>("apply_power_profile", { id });
}

export function listNicAdapters(): Promise<NicAdapter[]> {
  return invoke<NicAdapter[]>("list_nic_adapters");
}

export function disableNicPowerSaving(): Promise<NicPowerResult> {
  return invoke<NicPowerResult>("disable_nic_power_saving");
}
