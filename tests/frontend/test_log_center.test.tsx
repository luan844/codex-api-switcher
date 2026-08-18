import React from "react";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { LogCenter } from "@/features/logs/log-center";
import { useAppShellStore } from "@/store/app-shell-store";
import type { LogEntry } from "@/types/domain";
import { createBootstrapFixture } from "./test_fixtures";

const readRecentLogs = vi.fn<(limit?: number) => Promise<LogEntry[]>>();

vi.mock("@/lib/tauri-api/app", () => ({
  appApi: {
    readRecentLogs: (limit?: number) => readRecentLogs(limit),
  },
}));

describe("LogCenter", () => {
  beforeEach(() => {
    readRecentLogs.mockReset();
    useAppShellStore.setState({
      bootstrap: createBootstrapFixture(),
      bootstrapError: null,
      isBootstrapping: false,
      selectedProfileId: "profile-2",
      route: "/logs",
    });
  });

  it("加载日志期间会禁用刷新按钮，并在完成后渲染结构化上下文", async () => {
    let resolveLogs: ((logs: LogEntry[]) => void) | null = null;
    readRecentLogs.mockReturnValueOnce(
      new Promise<LogEntry[]>((resolve) => {
        resolveLogs = resolve;
      }),
    );

    render(<LogCenter />);

    const refreshButton = screen.getByRole("button", { name: /刷新中/i });
    expect((refreshButton as HTMLButtonElement).disabled).toBe(true);

    resolveLogs?.([
      {
        timestamp: "2026-03-28T10:00:00Z",
        level: "info",
        action: "switch.execute",
        message: "切换完成",
        context: {
          provider: "openai",
          targets: ["Windows"],
        },
      },
    ]);

    expect(await screen.findByText("切换完成")).toBeTruthy();
    expect(screen.getByText(/"provider": "openai"/)).toBeTruthy();
    expect((screen.getByRole("button", { name: /刷新日志/i }) as HTMLButtonElement).disabled).toBe(
      false,
    );
  });
});