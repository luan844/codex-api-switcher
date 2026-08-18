import { useEffect, useMemo, useState } from "react";
import { CheckCheck, LoaderCircle, PlugZap, ShieldBan, TestTube2 } from "lucide-react";
import { toast } from "sonner";

import { appApi } from "@/lib/tauri-api/app";
import { formatDateTime } from "@/lib/utils";
import { useAppShellStore } from "@/store/app-shell-store";
import type { BatchConnectionTestItem, TemplateKind } from "@/types/domain";
import {
  filterProfilesByStatus,
  resolveBatchSelection,
  summarizeBatchResults,
  type ProfileStatusFilter,
} from "@/features/templates/batch-test-utils";
import { Checkbox } from "@/components/ui/checkbox";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";

export function TemplateAndTest() {
  const bootstrap = useAppShellStore((state) => state.bootstrap);
  const setBootstrap = useAppShellStore((state) => state.setBootstrap);
  const selectedProfileId = useAppShellStore((state) => state.selectedProfileId);
  const [activeTab, setActiveTab] = useState<TemplateKind>("openAi");
  const [openAiTemplate, setOpenAiTemplate] = useState(bootstrap?.templates.openAi ?? "");
  const [apiKeyTemplate, setApiKeyTemplate] = useState(bootstrap?.templates.apiKey ?? "");
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [isRestoring, setIsRestoring] = useState(false);
  const [overrideModel, setOverrideModel] = useState("");
  const [disableOnFailure, setDisableOnFailure] = useState(true);
  const [profileFilter, setProfileFilter] = useState<ProfileStatusFilter>("all");
  const [selectedProfileIds, setSelectedProfileIds] = useState<string[]>([]);
  const [testResults, setTestResults] = useState<BatchConnectionTestItem[]>([]);
  const apiKeyProfiles = useMemo(
    () => (bootstrap?.profiles ?? []).filter((profile) => profile.providerCategory === "apiKey"),
    [bootstrap?.profiles],
  );
  const enabledProfiles = useMemo(() => filterProfilesByStatus(apiKeyProfiles, "enabled"), [apiKeyProfiles]);
  const disabledProfiles = useMemo(() => filterProfilesByStatus(apiKeyProfiles, "disabled"), [apiKeyProfiles]);
  const visibleProfiles = useMemo(
    () => filterProfilesByStatus(apiKeyProfiles, profileFilter),
    [apiKeyProfiles, profileFilter],
  );
  const restorableProfiles = useMemo(
    () => visibleProfiles.filter((profile) => profile.autoDisabled),
    [visibleProfiles],
  );
  const selectedCount = selectedProfileIds.length;
  const resultSummary = useMemo(() => summarizeBatchResults(testResults), [testResults]);

  useEffect(() => {
    if (!bootstrap) {
      return;
    }
    setOpenAiTemplate(bootstrap.templates.openAi);
    setApiKeyTemplate(bootstrap.templates.apiKey);
  }, [bootstrap]);

  useEffect(() => {
    setTestResults([]);
  }, [apiKeyProfiles.length]);

  useEffect(() => {
    if (!apiKeyProfiles.length) {
      setSelectedProfileIds([]);
      return;
    }

    setSelectedProfileIds((current) => {
      return resolveBatchSelection(apiKeyProfiles, current, selectedProfileId);
    });
  }, [apiKeyProfiles, selectedProfileId]);

  const resultByProfileId = useMemo(
    () => new Map(testResults.map((item) => [item.profileId, item])),
    [testResults],
  );

  function toggleProfile(profileId: string, checked: boolean) {
    setSelectedProfileIds((current) => {
      if (checked) {
        return current.includes(profileId) ? current : [...current, profileId];
      }
      return current.filter((item) => item !== profileId);
    });
  }

  function selectAllEnabledProfiles() {
    setSelectedProfileIds(enabledProfiles.map((profile) => profile.id));
  }

  function clearSelection() {
    setSelectedProfileIds([]);
  }

  function restoreDisabledProfiles() {
    if (restorableProfiles.length === 0) {
      return;
    }

    setIsRestoring(true);
    const restoredProfileIds = new Set(restorableProfiles.map((profile) => profile.id));

    void Promise.all(
      restorableProfiles.map((profile) => appApi.setProfileTestDisabled(profile.id, false)),
    )
      .then(() => appApi.loadBootstrap())
      .then((data) => {
        setBootstrap(data);
        setTestResults((current) =>
          current.map((item) =>
            restoredProfileIds.has(item.profileId) ? { ...item, autoDisabled: false } : item,
          ),
        );
        toast.success(`已恢复 ${restorableProfiles.length} 个禁用组合`);
      })
      .catch((error) => toast.error(error.message ?? "批量恢复启用失败"))
      .finally(() => setIsRestoring(false));
  }

  if (!bootstrap) {
    return null;
  }

  return (
    <div className="grid h-full min-h-0 gap-4 overflow-hidden xl:grid-cols-[1fr_1.1fr] xl:grid-rows-[minmax(0,1fr)] xl:overflow-hidden">
      <Card className="flex min-h-0 flex-col overflow-hidden border-slate-200/60 shadow-sm">
        <CardHeader className="bg-slate-50/50 pb-4">
          <CardTitle className="text-lg">模板编辑器</CardTitle>
          <CardDescription>
            分别维护 OpenAI / APIKEY 模板，APIKEY 模板支持
            {" {base_url} "}、{" {provider_name} "}、{" {provider_key} "}变量。
          </CardDescription>
        </CardHeader>
        <CardContent className="flex min-h-0 flex-1 flex-col overflow-y-auto p-4">
          <Tabs
            className="flex min-h-0 flex-1 flex-col"
            onValueChange={(value) => setActiveTab(value as TemplateKind)}
            value={activeTab}
          >
            <TabsList className="w-full justify-start rounded-none border-b bg-transparent p-0">
              <TabsTrigger
                className="relative h-9 rounded-none border-b-2 border-b-transparent bg-transparent px-4 pb-3 pt-2 font-semibold text-muted-foreground shadow-none transition-none data-[state=active]:border-b-primary data-[state=active]:text-foreground data-[state=active]:shadow-none"
                value="openAi"
              >
                OpenAI 模板
              </TabsTrigger>
              <TabsTrigger
                className="relative h-9 rounded-none border-b-2 border-b-transparent bg-transparent px-4 pb-3 pt-2 font-semibold text-muted-foreground shadow-none transition-none data-[state=active]:border-b-primary data-[state=active]:text-foreground data-[state=active]:shadow-none"
                value="apiKey"
              >
                APIKEY 模板
              </TabsTrigger>
            </TabsList>
            <TabsContent className="mt-4 flex min-h-0 flex-1" value="openAi">
              <Textarea
                className="h-full min-h-[420px] flex-1 font-mono text-sm leading-relaxed border-0 bg-slate-50/50 focus-visible:ring-1 p-4"
                onChange={(event) => setOpenAiTemplate(event.target.value)}
                value={openAiTemplate}
              />
            </TabsContent>
            <TabsContent className="mt-4 flex min-h-0 flex-1" value="apiKey">
              <Textarea
                className="h-full min-h-[420px] flex-1 font-mono text-sm leading-relaxed border-0 bg-slate-50/50 focus-visible:ring-1 p-4"
                onChange={(event) => setApiKeyTemplate(event.target.value)}
                value={apiKeyTemplate}
              />
            </TabsContent>
          </Tabs>

          <div className="mt-4 flex flex-wrap gap-3">
            <Button
              disabled={isSaving}
              onClick={() => {
                setIsSaving(true);
                void appApi
                  .saveTemplate({
                    content: activeTab === "openAi" ? openAiTemplate : apiKeyTemplate,
                    kind: activeTab,
                  })
                  .then((data) => {
                    setBootstrap(data);
                    setOpenAiTemplate(data.templates.openAi);
                    setApiKeyTemplate(data.templates.apiKey);
                    toast.success("模板已保存");
                  })
                  .catch((error) => toast.error(error.message ?? "模板保存失败"))
                  .finally(() => setIsSaving(false));
              }}
              className="min-w-[120px]"
            >
              {isSaving ? <LoaderCircle className="mr-2 h-4 w-4 animate-spin" /> : null}
              保存模板
            </Button>
            <Button
              disabled={isSaving}
              onClick={() => {
                setIsSaving(true);
                void appApi
                  .resetTemplate(activeTab)
                  .then((data) => {
                    setBootstrap(data);
                    setOpenAiTemplate(data.templates.openAi);
                    setApiKeyTemplate(data.templates.apiKey);
                    toast.success("模板已重置");
                  })
                  .catch((error) => toast.error(error.message ?? "重置失败"))
                  .finally(() => setIsSaving(false));
              }}
              variant="outline"
              className="min-w-[120px]"
            >
              重置参数
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card className="flex min-h-0 flex-col overflow-hidden border-slate-200/60 shadow-sm bg-slate-50/30">
        <CardHeader className="bg-slate-50/80 pb-4 border-b">
          <CardTitle className="text-lg flex items-center justify-between">
            <span>连接测试</span>
            <span className="text-sm font-normal text-muted-foreground">已选 {selectedCount} / {apiKeyProfiles.length}</span>
          </CardTitle>
          <CardDescription>
            批量执行 APIKEY 连通性测试。支持自定义测试模型，自动封禁失效节点。
          </CardDescription>
        </CardHeader>
        <CardContent className="flex min-h-0 flex-1 flex-col p-0">
          <div className="flex flex-wrap items-center justify-between gap-3 p-4 border-b bg-white">
            <div className="flex flex-wrap gap-2 items-center">
              <Select
                onValueChange={(value) => setProfileFilter(value as ProfileStatusFilter)}
                value={profileFilter}
              >
                <SelectTrigger className="w-[140px] h-8 text-xs bg-slate-50">
                  <SelectValue placeholder="筛选状态" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">全部组合 ({apiKeyProfiles.length})</SelectItem>
                  <SelectItem value="enabled">仅看启用 ({enabledProfiles.length})</SelectItem>
                  <SelectItem value="disabled">仅看禁用 ({disabledProfiles.length})</SelectItem>
                </SelectContent>
              </Select>
              <Button onClick={selectAllEnabledProfiles} type="button" variant="outline" size="sm" className="h-8 text-xs">
                <CheckCheck className="mr-1.5 h-3.5 w-3.5" />
                全选可用
              </Button>
              <Button onClick={clearSelection} type="button" variant="ghost" size="sm" className="h-8 text-xs text-muted-foreground">
                清空
              </Button>
            </div>
            {restorableProfiles.length > 0 ? (
              <Button
                disabled={isRestoring}
                onClick={restoreDisabledProfiles}
                type="button"
                variant="outline"
                size="sm"
                className="h-8 text-xs text-brand-600 border-brand-200 bg-brand-50 hover:bg-brand-100"
              >
                {isRestoring ? <LoaderCircle className="mr-1.5 h-3 w-3 animate-spin" /> : null}
                恢复 {restorableProfiles.length} 个禁用项
              </Button>
            ) : null}
          </div>

          <div className="flex-1 overflow-y-auto p-4 space-y-3 bg-slate-50/50">
            {visibleProfiles.map((profile) => {
              const latestResult = resultByProfileId.get(profile.id);
              const isChecked = selectedProfileIds.includes(profile.id);
              return (
                <label
                  className={`flex cursor-pointer items-start gap-3 rounded-lg border p-4 transition-all duration-200 ${
                    profile.autoDisabled
                      ? "border-rose-200/70 bg-rose-50/30 opacity-70 grayscale-[0.3]"
                      : isChecked
                        ? "border-sky-300 bg-sky-50/40 shadow-sm ring-1 ring-sky-100"
                        : "border-slate-200 bg-white hover:border-slate-300 hover:shadow-sm"
                  }`}
                  key={profile.id}
                >
                  <Checkbox
                    checked={isChecked}
                    disabled={profile.autoDisabled}
                    onCheckedChange={(checked) => toggleProfile(profile.id, checked === true)}
                    className="mt-0.5"
                  />
                  <div className="min-w-0 flex-1 space-y-2.5">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <span className="font-semibold text-slate-900 text-sm tracking-tight">{profile.name}</span>
                      <div className="flex items-center gap-1.5">
                        {profile.autoDisabled ? <Badge variant="red" className="text-[10px] px-1.5 py-0">已禁用</Badge> : null}
                        {latestResult ? (
                          <Badge
                            variant={
                              latestResult.status === "success"
                                ? "green"
                                : latestResult.status === "warning"
                                  ? "amber"
                                  : "red"
                            }
                            className="text-[10px] px-1.5 py-0"
                          >
                            {latestResult.status === "success" ? "成功" : latestResult.status === "warning" ? "警告" : "失败"}
                          </Badge>
                        ) : null}
                      </div>
                    </div>
                    <div className="space-y-1">
                      <p className="truncate text-xs text-slate-500 font-mono" title={profile.baseUrl || "使用原生配置"}>
                        {profile.baseUrl || "默认配置路径 (通过模板映射)"}
                      </p>
                      <p className="text-xs text-slate-400">
                        使用模型：<span className="text-slate-600">{profile.testModel || "gpt-5.4-mini (默认)"}</span>
                      </p>
                    </div>
                    {(profile.autoDisabledReason || latestResult?.message) ? (
                      <div className={`mt-2 rounded p-2 text-xs border ${profile.autoDisabled ? "bg-rose-100/50 border-rose-100 text-rose-700" : "bg-slate-100/50 border-slate-100 text-slate-600"}`}>
                        <p className="line-clamp-2" title={latestResult?.message || profile.autoDisabledReason || ""}>
                          {latestResult?.message || profile.autoDisabledReason}
                        </p>
                        {profile.autoDisabledAtUtc && (
                          <p className="mt-1 text-[10px] opacity-80">禁用时间: {formatDateTime(profile.autoDisabledAtUtc)}</p>
                        )}
                      </div>
                    ) : null}
                  </div>
                </label>
              );
            })}
            {visibleProfiles.length === 0 ? (
              <div className="flex h-32 flex-col items-center justify-center rounded-lg border border-dashed border-slate-200 bg-white/50 text-slate-400">
                <ShieldBan className="mb-2 h-8 w-8 opacity-20" />
                <p className="text-sm">未找到符合条件的组合记录</p>
              </div>
            ) : null}
          </div>

          <div className="border-t bg-white p-4">
            <div className="flex flex-col xl:flex-row xl:items-center justify-between gap-4">
              <div className="flex flex-1 items-center gap-4">
                <div className="w-full max-w-[240px]">
                  <Label htmlFor="override-model" className="sr-only">测试模型覆盖</Label>
                  <Input
                    id="override-model"
                    className="h-9 text-sm"
                    onChange={(event) => setOverrideModel(event.target.value)}
                    placeholder="默认 gpt-5.4-mini, 另可覆写"
                    value={overrideModel}
                  />
                </div>
                <div className="flex items-center gap-2">
                  <Switch id="auto-disable" checked={disableOnFailure} onCheckedChange={setDisableOnFailure} />
                  <Label htmlFor="auto-disable" className="text-sm font-medium cursor-pointer">
                    失败自动禁用
                  </Label>
                </div>
              </div>

              <Button
                size="lg"
                className="w-full xl:w-auto font-medium"
                disabled={selectedCount === 0 || isTesting}
                onClick={() => {
                  if (selectedCount === 0) return;
                  setIsTesting(true);
                  void appApi
                    .batchTestProfileConnections({
                      disableOnFailure,
                      overrideModel: overrideModel.trim() || null,
                      profileIds: selectedProfileIds,
                    })
                    .then((response) => {
                      setBootstrap(response.bootstrap);
                      setTestResults(response.results);
                      const disabledCount = response.results.filter((item) => item.autoDisabled).length;
                      if (disabledCount > 0 && disableOnFailure) {
                        toast.warning(`已完成，${disabledCount} 个失效节点已被禁用`);
                      } else {
                        toast.success("测试通过，已全部完成");
                      }
                    })
                    .catch((error) => toast.error(error.message ?? "测试执行失败"))
                    .finally(() => setIsTesting(false));
                }}
              >
                {isTesting ? <LoaderCircle className="mr-2 h-4 w-4 animate-spin" /> : <PlugZap className="mr-2 h-4 w-4" />}
                执行批量测试 ({selectedCount})
              </Button>
            </div>

            {testResults.length > 0 && (
              <div className="mt-3 flex items-center gap-4 text-xs text-muted-foreground border-t pt-3">
                <span>测试结果摘要：</span>
                <span className="text-green-600 font-medium">成功 {resultSummary.success}</span>
                <span className="text-amber-500 font-medium">警告 {resultSummary.warning}</span>
                <span className="text-rose-500 font-medium">失败 {resultSummary.failure}</span>
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
