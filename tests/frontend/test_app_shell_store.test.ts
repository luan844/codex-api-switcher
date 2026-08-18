import { beforeEach, describe, expect, it } from "vitest";

import { useAppShellStore } from "@/store/app-shell-store";
import { createBootstrapFixture } from "./test_fixtures";

describe("app-shell-store", () => {
  beforeEach(() => {
    useAppShellStore.setState({
      bootstrap: null,
      bootstrapError: null,
      isBootstrapping: true,
      selectedProfileId: null,
      route: "/",
    });
  });

  it("加载 bootstrap 时优先选择工作台激活配置", () => {
    const bootstrap = createBootstrapFixture();

    useAppShellStore.getState().setBootstrap(bootstrap);

    const state = useAppShellStore.getState();
    expect(state.bootstrap).toEqual(bootstrap);
    expect(state.selectedProfileId).toBe("profile-2");
  });

  it("没有激活配置时回退到首个配置", () => {
    const bootstrap = createBootstrapFixture();
    bootstrap.dashboard.activeProfileId = null;

    useAppShellStore.getState().setBootstrap(bootstrap);

    expect(useAppShellStore.getState().selectedProfileId).toBe("profile-1");
  });

  it("更新 bootstrap 时优先保留当前选中的配置", () => {
    const bootstrap = createBootstrapFixture();
    useAppShellStore.setState({
      bootstrap,
      bootstrapError: null,
      isBootstrapping: false,
      selectedProfileId: "profile-1",
      route: "/",
    });

    useAppShellStore.getState().setBootstrap(createBootstrapFixture());

    expect(useAppShellStore.getState().selectedProfileId).toBe("profile-1");
  });

  it("可以单独记录启动错误信息", () => {
    useAppShellStore.getState().setBootstrapError("启动失败");

    expect(useAppShellStore.getState().bootstrapError).toBe("启动失败");
  });
});
