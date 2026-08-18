import { useDeferredValue, useMemo, useState } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm, useWatch } from "react-hook-form";
import { CopyPlus, PencilLine, Plus, Search, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { appApi } from "@/lib/tauri-api/app";
import { useAppShellStore } from "@/store/app-shell-store";
import type { AuthMode, CodexProfile, ProviderCategory, SaveProfileInput } from "@/types/domain";
import { profileFormSchema, type ProfileFormValues } from "@/features/profiles/schema";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { sortByUpdatedAt } from "@/lib/utils";

type PathFieldName = "configTomlSourcePath" | "authJsonSourcePath";

const providerOptions: Array<{ label: string; value: ProviderCategory }> = [
  { label: "APIKEY", value: "apiKey" },
  { label: "OpenAI", value: "openAI" },
];

const authModeOptions: Array<{ label: string; value: AuthMode }> = [
  { label: "auth.json 文件", value: "authJsonFile" },
  { label: "直接输入 API Key", value: "apiKey" },
];

export function ProfileManagement() {
  const bootstrap = useAppShellStore((state) => state.bootstrap);
  const selectedProfileId = useAppShellStore((state) => state.selectedProfileId);
  const setSelectedProfileId = useAppShellStore((state) => state.setSelectedProfileId);
  const setBootstrap = useAppShellStore((state) => state.setBootstrap);
  const [keyword, setKeyword] = useState("");
  const deferredKeyword = useDeferredValue(keyword);
  const [providerFilter, setProviderFilter] = useState<ProviderCategory | "all">("all");
  const [sortField, setSortField] = useState<"updated" | "name">("updated");
  const [dialogOpen, setDialogOpen] = useState(false);

  const form = useForm<ProfileFormValues>({
    resolver: zodResolver(profileFormSchema),
    defaultValues: {
      providerCategory: "apiKey",
      authMode: "authJsonFile",
      importConfigToml: false,
      name: "",
      baseUrl: "",
      authJsonSourcePath: "",
      configTomlSourcePath: "",
      apiKey: "",
      testModel: "",
    },
  });
  const profiles = bootstrap?.profiles ?? [];
  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId);
  const filteredProfiles = useMemo(() => {
    const sortedProfiles =
      sortField === "updated"
        ? sortByUpdatedAt(profiles)
        : [...profiles].sort((left, right) =>
            left.name.localeCompare(right.name, "zh-CN"),
          );

    return sortedProfiles.filter((profile) => {
      const matchesKeyword =
        !deferredKeyword.trim() ||
        profile.name.toLowerCase().includes(deferredKeyword.trim().toLowerCase()) ||
        profile.baseUrl.toLowerCase().includes(deferredKeyword.trim().toLowerCase());
      const matchesProvider =
        providerFilter === "all" || profile.providerCategory === providerFilter;
      return matchesKeyword && matchesProvider;
    });
  }, [deferredKeyword, profiles, providerFilter, sortField]);

  function openCreateDialog() {
    form.reset({
      providerCategory: "apiKey",
      authMode: "authJsonFile",
      importConfigToml: false,
      name: "",
      baseUrl: "",
      authJsonSourcePath: "",
      configTomlSourcePath: "",
      apiKey: "",
      testModel: "",
    });
    setDialogOpen(true);
  }

  function openEditDialog(profile: CodexProfile) {
    form.reset({
      id: profile.id,
      name: profile.name,
      providerCategory: profile.providerCategory,
      authMode: profile.authMode,
      importConfigToml: profile.hasStoredConfigToml,
      baseUrl: profile.baseUrl,
      authJsonSourcePath: "",
      configTomlSourcePath: "",
      apiKey: "",
      testModel: profile.testModel ?? "",
    });
    setDialogOpen(true);
  }

  async function saveProfile(values: ProfileFormValues) {
    const payload: SaveProfileInput = {
      ...values,
      apiKey: values.apiKey || null,
      authJsonSourcePath: values.authJsonSourcePath || null,
      baseUrl: values.baseUrl || null,
      configTomlSourcePath: values.configTomlSourcePath || null,
      id: values.id,
      testModel: values.testModel || null,
    };
    const previousProfileIds = new Set(profiles.map((profile) => profile.id));
    const data = await appApi.saveProfile(payload);
    setBootstrap(data);
    const createdProfileId =
      payload.id ??
      data.profiles.find((profile) => !previousProfileIds.has(profile.id))?.id ??
      data.profiles.find((profile) => profile.name === payload.name)?.id ??
      null;
    setSelectedProfileId(createdProfileId);
    setDialogOpen(false);
    toast.success(values.id ? "配置已更新" : "配置已创建");
  }

  async function browseFilePath(
    fieldName: PathFieldName,
    filterName: string,
    extensions: string[],
  ) {
    try {
      const selectedPath = await appApi.pickFilePath({
        extensions,
        filterName,
        title: `选择${filterName}文件`,
      });
      if (selectedPath) {
        form.setValue(fieldName, selectedPath, { shouldDirty: true });
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : "打开文件选择器失败";
      toast.error(message);
    }
  }

  const providerCategory = useWatch({
    control: form.control,
    name: "providerCategory",
  });
  const importConfigToml = useWatch({
    control: form.control,
    name: "importConfigToml",
  });
  const authMode = useWatch({
    control: form.control,
    name: "authMode",
  });
  const editingProfileId = useWatch({
    control: form.control,
    name: "id",
  });

  if (!bootstrap) {
    return null;
  }

  return (
    <div className="grid h-full min-h-0 gap-4 overflow-hidden xl:grid-cols-[0.8fr_1.2fr] xl:grid-rows-[minmax(0,1fr)] xl:overflow-hidden">
      <Card className="flex min-h-0 flex-col overflow-hidden">
        <CardHeader>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <CardTitle>配置列表</CardTitle>
              <CardDescription>支持搜索、筛选、复制、删除与编辑。</CardDescription>
            </div>
            <Button onClick={openCreateDialog}>
              <Plus className="h-4 w-4" />
              新增配置
            </Button>
          </div>
        </CardHeader>
        <CardContent className="flex min-h-0 flex-1 flex-col space-y-4">
          <div className="flex min-w-0 flex-wrap gap-3" data-testid="profile-filters-toolbar">
            <div className="relative min-w-0 flex-[1_1_18rem] basis-[18rem]">
              <Search className="pointer-events-none absolute left-3 top-3.5 h-4 w-4 text-muted-foreground" />
              <Input
                className="pl-9"
                onChange={(event) => setKeyword(event.target.value)}
                placeholder="按名称或 Base URL 搜索"
                value={keyword}
              />
            </div>
            <div className="min-w-[11rem] flex-1 basis-[11rem] md:flex-none md:w-[12rem]">
              <Select
                onValueChange={(value) =>
                  setProviderFilter(value as ProviderCategory | "all")
                }
                value={providerFilter}
              >
                <SelectTrigger>
                  <SelectValue placeholder="提供商" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">全部提供商</SelectItem>
                  <SelectItem value="apiKey">APIKEY</SelectItem>
                  <SelectItem value="openAI">OpenAI</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div
              className="min-w-[11rem] basis-full md:basis-full xl:flex-none xl:basis-[10rem]"
              data-testid="profile-sort-filter"
            >
              <Select
                onValueChange={(value) => setSortField(value as "updated" | "name")}
                value={sortField}
              >
                <SelectTrigger>
                  <SelectValue placeholder="排序" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="updated">按更新时间</SelectItem>
                  <SelectItem value="name">按名称</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div className="min-h-0 flex-1 space-y-3 overflow-hidden">
            {filteredProfiles.map((profile) => (
              <button
                className={`group w-full min-w-0 rounded-md border p-4 text-left transition-all duration-300 ${
                  profile.id === selectedProfileId
                    ? "border-primary bg-sky-50/60 shadow-sm"
                    : "border-border bg-white/80 hover:bg-slate-50 hover:shadow-md"
                }`}
                key={profile.id}
                onClick={() => setSelectedProfileId(profile.id)}
                type="button"
              >
                <div className="flex min-w-0 items-start justify-between gap-2">
                  <p className="min-w-0 flex-1 truncate font-medium text-slate-900" title={profile.name}>
                    {profile.name}
                  </p>
                  <div className="flex shrink-0 flex-wrap justify-end gap-2">
                    <Badge variant={profile.providerCategory === "openAI" ? "blue" : "teal"}>
                      {profile.providerCategory === "openAI" ? "OpenAI" : "APIKEY"}
                    </Badge>
                    {profile.autoDisabled ? <Badge variant="red">已禁用</Badge> : null}
                  </div>
                </div>
                <p className="mt-2 break-all text-sm text-muted-foreground line-clamp-2">
                  {profile.baseUrl || "使用模板或导入文件"}
                </p>
              </button>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card className="flex min-h-0 flex-col overflow-hidden">
        <CardHeader>
          <CardTitle>配置详情</CardTitle>
          <CardDescription>
            表单按基础信息、认证方式、提供商与模板来源分组维护。
          </CardDescription>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 space-y-4 overflow-y-auto">
          {selectedProfile ? (
            <>
              <div className="grid gap-3 rounded-md border border-border bg-slate-50/70 p-5">
                <div className="flex flex-wrap items-center gap-2">
                  <Badge className="max-w-full truncate" variant="blue">
                    {selectedProfile.name}
                  </Badge>
                  <Badge variant="slate">{selectedProfile.authMode}</Badge>
                  <Badge
                    variant={selectedProfile.providerCategory === "openAI" ? "blue" : "teal"}
                  >
                    {selectedProfile.providerCategory}
                  </Badge>
                  {selectedProfile.autoDisabled ? <Badge variant="red">已自动禁用</Badge> : null}
                </div>
                <p className="break-all text-sm text-muted-foreground">
                  Base URL：{selectedProfile.baseUrl || "使用导入 config.toml 或模板"}
                </p>
                <p className="text-sm text-muted-foreground">
                  测试模型：{selectedProfile.testModel || "默认 gpt-5.4-mini"}
                </p>
                {selectedProfile.autoDisabledReason ? (
                  <p className="break-all text-sm text-rose-700">
                    禁用原因：{selectedProfile.autoDisabledReason}
                  </p>
                ) : null}
              </div>

              <div className="grid gap-3 rounded-md border border-border bg-white/80 p-5">
                <p className="text-sm text-muted-foreground">
                  auth.json：{selectedProfile.hasStoredAuthJson ? "已保存" : "无"}
                </p>
                <p className="text-sm text-muted-foreground">
                  config.toml：{selectedProfile.hasStoredConfigToml ? "已保存" : "无"}
                </p>
                <p className="text-sm text-muted-foreground">
                  保存的 API Key：{selectedProfile.hasStoredApiKey ? "是" : "否"}
                </p>
              </div>

              <div className="flex flex-wrap gap-2">
                <Button onClick={() => openEditDialog(selectedProfile)} variant="outline">
                  <PencilLine className="h-4 w-4" />
                  编辑
                </Button>
                {selectedProfile.autoDisabled ? (
                  <Button
                    onClick={() => {
                      void appApi
                        .setProfileTestDisabled(selectedProfile.id, false)
                        .then((data) => {
                          setBootstrap(data);
                          toast.success("组合已恢复启用");
                        })
                        .catch((error) => toast.error(error.message ?? "恢复启用失败"));
                    }}
                    variant="outline"
                  >
                    恢复启用
                  </Button>
                ) : null}
                <Button
                  onClick={() => {
                    void appApi
                      .duplicateProfile(selectedProfile.id)
                      .then((data) => {
                        const previousProfileIds = new Set(
                          bootstrap.profiles.map((profile) => profile.id),
                        );
                        setBootstrap(data);
                        setSelectedProfileId(
                          data.profiles.find((profile) => !previousProfileIds.has(profile.id))
                            ?.id ?? selectedProfile.id,
                        );
                        toast.success("配置已复制");
                      })
                      .catch((error) => toast.error(error.message ?? "复制失败"));
                  }}
                  variant="outline"
                >
                  <CopyPlus className="h-4 w-4" />
                  复制
                </Button>
                <Button
                  onClick={() => {
                    const confirmed = window.confirm(`确认删除 ${selectedProfile.name} 吗？`);
                    if (!confirmed) {
                      return;
                    }
                    void appApi
                      .deleteProfile(selectedProfile.id)
                      .then((data) => {
                        setBootstrap(data);
                        toast.success("配置已删除");
                      })
                      .catch((error) => toast.error(error.message ?? "删除失败"));
                  }}
                  variant="danger"
                >
                  <Trash2 className="h-4 w-4" />
                  删除
                </Button>
              </div>
            </>
          ) : (
            <div className="rounded-md border border-dashed border-border p-12 text-center text-sm text-muted-foreground">
              请选择一个配置，或新建配置。
            </div>
          )}
        </CardContent>
      </Card>

      <Dialog onOpenChange={setDialogOpen} open={dialogOpen}>
        <DialogContent className="max-h-[90vh] overflow-auto">
          <DialogHeader>
            <DialogTitle>{editingProfileId ? "编辑配置" : "新增配置"}</DialogTitle>
            <DialogDescription>
              OpenAI 强制使用 auth.json；APIKEY 支持 Base URL / config.toml 与 API Key /
              auth.json 两套组合。
            </DialogDescription>
          </DialogHeader>

          <form
            className="mt-4 space-y-4"
            onSubmit={form.handleSubmit((values) => {
              void saveProfile(values).catch((error) =>
                toast.error(error.message ?? "保存失败"),
              );
            })}
          >
            <div className="grid gap-4 rounded-md border border-border bg-slate-50/70 p-4 sm:grid-cols-2">
              <div className="space-y-2 sm:col-span-2">
                <Label htmlFor="profile-name">名称</Label>
                <Input id="profile-name" {...form.register("name")} />
              </div>

              <div className="space-y-2">
                <Label>提供商</Label>
                <Select
                  onValueChange={(value) =>
                    form.setValue("providerCategory", value as ProviderCategory)
                  }
                  value={providerCategory}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {providerOptions.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {providerCategory === "apiKey" ? (
                <div className="space-y-2">
                  <Label>认证方式</Label>
                  <Select
                    onValueChange={(value) =>
                      form.setValue("authMode", value as AuthMode)
                    }
                    value={authMode}
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {authModeOptions.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              ) : null}
            </div>

            {providerCategory === "apiKey" ? (
              <div className="grid gap-4 rounded-md border border-border bg-slate-50/70 p-4">
                <div className="flex items-center justify-between gap-2">
                  <div>
                    <p className="font-medium text-slate-900">导入 config.toml</p>
                    <p className="text-sm text-muted-foreground">
                      打开后会直接替换目标 config.toml，关闭则按 Base URL 生成。
                    </p>
                  </div>
                  <Checkbox
                    checked={importConfigToml}
                    onCheckedChange={(checked) =>
                      form.setValue("importConfigToml", checked === true)
                    }
                  />
                </div>

                {importConfigToml ? (
                  <div className="space-y-2">
                    <Label>config.toml 路径</Label>
                    <div className="flex gap-2">
                      <Input readOnly {...form.register("configTomlSourcePath")} />
                      <Button
                        onClick={() => {
                          void browseFilePath("configTomlSourcePath", "TOML", ["toml"]);
                        }}
                        type="button"
                        variant="outline"
                      >
                        浏览
                      </Button>
                    </div>
                  </div>
                ) : (
                  <div className="space-y-2">
                    <Label htmlFor="base-url">Base URL</Label>
                    <Input id="base-url" {...form.register("baseUrl")} />
                  </div>
                )}

                <div className="space-y-2">
                  <Label htmlFor="test-model">测试模型</Label>
                  <Input id="test-model" {...form.register("testModel")} />
                </div>
              </div>
            ) : null}

            <div className="grid gap-4 rounded-md border border-border bg-slate-50/70 p-4">
              {(providerCategory === "openAI" || authMode === "authJsonFile") ? (
                <div className="space-y-2">
                  <Label>auth.json 路径</Label>
                  <div className="flex gap-2">
                    <Input readOnly {...form.register("authJsonSourcePath")} />
                    <Button
                      onClick={() => {
                        void browseFilePath("authJsonSourcePath", "JSON", ["json"]);
                      }}
                      type="button"
                      variant="outline"
                    >
                      浏览
                    </Button>
                  </div>
                </div>
              ) : null}

              {providerCategory === "apiKey" && authMode === "apiKey" ? (
                <div className="space-y-2">
                  <Label htmlFor="api-key">API Key</Label>
                  <Input id="api-key" type="password" {...form.register("apiKey")} />
                </div>
              ) : null}
            </div>

            <div className="flex justify-end gap-2">
              <Button onClick={() => setDialogOpen(false)} type="button" variant="outline">
                取消
              </Button>
              <Button type="submit">{editingProfileId ? "保存变更" : "创建配置"}</Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
