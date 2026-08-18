import type { AppBootstrap } from "@/types/domain";

export function createBootstrapFixture(): AppBootstrap {
  return {
    profiles: [
      {
        id: "profile-1",
        name: "生产 OpenAI",
        baseUrl: "https://api.openai.com/v1",
        testModel: "gpt-5.4-mini",
        autoDisabled: false,
        autoDisabledReason: null,
        autoDisabledAtUtc: null,
        providerCategory: "openAI",
        authMode: "authJsonFile",
        hasStoredAuthJson: true,
        hasStoredConfigToml: false,
        hasStoredApiKey: false,
        createdAtUtc: "2026-03-27T08:00:00Z",
        updatedAtUtc: "2026-03-27T09:00:00Z",
      },
      {
        id: "profile-2",
        name: "测试 APIKEY",
        baseUrl: "https://example.com/v1",
        testModel: "gpt-4.1-mini",
        autoDisabled: false,
        autoDisabledReason: null,
        autoDisabledAtUtc: null,
        providerCategory: "apiKey",
        authMode: "apiKey",
        hasStoredAuthJson: false,
        hasStoredConfigToml: true,
        hasStoredApiKey: true,
        createdAtUtc: "2026-03-26T08:00:00Z",
        updatedAtUtc: "2026-03-26T09:00:00Z",
      },
    ],
    settings: {
      replaceWindowsTarget: true,
      replaceWslTarget: true,
      wslDistroName: "Ubuntu-24.04",
      wslUserName: "workspace",
      cachedDefaultWsl: {
        distroName: "Ubuntu-24.04",
        userName: "workspace",
      },
      cachedDefaultWslErrorMessage: null,
      sessionMigrationDays: 7,
      apiKeyProviderName: "openai",
      uiPreferences: {
        sidebarCollapsed: false,
      },
      migrationVersion: 1,
    },
    dashboard: {
      activeProfileId: "profile-2",
      activeProfileName: "测试 APIKEY",
      selectedTargetLabels: ["Windows", "WSL"],
      wslStatus: "已检测到 Ubuntu-24.04 / coder",
      backupOverview: [
        {
          targetKey: "windows",
          displayName: "Windows",
          hasRecentBackup: true,
          count: 2,
        },
      ],
      lastSwitchSummary: null,
    },
    templates: {
      openAi: "openai template",
      apiKey: "apikey template",
    },
    backups: [],
    appMeta: {
      version: "0.1.0",
    },
  };
}
