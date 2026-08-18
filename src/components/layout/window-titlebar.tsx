import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { Maximize2, Minus, PanelTop, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import type { AppBootstrap } from "@/types/domain";
import type { AppRoute } from "@/store/app-shell-store";
import { routeMeta } from "@/components/layout/navigation";
import { AppBrandMark } from "@/components/branding/app-brand-mark";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

interface WindowTitlebarProps {
  bootstrap: AppBootstrap | null;
  route: AppRoute;
}

function canUseDesktopWindow() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function WindowTitlebar({ bootstrap, route }: WindowTitlebarProps) {
  const [isMaximized, setIsMaximized] = useState(false);
  const currentRoute = routeMeta[route];
  const currentWindow = useMemo(() => (canUseDesktopWindow() ? getCurrentWindow() : null), []);
  const activeProfileLabel = bootstrap?.dashboard.activeProfileName ?? "未执行切换";
  const targetLabel = bootstrap?.dashboard.selectedTargetLabels.join("、") || "未选择目标";
  const lastStatus = bootstrap?.dashboard.lastSwitchSummary?.status;

  const refreshMaximized = useCallback(async () => {
    if (!currentWindow) {
      setIsMaximized(false);
      return;
    }

    try {
      setIsMaximized(await currentWindow.isMaximized());
    } catch {
      setIsMaximized(false);
    }
  }, [currentWindow]);

  useEffect(() => {
    void refreshMaximized();
  }, [refreshMaximized]);

  async function runWindowAction(action: (windowHandle: ReturnType<typeof getCurrentWindow>) => Promise<unknown>) {
    if (!currentWindow) {
      return;
    }

    try {
      await action(currentWindow);
      await refreshMaximized();
    } catch {
      // 浏览器预览或权限不足时保持静默，避免影响前端测试与静态构建。
    }
  }

  return (
    <header className="flex h-[var(--titlebar-height)] items-stretch border-b border-slate-200 bg-white/80 text-slate-800 backdrop-blur-xl">
      <div className="app-region-drag flex min-w-0 flex-1 items-center gap-3 px-3">
        <div className="flex min-w-0 items-center gap-3">
          <div className="grid h-8 w-8 place-items-center rounded-md border border-slate-200 bg-slate-50 text-primary">
            <AppBrandMark className="h-5 w-5" />
          </div>
          <div className="min-w-0 leading-none">
            <p className="truncate text-sm font-semibold tracking-wide text-slate-900">
              Codex API Switcher
            </p>
            <p className="truncate pt-1 text-[11px] text-slate-500">
              Provider Control Center
            </p>
          </div>
        </div>

        <div
          className="app-region-drag flex min-w-0 flex-1 items-center gap-2 overflow-hidden"
          onDoubleClick={() => {
            void runWindowAction((windowHandle) => windowHandle.toggleMaximize());
          }}
        >
          <Badge variant="blue" className="ml-4">{currentRoute.shortLabel}</Badge>
          {bootstrap ? <Badge variant="slate" className="bg-slate-100 text-slate-600 hover:bg-slate-200 border-transparent">V{bootstrap.appMeta.version}</Badge> : null}
          {lastStatus ? (
            <Badge variant={lastStatus === "success" ? "green" : "red"}>
              {lastStatus === "success" ? "最近执行成功" : "最近执行失败"}
            </Badge>
          ) : null}
          <div className="hidden min-w-0 items-center gap-2 overflow-hidden xl:flex ml-2">
            <PanelTop className="h-3.5 w-3.5 shrink-0 text-slate-400" />
            <span className="truncate text-xs font-medium text-slate-600" title={activeProfileLabel}>
              {activeProfileLabel}
            </span>
            <span className="shrink-0 text-slate-300">/</span>
            <span className="truncate text-xs text-slate-500" title={targetLabel}>
              {targetLabel}
            </span>
          </div>
        </div>
      </div>

      <div className="app-region-no-drag flex items-center gap-1 border-l border-slate-200 px-2">
        <WindowControlButton
          ariaLabel="最小化"
          onClick={() => {
            void runWindowAction((windowHandle) => windowHandle.minimize());
          }}
        >
          <Minus className="h-4 w-4" />
        </WindowControlButton>
        <WindowControlButton
          ariaLabel={isMaximized ? "还原" : "最大化"}
          onClick={() => {
            void runWindowAction((windowHandle) => windowHandle.toggleMaximize());
          }}
        >
          {isMaximized ? <Square className="h-3 w-3" /> : <Maximize2 className="h-3 w-3" />}
        </WindowControlButton>
        <WindowControlButton
          ariaLabel="关闭"
          className="hover:bg-rose-500 hover:text-white"
          onClick={() => {
            void runWindowAction((windowHandle) => windowHandle.close());
          }}
        >
          <X className="h-4 w-4" />
        </WindowControlButton>
      </div>
    </header>
  );
}

function WindowControlButton({
  ariaLabel,
  children,
  className,
  onClick,
}: {
  ariaLabel: string;
  children: ReactNode;
  className?: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={ariaLabel}
      className={cn(
        "inline-flex h-8 w-8 items-center justify-center rounded-md text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-900",
        className,
      )}
      onClick={onClick}
      type="button"
    >
      {children}
    </button>
  );
}
