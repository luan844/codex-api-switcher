import { describe, expect, it } from "vitest";

import { profileFormSchema } from "@/features/profiles/schema";
import { settingsFormSchema } from "@/features/settings/schema";
import { formatDateTime, sortByUpdatedAt } from "@/lib/utils";

describe("表单校验与工具函数", () => {
  it("配置表单会拦截空名称", () => {
    const result = profileFormSchema.safeParse({
      providerCategory: "apiKey",
      importConfigToml: false,
      authMode: "apiKey",
      name: "",
    });

    expect(result.success).toBe(false);
  });

  it("系统设置会限制 provider name 和迁移天数", () => {
    const result = settingsFormSchema.safeParse({
      replaceWindowsTarget: true,
      replaceWslTarget: true,
      wslDistroName: "Ubuntu-24.04",
      wslUserName: "coder",
      sessionMigrationDays: 45,
      apiKeyProviderName: "bad value",
      sidebarCollapsed: false,
    });

    expect(result.success).toBe(false);
  });

  it("按更新时间倒序排序并格式化时间", () => {
    const sorted = sortByUpdatedAt([
      { updatedAtUtc: "2026-03-26T10:00:00Z", value: "older" },
      { updatedAtUtc: "2026-03-27T10:00:00Z", value: "newer" },
    ]);

    expect(sorted.map((item) => item.value)).toEqual(["newer", "older"]);
    expect(formatDateTime("2026-03-27T10:00:00Z")).toContain("2026");
  });
});
