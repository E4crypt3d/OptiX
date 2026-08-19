// Types mirroring the Rust models in `src-tauri/src/models/hardware.rs`
// (serde `rename_all = "camelCase"`).

export interface CpuInfo {
  name: string;
  brand: string;
  vendor: string;
  physicalCores: number;
  logicalCores: number;
  frequencyMhz: number;
  usagePercent: number;
}

export interface GpuInfo {
  name: string;
  vendor: string;
  driverVersion: string;
  memoryBytes: number;
  usagePercent: number;
}

export interface MemoryInfo {
  totalBytes: number;
  usedBytes: number;
  availableBytes: number;
  usagePercent: number;
}

export interface DiskInfo {
  name: string;
  mountPoint: string;
  fileSystem: string;
  totalBytes: number;
  availableBytes: number;
  usedBytes: number;
  kind: string;
  isRemovable: boolean;
}

export interface PhysicalDiskInfo {
  friendlyName: string;
  mediaType: string;
  healthStatus: string;
  busType: string;
  firmwareVersion: string | null;
  sizeBytes: number;
}

export interface NetworkInterface {
  name: string;
  receivedBytes: number;
  transmittedBytes: number;
  totalReceivedBytes: number;
  totalTransmittedBytes: number;
}

export interface DisplayInfo {
  width: number;
  height: number;
  refreshRate: number;
}

export interface TemperatureInfo {
  label: string;
  celsius: number | null;
}

export interface OsInfo {
  name: string;
  version: string;
  kernelVersion: string;
  hostName: string;
  uptimeSeconds: number;
  buildNumber: number | null;
  isWindows11: boolean;
  edition: string | null;
}

export interface MotherboardInfo {
  manufacturer: string;
  product: string;
}

export interface BiosInfo {
  vendor: string;
  version: string;
}

export interface ProcessInfo {
  pid: number;
  name: string;
  exe: string;
  cpuUsagePercent: number;
  memoryBytes: number;
  diskReadBytes: number;
  diskWrittenBytes: number;
  startTime: number;
}

export interface StartupApp {
  name: string;
  command: string;
  location: string;
}

export interface HardwareInfo {
  cpu: CpuInfo;
  gpus: GpuInfo[];
  memory: MemoryInfo;
  disks: DiskInfo[];
  physicalDisks: PhysicalDiskInfo[];
  network: NetworkInterface[];
  displays: DisplayInfo[];
  temperatures: TemperatureInfo[];
  os: OsInfo;
  motherboard: MotherboardInfo | null;
  bios: BiosInfo | null;
  processes: ProcessInfo[];
  startupApps: StartupApp[];
  scannedAtMs: number;
}

export interface SystemStats {
  cpuUsagePercent: number;
  perCoreUsage: number[];
  memory: MemoryInfo;
  network: NetworkInterface[];
  timestampMs: number;
}

export interface HardwareSample {
  id: number | null;
  tsMs: number;
  cpuUsage: number | null;
  cpuTemp: number | null;
  ramUsedMb: number | null;
  ramTotalMb: number | null;
  gpuUsage: number | null;
  gpuTemp: number | null;
  gpuVramMb: number | null;
  gpuPowerW: number | null;
  diskUsedMb: number | null;
  diskTotalMb: number | null;
  netDownBps: number | null;
  netUpBps: number | null;
  fps: number | null;
  frameTimeMs: number | null;
}

export type SnapshotStatus = "active" | "restored" | "deleted";

export interface Snapshot {
  id: string;
  name: string;
  reason: string | null;
  createdAtMs: number;
  restoredAtMs: number | null;
  status: SnapshotStatus;
}

export interface ChangeRecord {
  id: number | null;
  snapshotId: string;
  domain: string;
  location: string;
  kind: string;
  oldValue: string | null;
  newValue: string | null;
  oldJson: unknown | null;
  newJson: unknown | null;
  appliedAtMs: number | null;
  verified: boolean;
  rolledBack: boolean;
}

export interface CleanupCategory {
  id: string;
  name: string;
  description: string;
  safety: "safe" | "caution";
  sizeBytes: number;
  fileCount: number;
  expectedRebuild: boolean;
}

export interface CategoryResult {
  id: string;
  beforeBytes: number;
  freedBytes: number;
  filesRemoved: number;
  filesSkipped: number;
}

export interface CleanupResult {
  snapshotId: string;
  freedBytes: number;
  filesRemoved: number;
  filesSkipped: number;
  categories: CategoryResult[];
}

export type ProcessClass = "required" | "safe" | "unknown";
export type PriorityClass =
  | "idle"
  | "below_normal"
  | "normal"
  | "above_normal"
  | "high"
  | "realtime";

export interface ProcessDetail {
  pid: number;
  name: string;
  exe: string;
  cpuUsagePercent: number;
  memoryBytes: number;
  diskReadBytes: number;
  diskWrittenBytes: number;
  startTime: number;
  parentPid: number | null;
  status: string;
  classification: ProcessClass;
  isSystem: boolean;
  priority: PriorityClass | null;
}

export interface PriorityChange {
  pid: number;
  name: string;
  from: string;
  to: string;
}

export interface GamingModeResult {
  boosted: PriorityChange[];
  lowered: PriorityChange[];
}

export interface PowerScheme {
  guid: string;
  name: string;
  isActive: boolean;
}

export interface PowerProfile {
  id: string;
  name: string;
  description: string;
  baseGuid: string;
  note: string;
}

export interface PowerApplyResult {
  snapshotId: string;
  schemeGuid: string;
  schemeName: string;
  changeCount: number;
}

export interface NicAdapter {
  key: string;
  name: string;
  eee: number | null;
  greenEthernet: number | null;
  pnpCapabilities: number | null;
  powerManagement: number | null;
}

export interface NicPowerResult {
  snapshotId: string;
  adaptersChanged: number;
  changes: number;
}
