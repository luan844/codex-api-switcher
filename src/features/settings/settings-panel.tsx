import { useEffect, useRef, useState } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm, useWatch } from "react-hook-form";
import { toast } from "sonner";

import { appApi } from "@/lib/tauri-api/app";
import type { AppBootstrap, SaveSettingsInput } from "@/types/domain";
import { useAppShellStore } from "@/store/app-shell-store";
import {
  settingsFormSchema,
  type SettingsFormValues,
} from "@/features/settings/schema";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export function SettingsPanel() {
  const bootstrap = useAppShellStore((state) => state.bootstrap);
  const setBootstrap = useAppShellStore((state) => state.setBootstrap);
  const [isSaving, setIsSaving] = useState(false);
  const requestSequenceRef = useRef(0);
  const lastSubmittedRef = useRef("");

  const form = useForm<SettingsFormValues>({
    resolver: zodResolver(settingsFormSchema),
    defaultValues: bootstrap ? toSettingsFormValues(bootstrap) : undefined,
  });

  const watchedValues = useWatch({ control: form.control });
  const replaceWindowsTarget = useWatch({
    control: form.control,
    name: "replaceWindowsTarget",
  });
  const replaceWslTarget = useWatch({
    control: form.control,
    name: "replaceWslTarget",
  });
  const sidebarCollapsed = useWatch({
    control: form.control,
    name: "sidebarCollapsed",
  });

  useEffect(() => {
    if (!bootstrap) {
      return;
    }

    const nextValues = toSettingsFormValues(bootstrap);
    lastSubmittedRef.current = serializeSettings(nextValues);
    form.reset(nextValues);
  }, [bootstrap, form]);

  useEffect(() => {
    if (!bootstrap) {
      return;
    }

    const parsed = settingsFormSchema.safeParse(watchedValues);
    if (!parsed.success) {
      return;
    }

    const serialized = serializeSettings(parsed.data);
    if (serialized === lastSubmittedRef.current) {
      return;
    }

    const timer = window.setTimeout(() => {
      const requestId = ++requestSequenceRef.current;
      setIsSaving(true);
      void appApi
        .updateSettings(toSettingsPayload(parsed.data))
        .then((data) => {
          if (requestId !== requestSequenceRef.current) {
            return;
          }
          lastSubmittedRef.current = serializeSettings(toSettingsFormValues(data));
          setBootstrap(data);
        })
        .catch((error) => {
          if (requestId !== requestSequenceRef.current) {
            return;
          }
          toast.error(error instanceof Error ? error.message : "设置保存失败");
          const fallbackValues = toSettingsFormValues(bootstrap);
          lastSubmittedRef.current = serializeSettings(fallbackValues);
          form.reset(fallbackValues);
        })
        .finally(() => {
          if (requestId === requestSequenceRef.current) {
            setIsSaving(false);
          }
        });
    }, 350);

    return () => window.clearTimeout(timer);
  }, [bootstrap, form, setBootstrap, watchedValues]);

  if (!bootstrap) {
    return null;
  }

  return (
    <div className="grid h-full min-h-0 gap-4 overflow-hidden xl:grid-cols-[1fr_0.9fr] xl:grid-rows-[minmax(0,1fr)] xl:overflow-hidden">
      <Card className="flex min-h-0 flex-col overflow-hidden">
        <CardHeader>
          <CardTitle>系统设置</CardTitle>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 space-y-5 overflow-y-auto">
          <div className="rounded-md border border-border bg-white/80 px-4 py-3 text-sm text-muted-foreground">
            {isSaving ? "正在保存设置…" : "设置项变更会自动保存。"}
          </div>

          <div className="grid gap-3 rounded-md border border-border bg-slate-50/70 p-4">
            <Label htmlFor="providerName">APIKEY provider name</Label>
            <Input id="providerName" {...form.register("apiKeyProviderName")} />
          </div>

          <div className="grid gap-3 rounded-md border border-border bg-slate-50/70 p-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="distroName">WSL 发行版覆盖</Label>
              <Input id="distroName" {...form.register("wslDistroName")} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="userName">WSL 用户覆盖</Label>
              <Input id="userName" {...form.register("wslUserName")} />
            </div>
          </div>

          <div className="grid gap-3 rounded-md border border-border bg-slate-50/70 p-4 sm:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="migrationDays">Session migration days</Label>
              <Input
                id="migrationDays"
                max={30}
                min={0}
                type="number"
                {...form.register("sessionMigrationDays", { valueAsNumber: true })}
              />
            </div>
            <div className="space-y-2">
              <Label>默认 WSL 检测</Label>
              <div className="rounded-md border border-border bg-white/80 px-3 py-2 text-sm text-muted-foreground">
                {bootstrap.settings.cachedDefaultWsl
                  ? `${bootstrap.settings.cachedDefaultWsl.distroName} / ${bootstrap.settings.cachedDefaultWsl.userName}`
                  : bootstrap.settings.cachedDefaultWslErrorMessage ?? "暂无缓存"}
              </div>
            </div>
          </div>

          <div className="grid gap-3 rounded-md border border-border bg-slate-50/70 p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="font-medium text-slate-900">Windows 目标</p>
                <p className="text-sm text-muted-foreground">
                  关闭所有目标时会自动保留 Windows 目标。
                </p>
              </div>
              <Switch
                checked={replaceWindowsTarget ?? false}
                onCheckedChange={(checked) =>
                  form.setValue("replaceWindowsTarget", checked === true)
                }
              />
            </div>
            <div className="flex items-center justify-between">
              <div>
                <p className="font-medium text-slate-900">WSL 目标</p>
                <p className="text-sm text-muted-foreground">{bootstrap.dashboard.wslStatus}</p>
              </div>
              <Switch
                checked={replaceWslTarget ?? false}
                onCheckedChange={(checked) =>
                  form.setValue("replaceWslTarget", checked === true)
                }
              />
            </div>
            <div className="flex items-center justify-between">
              <div>
                <p className="font-medium text-slate-900">Sidebar 折叠</p>
                <p className="text-sm text-muted-foreground">即时保存 UI 偏好。</p>
              </div>
              <Switch
                checked={sidebarCollapsed ?? false}
                onCheckedChange={(checked) =>
                  form.setValue("sidebarCollapsed", checked === true)
                }
              />
            </div>
          </div>

          <Button
            onClick={() => {
              void appApi
                .refreshDefaultWsl()
                .then((data) => {
                  setBootstrap(data);
                  toast.success("默认 WSL 信息已刷新");
                })
                .catch((error) => toast.error(error.message ?? "刷新失败"));
            }}
            variant="outline"
          >
            刷新默认 WSL
          </Button>
        </CardContent>
      </Card>

      <Card className="flex min-h-0 flex-col overflow-hidden">
        <CardHeader>
          <CardTitle>当前状态</CardTitle>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 space-y-3 overflow-y-auto">
          <div className="rounded-md border border-border bg-white/80 p-4">
            <p className="text-sm font-medium text-slate-900">迁移版本</p>
            <p className="mt-2 text-sm text-muted-foreground">
              migrationVersion = {bootstrap.settings.migrationVersion}
            </p>
          </div>
          <div className="rounded-md border border-border bg-white/80 p-4">
            <p className="text-sm font-medium text-slate-900">数据访问策略</p>
            <p className="mt-2 text-sm text-muted-foreground">
              应用数据、备份和日志目录均由桌面端受管，不在界面中显示真实路径。
            </p>
          </div>
          <div className="rounded-md border border-border bg-white/80 p-4">
            <p className="text-sm font-medium text-slate-900">应用版本</p>
            <p className="mt-2 text-sm text-muted-foreground">
              V{bootstrap.appMeta.version}
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function toSettingsFormValues(bootstrap: AppBootstrap): SettingsFormValues {
  return {
    replaceWindowsTarget: bootstrap.settings.replaceWindowsTarget,
    replaceWslTarget: bootstrap.settings.replaceWslTarget,
    wslDistroName: bootstrap.settings.wslDistroName ?? "",
    wslUserName: bootstrap.settings.wslUserName ?? "",
    sessionMigrationDays: bootstrap.settings.sessionMigrationDays,
    apiKeyProviderName: bootstrap.settings.apiKeyProviderName,
    sidebarCollapsed: bootstrap.settings.uiPreferences.sidebarCollapsed,
  };
}

function toSettingsPayload(values: SettingsFormValues): SaveSettingsInput {
  return {
    replaceWindowsTarget: values.replaceWindowsTarget || !values.replaceWslTarget,
    replaceWslTarget: values.replaceWslTarget,
    wslDistroName: values.wslDistroName || null,
    wslUserName: values.wslUserName || null,
    sessionMigrationDays: values.sessionMigrationDays,
    apiKeyProviderName: values.apiKeyProviderName,
    uiPreferences: {
      sidebarCollapsed: values.sidebarCollapsed,
    },
  };
}

function serializeSettings(values: SettingsFormValues): string {
  return JSON.stringify(toSettingsPayload(values));
}
