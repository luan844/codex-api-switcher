import React from "react";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { AppHeader } from "@/components/layout/app-header";
import { createBootstrapFixture } from "./test_fixtures";

describe("AppHeader", () => {
  it("顶部包含操作按钮与基础文本", () => {
    const bootstrap = createBootstrapFixture();

    render(
      <AppHeader
        bootstrap={bootstrap}
        onOpenDataDirectory={vi.fn()}
        onRouteChange={vi.fn()}
        route="/"
      />,
    );

    const dataRootButton = screen.getByRole("button", { name: /数据目录/i });
    const actionArea = dataRootButton.parentElement;

    expect(actionArea?.className).toContain("flex");
  });
});
