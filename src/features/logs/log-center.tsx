import { startTransition, useEffect, useMemo, useRef, useState } from "react";
import { RefreshCcw } from "lucide-react";
import { toast } from "sonner";

import { appApi } from "@/lib/tauri-api/app";
import { useAppShellStore } from "@/store/app-shell-store";
import type { LogEntry } from "@/types/domain";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { formatDateTime } from "@/lib/utils";

export function LogCenter() {
  const bootstrap = useAppShellStore((state) => state.bootstrap);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [isLoadingLogs, setIsLoadingLogs] = useState(false);
  const requestIdRef = useRef(0);

  const renderedLogs = useMemo(
    () => logs.map((item) => ({ ...item, contextText: JSON.stringify(item.context, null, 2) })),
    [logs],
  );

  const loadLogs = (errorMessage: string) => {
    const requestId = ++requestIdRef.current;
    setIsLoadingLogs(true);

    void appApi
      .readRecentLogs()
      .then((nextLogs) => {
        if (requestId !== requestIdRef.current) {
          return;
        }
        startTransition(() => {
          setLogs(nextLogs);
        });
      })
      .catch((error) => {
        if (requestId !== requestIdRef.current) {
          return;
        }
        toast.error(error.message ?? errorMessage);
      })
      .finally(() => {
        if (requestId !== requestIdRef.current) {
          return;
        }
        setIsLoadingLogs(false);
      });
  };

  useEffect(() => {
    loadLogs("读取日志失败");

    return () => {
      requestIdRef.current += 1;
    };
  }, []);

  if (!bootstrap) {
    return null;
  }

  return (
    <div className="grid h-full min-h-0 gap-4 overflow-hidden xl:grid-cols-[0.84fr_1.16fr] xl:grid-rows-[minmax(0,1fr)] xl:overflow-hidden">
      <Card className="flex min-h-0 flex-col overflow-hidden">
        <CardHeader>
          <CardTitle>关于</CardTitle>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 space-y-4 overflow-y-auto text-sm text-muted-foreground">
          <div className="rounded-md border border-border bg-slate-50/70 p-4">
            <p className="font-medium text-slate-900">Codex API Switcher</p>
            <p className="mt-2">版本：V{bootstrap.appMeta.version}</p>
            <p className="mt-1">应用数据、备份和日志目录均由桌面端受管。</p>
            <p className="mt-1">界面不展示本机真实路径，相关目录可通过受限入口打开。</p>
          </div>
          <Button
            disabled={isLoadingLogs}
            onClick={() => {
              loadLogs("刷新日志失败");
            }}
            variant="outline"
          >
            <RefreshCcw className="h-4 w-4" />
            {isLoadingLogs ? "刷新中..." : "刷新日志"}
          </Button>
        </CardContent>
      </Card>

      <Card className="flex min-h-0 flex-col overflow-hidden">
        <CardHeader>
          <CardTitle>结构化日志</CardTitle>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 space-y-3 overflow-y-auto">
          {renderedLogs.length ? (
            renderedLogs.map((item) => (
              <div
                className="rounded-md border border-border bg-white/80 p-4"
                key={`${item.timestamp}-${item.action}`}
              >
                <div className="flex flex-wrap items-center gap-2">
                  <Badge variant={item.level === "error" ? "red" : "blue"}>{item.level}</Badge>
                  <Badge variant="slate">{item.action}</Badge>
                  <span className="text-xs text-muted-foreground">
                    {formatDateTime(item.timestamp)}
                  </span>
                </div>
                <p className="mt-3 text-sm text-slate-900">{item.message}</p>
                <pre className="mt-3 overflow-auto rounded-md bg-slate-950 p-3 text-xs text-slate-200">
                  {item.contextText}
                </pre>
              </div>
            ))
          ) : (
            <div className="rounded-md border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
              暂无结构化日志。
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
