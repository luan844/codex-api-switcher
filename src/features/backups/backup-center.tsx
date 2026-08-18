import { RotateCcw } from "lucide-react";
import { toast } from "sonner";

import { appApi } from "@/lib/tauri-api/app";
import { useAppShellStore } from "@/store/app-shell-store";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

export function BackupCenter() {
  const bootstrap = useAppShellStore((state) => state.bootstrap);
  const setBootstrap = useAppShellStore((state) => state.setBootstrap);

  if (!bootstrap) {
    return null;
  }

  return (
    <div className="grid h-full min-h-0 gap-4 overflow-hidden xl:grid-cols-[0.72fr_1.28fr] xl:grid-rows-[minmax(0,1fr)] xl:overflow-hidden">
      <Card className="flex min-h-0 flex-col overflow-hidden">
        <CardHeader>
          <CardTitle>恢复最近一次</CardTitle>
          <CardDescription>
            将按当前已启用目标恢复最近一组备份。恢复前请确认当前目录中的 auth/config
            可以被覆盖。
          </CardDescription>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 space-y-4 overflow-y-auto">
          <div className="space-y-3">
            {bootstrap.dashboard.backupOverview.map((item) => (
              <div
                className="rounded-md border border-border bg-slate-50/70 p-4"
                key={item.targetKey}
              >
                <div className="flex items-center justify-between gap-2">
                  <p className="font-medium text-slate-900">{item.displayName}</p>
                  <Badge variant="slate">{item.count} 组</Badge>
                </div>
                <p className="mt-2 text-sm text-muted-foreground">
                  最近备份：{item.hasRecentBackup ? "已存在" : "暂无"}
                </p>
              </div>
            ))}
          </div>

          <Button
            onClick={() => {
              const confirmed = window.confirm("确认按当前启用目标恢复最近一次备份吗？");
              if (!confirmed) {
                return;
              }

              void appApi
                .restoreLatestBackup()
                .then((data) => {
                  setBootstrap(data);
                  toast.success("最近备份已恢复");
                })
                .catch((error) => toast.error(error.message ?? "恢复失败"));
            }}
            variant="danger"
          >
            <RotateCcw className="h-4 w-4" />
            恢复最近一次
          </Button>
        </CardContent>
      </Card>

      <Card className="flex min-h-0 flex-col overflow-hidden">
        <CardHeader>
          <CardTitle>备份列表</CardTitle>
          <CardDescription>
            可按目标查看 Windows / WSL 备份内容，并直接打开目录。
          </CardDescription>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 space-y-3 overflow-y-auto">
          {bootstrap.backups.length ? (
            bootstrap.backups.map((item) => (
              <div
                className="rounded-md border border-border bg-white/80 p-4"
                key={`${item.targetKey}-${item.directoryName}`}
              >
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div>
                    <p className="font-medium text-slate-900">{item.displayName}</p>
                    <p className="mt-1 text-sm text-muted-foreground">{item.createdAtLabel}</p>
                  </div>
                  <div className="flex gap-2">
                    <Badge variant={item.hasAuthJson ? "green" : "amber"}>
                      auth:{String(item.hasAuthJson)}
                    </Badge>
                    <Badge variant={item.hasConfigToml ? "green" : "amber"}>
                      config:{String(item.hasConfigToml)}
                    </Badge>
                  </div>
                </div>
                <p className="mt-3 text-sm text-muted-foreground">
                  备份标识：{item.directoryName}
                </p>
                <div className="mt-3">
                  <Button
                    onClick={() => {
                      void appApi.openBackupDirectory(item.targetKey, item.directoryName);
                    }}
                    size="sm"
                    variant="outline"
                  >
                    打开目录
                  </Button>
                </div>
              </div>
            ))
          ) : (
            <div className="rounded-md border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
              暂无备份。
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
