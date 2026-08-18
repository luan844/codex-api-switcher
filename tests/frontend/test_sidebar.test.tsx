import React from "react";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Sidebar } from "@/components/layout/sidebar";

describe("Sidebar", () => {
  it("折叠态仍保留移动端全宽布局，只在大屏隐藏文字内容", () => {
    const { container } = render(
      <Sidebar
        collapsed
        onRouteChange={vi.fn()}
        onToggle={vi.fn()}
        route="/"
      />,
    );

    const aside = container.querySelector("aside");
    const workbenchLabel = screen.getByText("工作台");

    expect(aside?.className).toContain("w-full");
    expect(aside?.className).toContain("lg:w-[92px]");
    expect(workbenchLabel.className).toContain("lg:hidden");
  });
});