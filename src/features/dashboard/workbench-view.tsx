import { useState } from "react";
import {
  ArrowLeftRight,
  DatabaseBackup,
  HardDriveDownload,
  ServerCog,
  Search,
} from "lucide-react";
import { toast } from "sonner";

import { appApi } from "@/lib/tauri-api/app";
import { useAppShellStore } from "@/store/app-shell-store";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { StatusTile } from "@/components/business/status-tile";
import { formatDateTime } from "@/lib/utils";

export function WorkbenchView() {
  const bootstrap = useAppShellStore((state) => state.bootstrap);
  const [searchQuery, setSearchQuery] = useState("");

  if (!bootstrap) {
    return null;
  }

  const activeProfile = bootstrap.profiles.find(
    (profile) => profile.id === bootstrap.dashboard.activeProfileId,
  );
  const lastSwitch = bootstrap.dashboard.lastSwitchSummary;

  const filteredProfiles = bootstrap.profiles.filter(
    (p) =>
      p.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      (p.baseUrl && p.baseUrl.toLowerCase().includes(searchQuery.toLowerCase())),
  );

  return (
    <div
      className="flex h-full min-h-0 flex-col gap-4 overflow-hidden xl:overflow-hidden"
      data-testid="workbench-layout"
    >
      <div className="grid shrink-0 gap-4 md:grid-cols-2 xl:grid-cols-4">
        <StatusTile
          icon={<ServerCog className="h-5 w-5" />}
          label="当前激活配置"
          meta={activeProfile?.providerCategory === "openAI" ? "OpenAI 模式" : "APIKEY 模式"}
          value={bootstrap.dashboard.activeProfileName ?? "尚未执行"}
          valueClassName="overflow-visible whitespace-normal break-all font-sans text-[1.35rem] leading-8 tracking-[-0.02em]"
        />
        <StatusTile
          icon={<ArrowLeftRight className="h-5 w-5" />}
          label="目标环境"
          meta={bootstrap.dashboard.wslStatus}
          value={bootstrap.dashboard.selectedTargetLabels.join("、") || "未选择"}
        />
        <StatusTile
          icon={<DatabaseBackup className="h-5 w-5" />}
          label="最近切换"
          meta={lastSwitch?.message ?? "暂无记录"}
          value={formatDateTime(lastSwitch?.executedAtUtc)}
        />
        <StatusTile
          icon={<HardDriveDownload className="h-5 w-5" />}
          label="备份状态"
          meta={`共 ${bootstrap.dashboard.backupOverview.reduce((count, item) => count + item.count, 0)} 组备份`}
          value={
            bootstrap.dashboard.backupOverview
              .map((item) => `${item.displayName}:${item.count}`)
              .join(" / ") || "暂无"
          }
        />
      </div>

      <div className="grid gap-4 xl:min-h-0 xl:flex-1 xl:grid-cols-2 2xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
        <Card className="flex min-h-[22rem] min-w-0 flex-col overflow-hidden xl:min-h-0">
          <CardHeader className="pb-4">
            <CardTitle>快速切换</CardTitle>
            <CardDescription className="mb-2">
              按当前选中的组合直接执行完整流程：校验、备份、写入与
              sessions/state_5.sqlite迁移。
            </CardDescription>
            <div className="relative mt-2">
              <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-slate-400" />
              <Input
                type="search"
                placeholder="搜索配置名称或地址..."
                className="pl-9 h-9 bg-slate-50 border-slate-200 text-sm focus-visible:ring-primary/30"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>
          </CardHeader>
          <CardContent
            className="space-y-4 xl:min-h-0 xl:flex-1 overflow-y-auto pr-4 xl:[scrollbar-gutter:stable] custom-scrollbar"
            data-testid="workbench-quick-switch-content"
          >
            {filteredProfiles.length > 0 ? (
              <div className="grid auto-rows-max gap-3 sm:grid-cols-2">
                {filteredProfiles.map((profile) => (
                  <button
                    className="group flex min-w-0 flex-col rounded-xl border border-slate-200 bg-white p-4 text-left transition-all duration-300 hover:border-primary/30 hover:bg-slate-50/50 hover:shadow-[0_4px_20px_-4px_rgba(0,0,0,0.05)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/20"
                    key={profile.id}
                  onClick={() => {
                    void appApi
                      .executeSwitch(profile.id)
                      .then((data) => {
                        useAppShellStore.getState().setBootstrap(data);
                        toast.success(`已切换到 ${profile.name}`);
                      })
                      .catch((error) => {
                        toast.error(error.message ?? "切换失败");
                      });
                  }}
                  type="button"
                >
                  <div className="flex min-w-0 items-start justify-between gap-2">
                    <p className="min-w-0 flex-1 truncate font-medium text-slate-900" title={profile.name}>
                      {profile.name}
                    </p>
                    <div className="flex items-center gap-1 shrink-0">
                      {profile.autoDisabled && (
                        <Badge className="shrink-0" variant="red">
                          已禁用
                        </Badge>
                      )}
                      <Badge
                        className="shrink-0"
                        variant={profile.providerCategory === "openAI" ? "blue" : "teal"}
                      >
                        {profile.providerCategory === "openAI" ? "OpenAI" : "APIKEY"}
                      </Badge>
                    </div>
                  </div>
                  <p
                    className="mt-2 break-all text-sm leading-6 text-muted-foreground line-clamp-2"
                    title={profile.baseUrl || "使用模板生成 config.toml"}
                  >
                    {profile.baseUrl || "使用模板生成 config.toml"}
                  </p>
                </button>
                ))}
              </div>
            ) : (
              <div className="flex h-32 items-center justify-center rounded-xl border border-dashed border-slate-200 bg-slate-50/50 text-sm text-slate-500">
                暂无匹配的配置
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="flex min-h-[22rem] min-w-0 flex-col overflow-hidden xl:min-h-0">
          <CardHeader>
            <CardTitle>最近一次执行</CardTitle>
            <CardDescription>切换主流程的步骤结果和摘要。</CardDescription>
          </CardHeader>
          <CardContent
            className="space-y-4 xl:min-h-0 xl:flex-1 xl:overflow-y-auto xl:pr-2 xl:[scrollbar-gutter:stable]"
            data-testid="workbench-last-switch-content"
          >
            {lastSwitch ? (
              <>
                <div className="flex flex-wrap items-center gap-2">
                  <Badge variant={lastSwitch.status === "success" ? "green" : "red"}>
                    {lastSwitch.status === "success" ? "成功" : "失败"}
                  </Badge>
                  <Badge variant="slate">{lastSwitch.profileName}</Badge>
                  {lastSwitch.targets.map((target) => (
                    <Badge key={target} variant="blue">
                      {target}
                    </Badge>
                  ))}
                </div>
                <p className="break-all text-sm text-muted-foreground">{lastSwitch.message}</p>
                <Separator />
                <div className="space-y-3">
                  {lastSwitch.steps.map((step) => (
                    <div
                      className="rounded-md border border-border bg-slate-50/70 p-3"
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
                      <p className="mt-2 break-all text-sm text-muted-foreground">
                        {step.detail}
                      </p>
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <div className="rounded-md border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                尚未执行切换。
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
