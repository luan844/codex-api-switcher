export interface ModelEntry {
  id: string;
  displayName: string;
  contextWindow?: number | null;
}
export interface ProviderProfile {
  id: string;
  name: string;
  providerId: string;
  baseUrl: string;
  hasApiKey: boolean;
  defaultModel: string;
  models: ModelEntry[];
  timeoutSeconds: number;
  injectModels: boolean;
  createdAtUtc: string;
  updatedAtUtc: string;
}

export interface SwitcherSettings {
  codexHomeOverride?: string | null;
  sessionMigrationDays: number;
  backupRetention: number;
  injectModelsDefault: boolean;
}

export interface BackupSummary {
  id: string;
  createdAtUtc: string;
  kind: string;
  profileName?: string | null;
  status: string;
  fileCount: number;
}

export interface CodexRuntimeStatus {
  installed: boolean;
  running: boolean;
  version?: string | null;
  executablePath?: string | null;
  codexHome: string;
  activeProviderId?: string | null;
}

export interface SwitcherBootstrap {
  profiles: ProviderProfile[];
  settings: SwitcherSettings;
  activeProfileId?: string | null;
  backups: BackupSummary[];
  runtime: CodexRuntimeStatus;
  appVersion: string;
}

export interface SaveProviderInput {
  id?: string | null;
  name: string;
  providerId: string;
  baseUrl: string;
  apiKey?: string | null;
  clearApiKey: boolean;
  defaultModel: string;
  models: ModelEntry[];
  timeoutSeconds: number;
  injectModels: boolean;
}

export interface ModelDiscoveryResult {
  endpoint: string;
  models: ModelEntry[];
  message: string;
}

export interface ConnectionTestResult {
  endpoint: string;
  model: string;
  latencyMs: number;
  status: string;
  message: string;
}

export interface SwitchPreview {
  profileId: string;
  profileName: string;
  providerId: string;
  codexHome: string;
  filesToBackup: string[];
  sessionMigrationDays: number;
  codexRunning: boolean;
  willRestartCodex: boolean;
  willInjectModels: boolean;
  warnings: string[];
}

export interface SwitchStep {
  phase: string;
  status: string;
  message: string;
}

export interface SwitchResult {
  success: boolean;
  profileId?: string | null;
  profileName: string;
  backupId?: string | null;
  restartedCodex: boolean;
  injectedModels: boolean;
  warnings: string[];
  steps: SwitchStep[];
  bootstrap: SwitcherBootstrap;
}

export interface SwitcherLog {
  timestamp: string;
  level: string;
  action: string;
  message: string;
  context: Record<string, unknown>;
}

export interface ApiFailure {
  code?: string;
  message?: string;
  detail?: string | null;
}
