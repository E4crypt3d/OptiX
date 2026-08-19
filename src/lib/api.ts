import { invoke } from "@tauri-apps/api/core";
import type {
  AppxPackage,
  BenchmarkResult,
  BloatwareRemoveResult,
  ChangeRecord,
  CleanupCategory,
  CleanupResult,
  CrashReport,
  DetectedGame,
  Diagnostic,
  Game,
  GameProfile,
  GameProfileApplyResult,
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
  AmdShaderCache,
  CacheClearResult,
  DnsApplyResult,
  DnsBenchmarkResult,
  DnsServer,
  GamingToggle,
  GpuAdapter,
  GpuToggleResult,
  NetworkStatus,
  ServiceActionResult,
  ServiceInfo,
  ShaderCache,
  Snapshot,
  StartupActionResult,
  StartupEntry,
  SystemStats,
  TcpParameter,
  WSearchStatus,
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

export function listServices(): Promise<ServiceInfo[]> {
  return invoke<ServiceInfo[]>("list_services");
}

export function stopService(name: string): Promise<ServiceActionResult> {
  return invoke<ServiceActionResult>("stop_service", { name });
}

export function startService(name: string): Promise<ServiceActionResult> {
  return invoke<ServiceActionResult>("start_service", { name });
}

export function setServiceStartType(
  name: string,
  startType: string,
): Promise<ServiceActionResult> {
  return invoke<ServiceActionResult>("set_service_start_type", { name, startType });
}

export function getWsearch(): Promise<WSearchStatus> {
  return invoke<WSearchStatus>("get_wsearch");
}

export function setWsearch(enabled: boolean): Promise<ServiceActionResult> {
  return invoke<ServiceActionResult>("set_wsearch", { enabled });
}

export function listStartup(): Promise<StartupEntry[]> {
  return invoke<StartupEntry[]>("list_startup");
}

export function setStartupEnabled(
  location: string,
  enabled: boolean,
  command: string,
): Promise<StartupActionResult> {
  return invoke<StartupActionResult>("set_startup_enabled", { location, enabled, command });
}

export function networkStatus(): Promise<NetworkStatus> {
  return invoke<NetworkStatus>("network_status");
}

export function listDnsServers(): Promise<DnsServer[]> {
  return invoke<DnsServer[]>("list_dns_servers");
}

export function benchmarkDns(
  domains: string[],
  queriesPerDomain: number,
): Promise<DnsBenchmarkResult[]> {
  return invoke<DnsBenchmarkResult[]>("benchmark_dns", { domains, queriesPerDomain });
}

export function applyDns(guid: string, servers: string[]): Promise<DnsApplyResult> {
  return invoke<DnsApplyResult>("apply_dns", { guid, servers });
}

export function tcpParameters(): Promise<TcpParameter[]> {
  return invoke<TcpParameter[]>("tcp_parameters");
}

export function listGpuAdapters(): Promise<GpuAdapter[]> {
  return invoke<GpuAdapter[]>("list_gpu_adapters");
}

export function listGpuToggles(): Promise<GamingToggle[]> {
  return invoke<GamingToggle[]>("list_gpu_toggles");
}

export function setGpuToggle(id: string, enabled: boolean): Promise<GpuToggleResult> {
  return invoke<GpuToggleResult>("set_gpu_toggle", { id, enabled });
}

export function scanShaderCaches(): Promise<ShaderCache[]> {
  return invoke<ShaderCache[]>("scan_shader_caches");
}

export function clearShaderCaches(ids: string[]): Promise<CacheClearResult> {
  return invoke<CacheClearResult>("clear_shader_caches", { ids });
}

export function getAmdShaderCache(): Promise<AmdShaderCache> {
  return invoke<AmdShaderCache>("get_amd_shader_cache");
}

export function setAmdShaderCache(alwaysOn: boolean): Promise<AmdShaderCache> {
  return invoke<AmdShaderCache>("set_amd_shader_cache", { alwaysOn });
}

export function detectGames(): Promise<DetectedGame[]> {
  return invoke<DetectedGame[]>("detect_games");
}

export function listGames(): Promise<Game[]> {
  return invoke<Game[]>("list_games");
}

export function addGame(
  launcher: string,
  appId: string | null,
  name: string,
  installPath: string,
  executable: string,
): Promise<Game> {
  return invoke<Game>("add_game", { launcher, appId, name, installPath, executable });
}

export function addManualGame(name: string, executable: string): Promise<Game> {
  return invoke<Game>("add_manual_game", { name, executable });
}

export function removeGame(id: number): Promise<void> {
  return invoke<void>("remove_game", { id });
}

export function getGameProfile(gameId: number): Promise<GameProfile> {
  return invoke<GameProfile>("get_game_profile", { gameId });
}

export function saveGameProfile(profile: GameProfile): Promise<void> {
  return invoke<void>("save_game_profile", { profile });
}

export function applyGameProfile(gameId: number): Promise<GameProfileApplyResult> {
  return invoke<GameProfileApplyResult>("apply_game_profile", { gameId });
}

export function restoreGameProfile(gameId: number): Promise<number> {
  return invoke<number>("restore_game_profile", { gameId });
}

export function runFpsBenchmark(
  gameId: number | null,
  gameName: string | null,
  exeName: string,
  durationSecs: number,
): Promise<BenchmarkResult> {
  return invoke<BenchmarkResult>("run_fps_benchmark", {
    gameId,
    gameName,
    exeName,
    durationSecs,
  });
}

export function runStressBenchmark(durationSecs: number): Promise<BenchmarkResult> {
  return invoke<BenchmarkResult>("run_stress_benchmark", { durationSecs });
}

export function listBenchmarks(): Promise<BenchmarkResult[]> {
  return invoke<BenchmarkResult[]>("list_benchmarks");
}

export function deleteBenchmark(id: number): Promise<void> {
  return invoke<void>("delete_benchmark", { id });
}

export function benchmarkFrameTimes(id: number): Promise<number[]> {
  return invoke<number[]>("benchmark_frame_times", { id });
}

export function scanCrashes(): Promise<CrashReport[]> {
  return invoke<CrashReport[]>("scan_crashes");
}

export function generateCrashReport(crash: CrashReport): Promise<string> {
  return invoke<string>("generate_crash_report", { crash });
}

export function runDiagnostics(): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("run_diagnostics");
}

export function scanBloatware(): Promise<AppxPackage[]> {
  return invoke<AppxPackage[]>("scan_bloatware");
}

export function removeBloatware(fullNames: string[]): Promise<BloatwareRemoveResult> {
  return invoke<BloatwareRemoveResult>("remove_bloatware", { fullNames });
}
