export type ProviderCategory = "apiKey" | "openAI";
export type AuthMode = "authJsonFile" | "apiKey";
export type TemplateKind = "openAi" | "apiKey";
export type SwitchExecutionStatus = "success" | "failure";
export type SwitchStepStatus = "success" | "skipped" | "failure";
export type ConnectionTestStatus = "success" | "warning" | "failure";

export interface UiPreferences {
  sidebarCollapsed: boolean;
}

export interface CodexProfile {
  id: string;
  name: string;
  baseUrl: string;
  testModel?: string | null;
  autoDisabled: boolean;
  autoDisabledReason?: string | null;
  autoDisabledAtUtc?: string | null;
  providerCategory: ProviderCategory;
  authMode: AuthMode;
  hasStoredAuthJson: boolean;
  hasStoredConfigToml: boolean;
  hasStoredApiKey: boolean;
  createdAtUtc: string;
  updatedAtUtc: string;
}

export interface CachedWslEnvironmentInfo {
  distroName: string;
  userName: string;
}

export interface SettingsSnapshot {
  replaceWindowsTarget: boolean;
  replaceWslTarget: boolean;
  wslDistroName?: string | null;
  wslUserName?: string | null;
  cachedDefaultWsl?: CachedWslEnvironmentInfo | null;
  cachedDefaultWslErrorMessage?: string | null;
  sessionMigrationDays: number;
  apiKeyProviderName: string;
  uiPreferences: UiPreferences;
  migrationVersion: number;
}

export interface BackupOverview {
  targetKey: string;
  displayName: string;
  hasRecentBackup: boolean;
  count: number;
}

export interface SwitchExecutionStep {
  name: string;
  detail: string;
  status: SwitchStepStatus;
}

export interface SwitchExecutionSummary {
  executedAtUtc: string;
  profileId: string;
  profileName: string;
  targets: string[];
  message: string;
  status: SwitchExecutionStatus;
  steps: SwitchExecutionStep[];
}

export interface DashboardSnapshot {
  activeProfileId?: string | null;
  activeProfileName?: string | null;
  selectedTargetLabels: string[];
  wslStatus: string;
  backupOverview: BackupOverview[];
  lastSwitchSummary?: SwitchExecutionSummary | null;
}

export interface TemplateSnapshot {
  openAi: string;
  apiKey: string;
}

export interface BackupItem {
  targetKey: string;
  displayName: string;
  directoryName: string;
  createdAtLabel: string;
  hasAuthJson: boolean;
  hasConfigToml: boolean;
}

export interface AppMeta {
  version: string;
}

export interface AppBootstrap {
  profiles: CodexProfile[];
  settings: SettingsSnapshot;
  dashboard: DashboardSnapshot;
  templates: TemplateSnapshot;
  backups: BackupItem[];
  appMeta: AppMeta;
}

export interface SaveProfileInput {
  id?: string;
  name: string;
  providerCategory: ProviderCategory;
  importConfigToml: boolean;
  baseUrl?: string | null;
  configTomlSourcePath?: string | null;
  authMode: AuthMode;
  authJsonSourcePath?: string | null;
  apiKey?: string | null;
  testModel?: string | null;
}

export interface SaveSettingsInput {
  replaceWindowsTarget: boolean;
  replaceWslTarget: boolean;
  wslDistroName?: string | null;
  wslUserName?: string | null;
  sessionMigrationDays: number;
  apiKeyProviderName: string;
  uiPreferences: UiPreferences;
}

export interface PickFileInput {
  title?: string | null;
  filterName: string;
  extensions: string[];
}

export interface SwitchTargetPreview {
  targetKey: string;
  displayName: string;
}

export interface SwitchPreview {
  profileId: string;
  profileName: string;
  providerName: string;
  targets: SwitchTargetPreview[];
  warnings: string[];
  steps: string[];
}

export interface SaveTemplateInput {
  kind: TemplateKind;
  content: string;
}

export interface ConnectionTestResult {
  status: ConnectionTestStatus;
  endpoint: string;
  model: string;
  message: string;
}

export interface BatchConnectionTestInput {
  profileIds: string[];
  overrideModel?: string | null;
  disableOnFailure: boolean;
}

export interface BatchConnectionTestItem {
  profileId: string;
  profileName: string;
  status: ConnectionTestStatus;
  endpoint: string;
  model: string;
  message: string;
  autoDisabled: boolean;
}

export interface BatchConnectionTestResponse {
  results: BatchConnectionTestItem[];
  bootstrap: AppBootstrap;
}

export interface LogEntry {
  timestamp: string;
  level: string;
  action: string;
  message: string;
  context: Record<string, unknown>;
}
