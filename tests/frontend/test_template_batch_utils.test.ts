import { describe, expect, it } from "vitest";

import {
  filterProfilesByStatus,
  resolveBatchSelection,
  summarizeBatchResults,
} from "@/features/templates/batch-test-utils";
import { createBootstrapFixture } from "./test_fixtures";

describe("模板批量测试辅助函数", () => {
  it("会按启用和禁用状态筛选组合", () => {
    const bootstrap = createBootstrapFixture();
    const profiles = bootstrap.profiles.map((profile, index) => ({
      ...profile,
      autoDisabled: index === 1,
    }));

    expect(filterProfilesByStatus(profiles, "enabled").map((item) => item.id)).toEqual([
      "profile-1",
    ]);
    expect(filterProfilesByStatus(profiles, "disabled").map((item) => item.id)).toEqual([
      "profile-2",
    ]);
  });

  it("默认选择会保留现有启用项或回退到当前激活配置", () => {
    const bootstrap = createBootstrapFixture();
    const profiles = [
      {
        ...bootstrap.profiles[0],
        id: "profile-1",
        providerCategory: "apiKey" as const,
      },
      {
        ...bootstrap.profiles[1],
        id: "profile-2",
        autoDisabled: true,
      },
      {
        ...bootstrap.profiles[1],
        id: "profile-3",
        autoDisabled: false,
      },
    ];

    expect(resolveBatchSelection(profiles, ["profile-2", "profile-3"], "profile-1")).toEqual([
      "profile-3",
    ]);
    expect(resolveBatchSelection(profiles, [], "profile-1")).toEqual(["profile-1"]);
  });

  it("会汇总批量测试结果的状态数量", () => {
    const summary = summarizeBatchResults([
      {
        autoDisabled: false,
        endpoint: "https://example.com/a",
        message: "ok",
        model: "gpt-5.4-mini",
        profileId: "profile-1",
        profileName: "组合 A",
        status: "success",
      },
      {
        autoDisabled: true,
        endpoint: "https://example.com/b",
        message: "denied",
        model: "gpt-5.4-mini",
        profileId: "profile-2",
        profileName: "组合 B",
        status: "failure",
      },
      {
        autoDisabled: false,
        endpoint: "https://example.com/c",
        message: "warning",
        model: "gpt-5.4-mini",
        profileId: "profile-3",
        profileName: "组合 C",
        status: "warning",
      },
    ]);

    expect(summary).toEqual({
      autoDisabled: 1,
      failure: 1,
      success: 1,
      warning: 1,
    });
  });
});