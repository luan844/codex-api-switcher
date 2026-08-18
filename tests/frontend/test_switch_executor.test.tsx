import React, { act } from "react";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { SwitchExecutor } from "@/features/switch/switch-executor";
import { useAppShellStore } from "@/store/app-shell-store";
import type { AppBootstrap, SwitchPreview } from "@/types/domain";
import { createBootstrapFixture } from "./test_fixtures";

const getSwitchPreview = vi.fn<(profileId: string) => Promise<SwitchPreview>>();
const executeSwitch = vi.fn<(profileId: string) => Promise<AppBootstrap>>();

vi.mock("@/lib/tauri-api/app", () => ({
  appApi: {
    getSwitchPreview: (profileId: string) => getSwitchPreview(profileId),
    executeSwitch: (profileId: string) => executeSwitch(profileId),
  },
}));

function createPreview(profileId: string, providerName: string, targetName: string): SwitchPreview {
  return {
    profileId,
    profileName: `${profileId}-name`,
    providerName,
    targets: [
      {
        targetKey: `${profileId}-target`,
        displayName: targetName,
      },
    ],
    warnings: [],
    steps: ["validate"],
  };
}

describe("SwitchExecutor", () => {
  beforeEach(() => {
    getSwitchPreview.mockReset();
    executeSwitch.mockReset();

    const bootstrap = createBootstrapFixture();
    bootstrap.profiles[0].id = "profile-1";
    bootstrap.profiles[1].id = "profile-2";

    useAppShellStore.setState({
      bootstrap,
      bootstrapError: null,
      isBootstrapping: false,
      selectedProfileId: "profile-1",
      route: "/switch",
    });
  });

  it("仅展示最新一次预检查结果，并在加载期间禁用执行按钮", async () => {
    let resolveFirst: ((value: SwitchPreview) => void) | null = null;
    let resolveSecond: ((value: SwitchPreview) => void) | null = null;
    getSwitchPreview
      .mockReturnValueOnce(
        new Promise<SwitchPreview>((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockReturnValueOnce(
        new Promise<SwitchPreview>((resolve) => {
          resolveSecond = resolve;
        }),
      );

    render(<SwitchExecutor />);

    expect(screen.getByRole("button", { name: /执行切换/i })).toHaveProperty("disabled", true);

    act(() => {
      useAppShellStore.getState().setSelectedProfileId("profile-2");
    });

    resolveSecond?.(createPreview("profile-2", "provider-2", "新目标"));
    expect(await screen.findByText("provider: provider-2")).toBeTruthy();
    expect(screen.getByText("新目标")).toBeTruthy();

    resolveFirst?.(createPreview("profile-1", "provider-1", "旧目标"));

    await act(async () => {
      await Promise.resolve();
    });

    expect(screen.queryByText("provider: provider-1")).toBeNull();
    expect(screen.queryByText("旧目标")).toBeNull();
    expect(screen.getByRole("button", { name: /执行切换/i })).toHaveProperty("disabled", false);
  });
});