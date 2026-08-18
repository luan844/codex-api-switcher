import type {
  AppBootstrap,
  BackupItem,
  BatchConnectionTestInput,
  BatchConnectionTestResponse,
  ConnectionTestResult,
  LogEntry,
  PickFileInput,
  SaveProfileInput,
  SaveSettingsInput,
  SaveTemplateInput,
  SwitchPreview,
  TemplateKind,
} from "@/types/domain";
import { invokeCommand } from "@/lib/tauri-api/client";

export const appApi = {
  loadBootstrap: () => invokeCommand<AppBootstrap>("load_app_bootstrap"),
  saveProfile: (payload: SaveProfileInput) =>
    invokeCommand<AppBootstrap>("save_profile", { payload }),
  deleteProfile: (profileId: string) =>
    invokeCommand<AppBootstrap>("delete_profile", { profileId }),
  duplicateProfile: (profileId: string) =>
    invokeCommand<AppBootstrap>("duplicate_profile", { profileId }),
  updateSettings: (payload: SaveSettingsInput) =>
    invokeCommand<AppBootstrap>("update_settings", { payload }),
  refreshDefaultWsl: () => invokeCommand<AppBootstrap>("refresh_default_wsl"),
  getSwitchPreview: (profileId: string) =>
    invokeCommand<SwitchPreview>("get_switch_preview", { profileId }),
  executeSwitch: (profileId: string) =>
    invokeCommand<AppBootstrap>("execute_switch", { profileId }),
  listBackups: () => invokeCommand<BackupItem[]>("list_backups"),
  restoreLatestBackup: () => invokeCommand<AppBootstrap>("restore_latest_backup"),
  saveTemplate: (payload: SaveTemplateInput) =>
    invokeCommand<AppBootstrap>("save_template", { payload }),
  resetTemplate: (kind: TemplateKind) =>
    invokeCommand<AppBootstrap>("reset_template", { kind }),
  testProfileConnection: (profileId: string) =>
    invokeCommand<ConnectionTestResult>("test_profile_connection", { profileId }),
  batchTestProfileConnections: (payload: BatchConnectionTestInput) =>
    invokeCommand<BatchConnectionTestResponse>("batch_test_profile_connections", { payload }),
  setProfileTestDisabled: (profileId: string, disabled: boolean) =>
    invokeCommand<AppBootstrap>("set_profile_test_disabled", { disabled, profileId }),
  readRecentLogs: (limit = 200) =>
    invokeCommand<LogEntry[]>("read_recent_logs", { limit }),
  openDataDirectory: () => invokeCommand<void>("open_data_directory"),
  openBackupsDirectory: () => invokeCommand<void>("open_backups_directory"),
  openLogsDirectory: () => invokeCommand<void>("open_logs_directory"),
  openBackupDirectory: (targetKey: string, directoryName: string) =>
    invokeCommand<void>("open_backup_directory", { directoryName, targetKey }),
  pickFilePath: (payload: PickFileInput) =>
    invokeCommand<string | null>("pick_file_path", { payload }),
};
