import { startTransition, useEffect, useRef, useState } from "react";
import { ArrowRightLeft, LoaderCircle } from "lucide-react";
import { toast } from "sonner";

import { appApi } from "@/lib/tauri-api/app";
import { useAppShellStore } from "@/store/app-shell-store";
import type { SwitchPreview } from "@/types/domain";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";

export function SwitchExecutor() {
  const bootstrap = useAppShellStore((state) => state.bootstrap);
  const selectedProfileId = useAppShellStore((state) => state.selectedProfileId);
  const setBootstrap = useAppShellStore((state) => state.setBootstrap);
  const [preview, setPreview] = useState<SwitchPreview | null>(null);
  const [isLoadingPreview, setIsLoadingPreview] = useState(false);
  const [isExecuting, setIsExecuting] = useState(false);
  const previewRequestRef = useRef(0);

  useEffect(() => {
    if (!selectedProfileId) {
      setPreview(null);
      return;
    }

    const requestId = ++previewRequestRef.current;
    setIsLoadingPreview(true);
    void appApi
      .getSwitchPreview(selectedProfileId)
      .then((data) => {
        if (requestId !== previewRequestRef.current) {
          return;
        }
        startTransition(() => {
          setPreview(data);
        });
      })
      .catch((error) => {
        if (requestId !== previewRequestRef.current) {
          return;
        }
        setPreview((current) => (current?.profileId === selectedProfileId ? null : current));
        toast.error(error.message ?? "预检查失败");
      })
      .finally(() => {
        if (requestId !== previewRequestRef.current) {
          return;
        }
        setIsLoadingPreview(false);
      });

    return () => {
      previewRequestRef.current += 1;
    };
  }, [selectedProfileId]);

  if (!bootstrap) {
    return null;
  }

  const currentProfile = bootstrap.profiles.find((profile) => profile.id === selectedProfileId);
  const activePreview = selectedProfileId && preview?.profileId === selectedProfileId ? preview : null;

  return (
    <div className="grid h-full min-h-0 gap-4 overflow-hidden xl:grid-cols-[0.95fr_1.05fr] xl:grid-rows-[minmax(0,1fr)] xl:overflow-hidden">
      <Card className="flex min-h-0 flex-col overflow-hidden">
        <CardHeader>
          <CardTitle>预检查与目标写入</CardTitle>
          <CardDescription>
            切换前先解析目标目录、配置来源和 provider 写入范围。
          </CardDescription>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 space-y-4 overflow-y-auto">
          {isLoadingPreview ? (
            <div className="flex items-center gap-2 rounded-md border border-border bg-slate-50/80 p-4 text-sm text-muted-foreground">
              <LoaderCircle className="h-4 w-4 animate-spin" />
              正在生成预检查结果…
            </div>
          ) : activePreview ? (
            <>
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="blue">{activePreview.profileName}</Badge>
                <Badge variant="teal">provider: {activePreview.providerName}</Badge>
              </div>

              <div className="space-y-3">
                {activePreview.targets.map((target) => (
                  <div
                    className="rounded-md border border-border bg-slate-50/80 p-4"
                    key={target.targetKey}
                  >
                    <p className="font-medium text-slate-900">{target.displayName}</p>
                    <p className="mt-2 text-sm text-muted-foreground">
                      目标标识：{target.targetKey}
                    </p>
                    <p className="mt-1 text-sm text-muted-foreground">
                      写入与备份目录由应用受管，预览中不展示真实路径。
                    </p>
                  </div>
                ))}
              </div>

              {activePreview.warnings.length ? (
                <div className="rounded-md border border-amber-200 bg-amber-50 p-4 text-sm text-amber-800">
                  {activePreview.warnings.map((warning) => (
                    <p key={warning}>{warning}</p>
                  ))}
                </div>
              ) : null}
            </>
          ) : (
            <div className="rounded-md border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
              请选择一个组合后查看切换预览。
            </div>
          )}
        </CardContent>
      </Card>

      <Card className="flex min-h-0 flex-col overflow-hidden">
        <CardHeader>
          <CardTitle>执行切换</CardTitle>
          <CardDescription>
            流程固定为 validate → backup → write auth → write config → migrate sessions
            → finalize。
          </CardDescription>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 space-y-4 overflow-y-auto">
          <div className="flex flex-wrap items-center gap-2">
            <Button
              disabled={!selectedProfileId || isExecuting || isLoadingPreview}
              onClick={() => {
                if (!selectedProfileId) {
                  return;
                }
                const confirmed = window.confirm(
                  `确认执行切换并覆盖${currentProfile?.name ?? "当前组合"}对应目标吗？`,
                );
                if (!confirmed) {
                  return;
                }

                setIsExecuting(true);
                void appApi
                  .executeSwitch(selectedProfileId)
                  .then((data) => {
                    setBootstrap(data);
                    toast.success(`已切换到 ${currentProfile?.name ?? "目标组合"}`);
                  })
                  .catch((error) => {
                    toast.error(error.message ?? "切换失败");
                  })
                  .finally(() => {
                    setIsExecuting(false);
                  });
              }}
            >
              {isExecuting ? (
                <LoaderCircle className="h-4 w-4 animate-spin" />
              ) : (
                <ArrowRightLeft className="h-4 w-4" />
              )}
              执行切换
            </Button>
            {bootstrap.dashboard.lastSwitchSummary ? (
              <Badge
                variant={
                  bootstrap.dashboard.lastSwitchSummary.status === "success"
                    ? "green"
                    : "red"
                }
              >
                {bootstrap.dashboard.lastSwitchSummary.status}
              </Badge>
            ) : null}
          </div>

          <Separator />

          <div className="space-y-3">
            {(bootstrap.dashboard.lastSwitchSummary?.steps ?? []).map((step) => (
              <div
                className="rounded-md border border-border bg-white/70 p-4"
                key={`${step.name}-${step.detail}`}
              >
                <div className="flex items-center justify-between gap-2">
                  <p className="font-medium capitalize text-slate-900">{step.name}</p>
                  <Badge
                    variant={
                      step.status === "success"
                        ? "green"
                        : step.status === "failure"
                          ? "red"
                          : "amber"
                    }
                  >
                    {step.status}
                  </Badge>
                </div>
                <p className="mt-2 text-sm text-muted-foreground">{step.detail}</p>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
