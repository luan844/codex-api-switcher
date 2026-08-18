import React from "react";
import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { WorkbenchView } from "@/features/dashboard/workbench-view";
import { useAppShellStore } from "@/store/app-shell-store";
import { createBootstrapFixture } from "./test_fixtures";

describe("WorkbenchView", () => {
  beforeEach(() => {
    useAppShellStore.setState({
      bootstrap: null,
      bootstrapError: null,
      isBootstrapping: false,
      selectedProfileId: null,
      route: "/",
    });
  });

  it("大屏布局会为快速切换和最近执行保留各自滚动区", () => {
    const bootstrap = createBootstrapFixture();
    bootstrap.profiles = [
      ...bootstrap.profiles,
      {
        ...bootstrap.profiles[0],
        id: "profile-3",
        name: "备用线路 A",
        baseUrl: "https://backup-a.example.com/v1",
      },
      {
        ...bootstrap.profiles[1],
        id: "profile-4",
        name: "备用线路 B",
        baseUrl: "https://backup-b.example.com/v1",
      },
    ];
    bootstrap.dashboard.lastSwitchSummary = {
      executedAtUtc: "2026-03-27T10:00:00Z",
      profileId: "profile-2",
      profileName: "测试 APIKEY",
      targets: ["Windows", "WSL"],
      message: "Windows 与 WSL 目标均已完成切换。",
      status: "success",
      steps: Array.from({ length: 6 }, (_, index) => ({
        name: `step-${index + 1}`,
        detail: `第 ${index + 1} 步的校验和写入日志详情。`,
        status: "success" as const,
      })),
    };

    useAppShellStore.getState().setBootstrap(bootstrap);

    render(<WorkbenchView />);

    const layout = screen.getByTestId("workbench-layout");
    const quickSwitchContent = screen.getByTestId("workbench-quick-switch-content");
    const lastSwitchContent = screen.getByTestId("workbench-last-switch-content");

    expect(layout.className).toContain("xl:overflow-hidden");
    expect(quickSwitchContent.className).toContain("overflow-y-auto");
    expect(quickSwitchContent.className).toContain("xl:[scrollbar-gutter:stable]");
    expect(lastSwitchContent.className).toContain("overflow-y-auto");
    expect(lastSwitchContent.className).toContain("xl:[scrollbar-gutter:stable]");
  });

  it("最近执行的长文本会启用换行样式避免撑破卡片", () => {
    const bootstrap = createBootstrapFixture();
    bootstrap.dashboard.lastSwitchSummary = {
      executedAtUtc: "2026-03-27T10:00:00Z",
      profileId: "profile-2",
      profileName: "测试 APIKEY",
      targets: ["Windows"],
      message:
        "managed/backups/windows/run-20260327-194758-388/very/long/path/result.json 已写入完成",
      status: "success",
      steps: [
        {
          name: "backup",
          detail:
            "managed/backups/windows/run-20260327-194758-388/very/long/path/result.json",
          status: "success",
        },
      ],
    };

    useAppShellStore.getState().setBootstrap(bootstrap);

    const { container } = render(<WorkbenchView />);
    const summaries = screen.getAllByText(/已写入完成/);
    const detail = screen.getByText(/result\.json$/);

    expect(summaries.some((element) => element.className.includes("break-all"))).toBe(true);
    expect(detail.className).toContain("break-all");
    expect(container.textContent).toContain("最近一次执行");
  });

  it("快速切换卡片会约束长名称和 URL，避免把 APIKEY 标签顶出容器", () => {
    const bootstrap = createBootstrapFixture();
    bootstrap.profiles[1] = {
      ...bootstrap.profiles[1],
      name: "一个非常长的 APIKEY 配置名称用于验证卡片头部收缩行为",
      baseUrl:
        "https://very-long-hostname-for-apikey-profile.example.com/v1/chat/completions/with/a/really/long/path",
    };

    useAppShellStore.getState().setBootstrap(bootstrap);

    render(<WorkbenchView />);

    const profileButton = screen.getByRole("button", {
      name: /一个非常长的 APIKEY 配置名称用于验证卡片头部收缩行为/i,
    });
    const title = screen.getByText("一个非常长的 APIKEY 配置名称用于验证卡片头部收缩行为");
    const apiKeyBadge = within(profileButton).getByText("APIKEY");
    const url = screen.getByText(/very-long-hostname-for-apikey-profile/i);

    expect(profileButton.className).toContain("min-w-0");
    expect(title.className).toContain("truncate");
    expect(apiKeyBadge.className).toContain("shrink-0");
    expect(url.className).toContain("break-all");
  });
});
