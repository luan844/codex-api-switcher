import { Cog, DatabaseBackup, FolderOpen, MonitorCog } from "lucide-react";

import type { AppBootstrap } from "@/types/domain";
import { routeMeta } from "@/components/layout/navigation";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

interface AppHeaderProps {
  route: keyof typeof routeMeta;
  bootstrap: AppBootstrap;
  onOpenDataDirectory: () => void;
  onRouteChange: (route: "/backups" | "/settings" | "/logs") => void;
}

export function AppHeader({
  route,
  bootstrap,
  onOpenDataDirectory,
  onRouteChange,
}: AppHeaderProps) {
  const meta = routeMeta[route];
  const targetLabel = bootstrap.dashboard.selectedTargetLabels.join("、") || "未选择";
  const lastSwitch = bootstrap.dashboard.lastSwitchSummary;

  return (
    <header className="flex h-full w-full flex-col justify-center gap-4 bg-transparent px-5 py-4 md:flex-row md:items-center md:justify-between md:max-h-[82px] xl:max-h-full">
      <div className="min-w-0">
        <h1 className="text-xl font-semibold tracking-tight text-slate-900">
          {meta.label}
        </h1>
        <p className="mt-1 text-sm text-slate-500">{meta.description}</p>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Button
          className="h-8 text-xs bg-white text-slate-700 hover:bg-slate-50"
          onClick={onOpenDataDirectory}
          type="button"
          variant="outline"
          size="sm"
        >
          <FolderOpen className="mr-1.5 h-3.5 w-3.5" />
          数据目录
        </Button>
        <Button
          className="h-8 text-xs bg-white text-slate-700 hover:bg-slate-50"
          onClick={() => onRouteChange("/backups")}
          type="button"
          variant="outline"
          size="sm"
        >
          <DatabaseBackup className="mr-1.5 h-3.5 w-3.5" />
          恢复入口
        </Button>
        <Button
          className="h-8 text-xs bg-white text-slate-700 hover:bg-slate-50"
          onClick={() => onRouteChange("/settings")}
          type="button"
          variant="outline"
          size="sm"
        >
          <Cog className="mr-1.5 h-3.5 w-3.5" />
          设置
        </Button>
        <Button
          className="h-8 text-xs"
          onClick={() => onRouteChange("/logs")}
          type="button"
          variant="secondary"
          size="sm"
        >
          <MonitorCog className="mr-1.5 h-3.5 w-3.5" />
          日志 / 关于
        </Button>
      </div>
    </header>
  );
}
