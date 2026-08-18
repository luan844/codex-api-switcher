import { invoke } from "@tauri-apps/api/core";

import type {
  ConnectionTestResult,
  ModelDiscoveryResult,
  SaveProviderInput,
  SwitchPreview,
  SwitchResult,
  SwitcherBootstrap,
  SwitcherLog,
  SwitcherSettings,
} from "@/switcher/types";

export const switcherApi = {
  load: () => invoke<SwitcherBootstrap>("load_switcher_bootstrap"),
  saveProvider: (payload: SaveProviderInput) =>
    invoke<SwitcherBootstrap>("save_switcher_provider", { payload }),
  duplicateProvider: (profileId: string) =>
    invoke<SwitcherBootstrap>("duplicate_switcher_provider", { profileId }),
  deleteProvider: (profileId: string) =>
    invoke<SwitcherBootstrap>("delete_switcher_provider", { profileId }),
  saveSettings: (payload: SwitcherSettings) =>
    invoke<SwitcherBootstrap>("save_switcher_settings", { payload }),
  previewSwitch: (profileId: string) =>
    invoke<SwitchPreview>("preview_switcher", { profileId }),
  discoverModels: (profileId: string) =>
    invoke<ModelDiscoveryResult>("discover_switcher_models", { profileId }),
  testProvider: (profileId: string) =>
    invoke<ConnectionTestResult>("test_switcher_provider", { profileId }),
  executeSwitch: (profileId: string, forceClose = false, restartCodex = true) =>
    invoke<SwitchResult>("execute_switcher", {
      payload: { profileId, forceClose, restartCodex },
    }),
  restoreOfficial: (forceClose = false, restartCodex = true) =>
    invoke<SwitchResult>("restore_switcher_official", {
      forceClose,
      restartCodex,
    }),
  restoreBackup: (backupId: string, force = false) =>
    invoke<SwitcherBootstrap>("restore_switcher_backup", {
      payload: { backupId, force },
    }),
  readLogs: (limit = 200) =>
    invoke<SwitcherLog[]>("read_switcher_logs", { limit }),
  openDataDirectory: () => invoke<void>("open_switcher_data_directory"),
  openBackupDirectory: () => invoke<void>("open_switcher_backup_directory"),
  openLogDirectory: () => invoke<void>("open_switcher_log_directory"),
};
