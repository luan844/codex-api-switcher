import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeCommand } = vi.hoisted(() => ({
  invokeCommand: vi.fn(),
}));

vi.mock("@/lib/tauri-api/client", () => ({
  invokeCommand,
}));

import { appApi } from "@/lib/tauri-api/app";

describe("appApi", () => {
  beforeEach(() => {
    invokeCommand.mockReset();
    invokeCommand.mockResolvedValue(undefined);
  });

  it("切换命令会透传 profileId", async () => {
    await appApi.executeSwitch("profile-1");

    expect(invokeCommand).toHaveBeenCalledWith("execute_switch", {
      profileId: "profile-1",
    });
  });

  it("模板保存命令会透传 kind 与 content", async () => {
    await appApi.saveTemplate({
      kind: "apiKey",
      content: "model_provider = \"openai\"",
    });

    expect(invokeCommand).toHaveBeenCalledWith("save_template", {
      payload: {
        kind: "apiKey",
        content: "model_provider = \"openai\"",
      },
    });
  });

  it("批量测试命令会透传 profileIds、模型覆盖和自动禁用开关", async () => {
    await appApi.batchTestProfileConnections({
      profileIds: ["profile-1", "profile-2"],
      overrideModel: "gpt-5.4",
      disableOnFailure: true,
    });

    expect(invokeCommand).toHaveBeenCalledWith("batch_test_profile_connections", {
      payload: {
        profileIds: ["profile-1", "profile-2"],
        overrideModel: "gpt-5.4",
        disableOnFailure: true,
      },
    });
  });

  it("恢复启用命令会透传 disabled 状态", async () => {
    await appApi.setProfileTestDisabled("profile-2", false);

    expect(invokeCommand).toHaveBeenCalledWith("set_profile_test_disabled", {
      disabled: false,
      profileId: "profile-2",
    });
  });

  it("打开受管备份目录命令只透传目标标识和目录名", async () => {
    await appApi.openBackupDirectory("windows", "run-20260327-090000");

    expect(invokeCommand).toHaveBeenCalledWith("open_backup_directory", {
      directoryName: "run-20260327-090000",
      targetKey: "windows",
    });
  });
});
