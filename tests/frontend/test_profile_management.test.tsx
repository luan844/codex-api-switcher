import React from "react";
import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { ProfileManagement } from "@/features/profiles/profile-management";
import { useAppShellStore } from "@/store/app-shell-store";
import { createBootstrapFixture } from "./test_fixtures";

describe("ProfileManagement", () => {
  beforeEach(() => {
    useAppShellStore.setState({
      bootstrap: null,
      bootstrapError: null,
      isBootstrapping: false,
      selectedProfileId: null,
      route: "/profiles",
    });
  });

  it("筛选工具条会在缩放或中等宽度下自动换行，避免排序控件溢出", () => {
    const bootstrap = createBootstrapFixture();
    useAppShellStore.getState().setBootstrap(bootstrap);

    render(<ProfileManagement />);

    const toolbar = screen.getByTestId("profile-filters-toolbar");
    const sortTrigger = screen.getAllByRole("combobox")[1];
    const sortWrapper = screen.getByTestId("profile-sort-filter");

    expect(toolbar.className).toContain("flex-wrap");
    expect(sortWrapper.className).toContain("basis-full");
    expect(sortWrapper.className).toContain("xl:basis-[10rem]");
    expect(sortTrigger.className).toContain("min-w-0");
  });

  it("配置列表项会约束长名称和长地址，避免标签把内容顶出", () => {
    const bootstrap = createBootstrapFixture();
    bootstrap.profiles[1] = {
      ...bootstrap.profiles[1],
      name: "一个非常长的 APIKEY 配置名称用于验证配置列表项头部收缩行为",
      baseUrl:
        "https://very-long-hostname-for-profiles-page.example.com/v1/chat/completions/with/a/really/long/path",
      autoDisabled: true,
    };
    bootstrap.dashboard.activeProfileId = bootstrap.profiles[1].id;
    bootstrap.dashboard.activeProfileName = bootstrap.profiles[1].name;

    useAppShellStore.getState().setBootstrap(bootstrap);

    render(<ProfileManagement />);

    const profileButton = screen.getByRole("button", {
      name: /一个非常长的 APIKEY 配置名称用于验证配置列表项头部收缩行为/i,
    });
    const title = within(profileButton).getByText(
      "一个非常长的 APIKEY 配置名称用于验证配置列表项头部收缩行为",
    );
    const url = within(profileButton).getByText(/very-long-hostname-for-profiles-page/i);

    expect(profileButton.className).toContain("min-w-0");
    expect(title.className).toContain("truncate");
    expect(url.className).toContain("break-all");
  });
});