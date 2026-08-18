import { useCallback, useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Activity,
  ArchiveRestore,
  Check,
  ChevronRight,
  CircleAlert,
  Copy,
  DatabaseBackup,
  FolderOpen,
  KeyRound,
  LoaderCircle,
  Maximize2,
  Minimize2,
  PanelLeft,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  Save,
  Server,
  Settings,
  ShieldCheck,
  SquareTerminal,
  Trash2,
  X,
} from "lucide-react";
import { Toaster, toast } from "sonner";

import { switcherApi } from "@/switcher/api";
import type {
  ApiFailure,
  ModelEntry,
  ProviderProfile,
  SaveProviderInput,
  SwitchPreview,
  SwitchResult,
  SwitcherBootstrap,
  SwitcherLog,
} from "@/switcher/types";

type View = "providers" | "backups" | "logs" | "settings";

const emptyProvider = (injectModels = true): SaveProviderInput => ({
  id: null,
  name: "",
  providerId: "",
  baseUrl: "https://",
  apiKey: "",
  clearApiKey: false,
  defaultModel: "",
  models: [],
  timeoutSeconds: 30,
  injectModels,
});

function profileToForm(profile: ProviderProfile): SaveProviderInput {
  return {
    id: profile.id,
    name: profile.name,
    providerId: profile.providerId,
    baseUrl: profile.baseUrl,
    apiKey: "",
    clearApiKey: false,
    defaultModel: profile.defaultModel,
    models: profile.models.map((model) => ({ ...model })),
    timeoutSeconds: profile.timeoutSeconds,
    injectModels: profile.injectModels,
  };
}

export function AppRoot() {
  const [bootstrap, setBootstrap] = useState<SwitcherBootstrap | null>(null);
  const [loading, setLoading] = useState(true);
  const [view, setView] = useState<View>("providers");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [form, setForm] = useState<SaveProviderInput>(emptyProvider());
  const [busy, setBusy] = useState<string | null>(null);
  const [preview, setPreview] = useState<SwitchPreview | null>(null);
  const [lastResult, setLastResult] = useState<SwitchResult | null>(null);
  const [logs, setLogs] = useState<SwitcherLog[]>([]);
  const [confirm, setConfirm] = useState<{
    title: string;
    body: string;
    confirmLabel: string;
    danger?: boolean;
    action: () => Promise<void>;
  } | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const data = await switcherApi.load();
      setBootstrap(data);
      setSelectedId((current) => {
        const next = current && data.profiles.some((profile) => profile.id === current)
          ? current
          : data.activeProfileId ?? data.profiles[0]?.id ?? null;
        const profile = data.profiles.find((item) => item.id === next);
        setForm(profile ? profileToForm(profile) : emptyProvider(data.settings.injectModelsDefault));
        return next;
      });
    } catch (error) {
      toast.error(errorText(error));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (view !== "logs") return;
    void switcherApi
      .readLogs()
      .then(setLogs)
      .catch((error) => toast.error(errorText(error)));
  }, [view, lastResult]);

  const activeProfile = bootstrap?.profiles.find(
    (profile) => profile.id === bootstrap.activeProfileId,
  );
  const selectedProfile = bootstrap?.profiles.find((profile) => profile.id === selectedId);

  function selectProvider(profile: ProviderProfile) {
    setSelectedId(profile.id);
    setForm(profileToForm(profile));
    setView("providers");
    setPreview(null);
  }

  function startNewProvider() {
    setSelectedId(null);
    setForm(emptyProvider(bootstrap?.settings.injectModelsDefault ?? true));
    setView("providers");
    setPreview(null);
  }

  async function saveCurrent(showToast = true): Promise<ProviderProfile> {
    setBusy("save");
    try {
      const data = await switcherApi.saveProvider(form);
      setBootstrap(data);
      const saved =
        data.profiles.find((profile) => profile.id === form.id) ??
        [...data.profiles]
          .reverse()
          .find(
            (profile) =>
              profile.providerId === form.providerId.trim().toLowerCase() &&
              profile.name === form.name.trim(),
          );
      if (!saved) throw new Error("Provider 已保存，但无法重新定位记录。");
      setSelectedId(saved.id);
      setForm(profileToForm(saved));
      if (showToast) toast.success("Provider 已保存");
      return saved;
    } finally {
      setBusy(null);
    }
  }

  async function discoverModels() {
    try {
      const saved = await saveCurrent(false);
      setBusy("models");
      const result = await switcherApi.discoverModels(saved.id);
      const defaultModel =
        form.defaultModel || result.models[0]?.id || saved.defaultModel;
      const discoveredForm: SaveProviderInput = {
        id: saved.id,
        name: saved.name,
        providerId: saved.providerId,
        baseUrl: saved.baseUrl,
        apiKey: null,
        clearApiKey: false,
        models: result.models,
        defaultModel,
        timeoutSeconds: saved.timeoutSeconds,
        injectModels: saved.injectModels,
      };
      const data = await switcherApi.saveProvider(discoveredForm);
      const updated = data.profiles.find((profile) => profile.id === saved.id);
      setBootstrap(data);
      setForm(updated ? profileToForm(updated) : discoveredForm);
      toast.success(result.message);
    } catch (error) {
      toast.error(errorText(error));
    } finally {
      setBusy(null);
    }
  }

  async function testConnection() {
    if (!form.defaultModel.trim()) {
      toast.error("请先获取模型并选择默认模型");
      return;
    }
    try {
      const saved = await saveCurrent(false);
      setBusy("test");
      const result = await switcherApi.testProvider(saved.id);
      toast.success(result.message);
    } catch (error) {
      toast.error(errorText(error));
    } finally {
      setBusy(null);
    }
  }

  async function prepareSwitch() {
    if (!form.defaultModel.trim()) {
      toast.error("请先获取模型并选择默认模型");
      return;
    }
    try {
      const saved = await saveCurrent(false);
      setBusy("preview");
      const result = await switcherApi.previewSwitch(saved.id);
      setPreview(result);
    } catch (error) {
      toast.error(errorText(error));
    } finally {
      setBusy(null);
    }
  }

  async function runSwitch(forceClose = false) {
    if (!preview) return;
    setBusy("switch");
    try {
      const result = await switcherApi.executeSwitch(
        preview.profileId,
        forceClose,
        true,
      );
      setBootstrap(result.bootstrap);
      setLastResult(result);
      setPreview(null);
      toast.success(`已切换到 ${result.profileName}`);
      for (const warning of result.warnings) toast.warning(warning);
    } catch (error) {
      const failure = error as ApiFailure;
      if (failure?.code === "codex_force_close_required") {
        setConfirm({
          title: "Codex 尚未退出",
          body: failure.message ?? "需要强制关闭 Codex 才能继续。",
          confirmLabel: "强制关闭并继续",
          danger: true,
          action: async () => {
            setConfirm(null);
            await runSwitch(true);
          },
        });
      } else {
        toast.error(errorText(error));
      }
    } finally {
      setBusy(null);
    }
  }

  async function restoreOfficial(forceClose = false) {
    setBusy("official");
    try {
      const result = await switcherApi.restoreOfficial(forceClose, true);
      setBootstrap(result.bootstrap);
      setLastResult(result);
      toast.success("已恢复官方 OpenAI 配置");
      for (const warning of result.warnings) toast.warning(warning);
    } catch (error) {
      const failure = error as ApiFailure;
      if (failure?.code === "codex_force_close_required") {
        setConfirm({
          title: "Codex 尚未退出",
          body: failure.message ?? "需要强制关闭 Codex 才能继续。",
          confirmLabel: "强制关闭并恢复",
          danger: true,
          action: async () => {
            setConfirm(null);
            await restoreOfficial(true);
          },
        });
      } else {
        toast.error(errorText(error));
      }
    } finally {
      setBusy(null);
    }
  }

  if (loading || !bootstrap) {
    return (
      <div className="app-loading">
        <LoaderCircle className="spin" size={22} />
        <span>正在读取 Codex 配置</span>
      </div>
    );
  }

  return (
    <div className="app-shell">
      <Titlebar version={bootstrap.appVersion} />
      <div className="app-body">
        <aside className="rail">
          <div className="brand-block">
            <div className="brand-mark"><SquareTerminal size={20} /></div>
            <div>
              <strong>API Switcher</strong>
              <span>Codex Desktop</span>
            </div>
          </div>

          <nav className="rail-nav" aria-label="主导航">
            <NavButton active={view === "providers"} icon={<Server size={17} />} label="Providers" onClick={() => setView("providers")} />
            <NavButton active={view === "backups"} icon={<DatabaseBackup size={17} />} label="备份恢复" onClick={() => setView("backups")} />
            <NavButton active={view === "logs"} icon={<Activity size={17} />} label="运行日志" onClick={() => setView("logs")} />
            <NavButton active={view === "settings"} icon={<Settings size={17} />} label="设置" onClick={() => setView("settings")} />
          </nav>

          <div className="rail-status">
            <span className={`status-dot ${bootstrap.runtime.running ? "online" : ""}`} />
            <div>
              <strong>{bootstrap.runtime.running ? "Codex 运行中" : "Codex 未运行"}</strong>
              <span>{bootstrap.runtime.version ? `v${bootstrap.runtime.version}` : "未检测到版本"}</span>
            </div>
          </div>
        </aside>

        <main className="workspace">
          {view === "providers" && (
            <ProviderWorkspace
              activeProfile={activeProfile}
              bootstrap={bootstrap}
              busy={busy}
              form={form}
              lastResult={lastResult}
              onChange={setForm}
              onDiscover={discoverModels}
              onDuplicate={async () => {
                if (!selectedId) return;
                try {
                  const data = await switcherApi.duplicateProvider(selectedId);
                  setBootstrap(data);
                  const duplicated = data.profiles[data.profiles.length - 1];
                  if (duplicated) selectProvider(duplicated);
                  toast.success("Provider 已复制");
                } catch (error) {
                  toast.error(errorText(error));
                }
              }}
              onNew={startNewProvider}
              onPrepareSwitch={prepareSwitch}
              onRestoreOfficial={() => void restoreOfficial()}
              onSave={() => void saveCurrent()}
              onSelect={selectProvider}
              onTest={testConnection}
              onDelete={() => {
                if (!selectedId || !selectedProfile) return;
                setConfirm({
                  title: "删除 Provider",
                  body: `确定删除“${selectedProfile.name}”？此操作不会修改当前 Codex 配置。`,
                  confirmLabel: "删除",
                  danger: true,
                  action: async () => {
                    const data = await switcherApi.deleteProvider(selectedId);
                    setBootstrap(data);
                    setConfirm(null);
                    startNewProvider();
                    toast.success("Provider 已删除");
                  },
                });
              }}
            />
          )}
          {view === "backups" && (
            <BackupView
              bootstrap={bootstrap}
              onRefresh={refresh}
              onRestore={(backupId) => {
                setConfirm({
                  title: "恢复此备份",
                  body: "恢复会覆盖该次切换涉及的 Codex 配置、会话和状态数据库。若文件后来被修改，应用会先阻止并要求二次确认。",
                  confirmLabel: "开始恢复",
                  action: async () => {
                    try {
                      const data = await switcherApi.restoreBackup(backupId);
                      setBootstrap(data);
                      setConfirm(null);
                      toast.success("备份已恢复");
                    } catch (error) {
                      const failure = error as ApiFailure;
                      if (failure?.code === "backup_conflict") {
                        setConfirm({
                          title: "检测到文件冲突",
                          body: failure.message ?? "文件在备份后被修改。",
                          confirmLabel: "强制恢复",
                          danger: true,
                          action: async () => {
                            const data = await switcherApi.restoreBackup(backupId, true);
                            setBootstrap(data);
                            setConfirm(null);
                            toast.success("备份已强制恢复");
                          },
                        });
                      } else {
                        toast.error(errorText(error));
                      }
                    }
                  },
                });
              }}
            />
          )}
          {view === "logs" && <LogView logs={logs} />}
          {view === "settings" && (
            <SettingsView
              bootstrap={bootstrap}
              onSaved={setBootstrap}
            />
          )}
        </main>
      </div>

      {preview && (
        <PreviewDialog
          busy={busy === "switch"}
          preview={preview}
          onCancel={() => setPreview(null)}
          onConfirm={() => void runSwitch()}
        />
      )}
      {confirm && (
        <ConfirmDialog
          danger={confirm.danger}
          title={confirm.title}
          body={confirm.body}
          confirmLabel={confirm.confirmLabel}
          onCancel={() => setConfirm(null)}
          onConfirm={() => {
            void confirm.action().catch((error) => toast.error(errorText(error)));
          }}
        />
      )}
      <Toaster position="top-right" richColors closeButton />
    </div>
  );
}

function ProviderWorkspace(props: {
  bootstrap: SwitcherBootstrap;
  activeProfile?: ProviderProfile;
  form: SaveProviderInput;
  busy: string | null;
  lastResult: SwitchResult | null;
  onChange: (form: SaveProviderInput) => void;
  onSelect: (profile: ProviderProfile) => void;
  onNew: () => void;
  onSave: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onDiscover: () => void;
  onTest: () => void;
  onPrepareSwitch: () => void;
  onRestoreOfficial: () => void;
}) {
  const { bootstrap, form, busy, onChange } = props;
  return (
    <div className="provider-layout">
      <section className="provider-list-panel">
        <div className="section-heading">
          <div>
            <span className="eyebrow">ROUTING PROFILES</span>
            <h1>API Providers</h1>
          </div>
          <IconButton label="新增 Provider" onClick={props.onNew}><Plus size={17} /></IconButton>
        </div>

        <button
          className={`provider-row official ${!bootstrap.activeProfileId ? "active" : ""}`}
          onClick={props.onRestoreOfficial}
          type="button"
        >
          <div className="provider-symbol official-symbol"><ShieldCheck size={17} /></div>
          <div className="provider-row-copy">
            <strong>官方 OpenAI</strong>
            <span>ChatGPT / OpenAI 官方认证</span>
          </div>
          {!bootstrap.activeProfileId && <span className="active-tag">当前</span>}
        </button>

        <div className="provider-scroll">
          {bootstrap.profiles.map((profile) => (
            <button
              className={`provider-row ${form.id === profile.id ? "selected" : ""}`}
              key={profile.id}
              onClick={() => props.onSelect(profile)}
              type="button"
            >
              <div className="provider-symbol">{profile.name.slice(0, 1).toUpperCase()}</div>
              <div className="provider-row-copy">
                <strong>{profile.name}</strong>
                <span>{profile.baseUrl}</span>
              </div>
              {bootstrap.activeProfileId === profile.id ? (
                <span className="active-tag">当前</span>
              ) : (
                <ChevronRight size={16} />
              )}
            </button>
          ))}
          {bootstrap.profiles.length === 0 && (
            <div className="empty-list">
              <Server size={20} />
              <span>还没有自定义 Provider</span>
            </div>
          )}
        </div>
      </section>

      <section className="editor-panel">
        <div className="editor-toolbar">
          <div>
            <span className="eyebrow">{form.id ? "EDIT PROVIDER" : "NEW PROVIDER"}</span>
            <h2>{form.name || "未命名 Provider"}</h2>
          </div>
          <div className="toolbar-actions">
            {form.id && (
              <>
                <IconButton label="复制 Provider" onClick={props.onDuplicate}><Copy size={16} /></IconButton>
                <IconButton danger label="删除 Provider" onClick={props.onDelete}><Trash2 size={16} /></IconButton>
              </>
            )}
            <button className="button secondary" disabled={busy !== null} onClick={props.onTest} type="button">
              {busy === "test" ? <LoaderCircle className="spin" size={16} /> : <Activity size={16} />}
              测试连接
            </button>
            <button className="button" disabled={busy !== null} onClick={props.onSave} type="button">
              {busy === "save" ? <LoaderCircle className="spin" size={16} /> : <Save size={16} />}
              保存
            </button>
          </div>
        </div>

        <div className="editor-scroll">
          <div className="form-grid two">
            <Field label="显示名称">
              <input value={form.name} onChange={(event) => onChange({ ...form, name: event.target.value })} placeholder="例如：公司 API" />
            </Field>
            <Field label="Provider ID" hint="仅小写字母、数字、_ 和 -">
              <input value={form.providerId} onChange={(event) => onChange({ ...form, providerId: event.target.value.toLowerCase() })} placeholder="company_api" />
            </Field>
          </div>

          <Field label="Responses API 地址" hint="远程地址必须使用 HTTPS；软件会自动拼接 /models 与 /responses">
            <div className="input-with-icon">
              <Server size={16} />
              <input value={form.baseUrl} onChange={(event) => onChange({ ...form, baseUrl: event.target.value })} placeholder="https://api.example.com/v1" />
            </div>
          </Field>

          <div className="form-grid two">
            <Field label="API Key" hint={form.id && !form.apiKey ? "留空表示保留已经加密保存的 Key" : "使用 Windows DPAPI 加密保存"}>
              <div className="input-with-icon">
                <KeyRound size={16} />
                <input type="password" autoComplete="new-password" value={form.apiKey ?? ""} onChange={(event) => onChange({ ...form, apiKey: event.target.value, clearApiKey: false })} placeholder={form.id ? "••••••••••••••••" : "sk-..."} />
              </div>
            </Field>
            <Field label="连接超时">
              <div className="number-field">
                <input min={5} max={180} type="number" value={form.timeoutSeconds} onChange={(event) => onChange({ ...form, timeoutSeconds: Number(event.target.value) })} />
                <span>秒</span>
              </div>
            </Field>
          </div>

          <div className="model-section">
            <div className="model-heading">
              <div>
                <span className="eyebrow">MODEL CATALOG</span>
                <h3>模型目录</h3>
              </div>
              <div className="toolbar-actions">
                <button className="button secondary" disabled={busy !== null} onClick={props.onDiscover} type="button">
                  {busy === "models" ? <LoaderCircle className="spin" size={16} /> : <RefreshCw size={16} />}
                  从 API 获取
                </button>
                <button className="button tertiary" onClick={() => onChange({ ...form, models: [...form.models, { id: "", displayName: "", contextWindow: 128000 }] })} type="button">
                  <Plus size={16} /> 添加模型
                </button>
              </div>
            </div>

            <div className="model-table">
              <div className="model-table-head">
                <span>模型 ID</span><span>显示名称</span><span>上下文</span><span />
              </div>
              {form.models.map((model, index) => (
                <ModelRow
                  key={`${index}-${model.id}`}
                  model={model}
                  onChange={(next) => {
                    const models = [...form.models];
                    models[index] = next;
                    onChange({ ...form, models });
                  }}
                  onRemove={() => onChange({ ...form, models: form.models.filter((_, itemIndex) => itemIndex !== index) })}
                />
              ))}
              {form.models.length === 0 && (
                <div className="model-empty">从 API 获取模型，或手动添加第一条模型记录。</div>
              )}
            </div>
          </div>

          <div className="form-grid two">
            <Field label="默认模型">
              <select value={form.defaultModel} onChange={(event) => onChange({ ...form, defaultModel: event.target.value })}>
                <option value="">选择默认模型</option>
                {form.models.filter((model) => model.id).map((model) => (
                  <option key={model.id} value={model.id}>{model.displayName || model.id}</option>
                ))}
              </select>
            </Field>
            <Field label="Codex Desktop">
              <label className="toggle-row">
                <input checked={form.injectModels} onChange={(event) => onChange({ ...form, injectModels: event.target.checked })} type="checkbox" />
                <span className="toggle" />
                <span>
                  <strong>注入模型菜单</strong>
                  <small>重启后将自定义模型加入 Codex 选择器</small>
                </span>
              </label>
            </Field>
          </div>
        </div>

        <div className="action-bar">
          <div className="active-route">
            <span className={`status-dot ${bootstrap.runtime.running ? "online" : ""}`} />
            <div>
              <strong>{props.activeProfile?.name ?? "官方 OpenAI"}</strong>
              <span>{bootstrap.runtime.codexHome}</span>
            </div>
          </div>
          <button className="button primary-action" disabled={busy !== null} onClick={props.onPrepareSwitch} type="button">
            {busy === "preview" ? <LoaderCircle className="spin" size={17} /> : <Play size={17} />}
            切换并重启 Codex
          </button>
        </div>
      </section>

      <aside className="inspector-panel">
        <div className="inspector-section">
          <span className="eyebrow">RUNTIME</span>
          <h3>Codex 状态</h3>
          <dl className="runtime-list">
            <div><dt>安装</dt><dd>{bootstrap.runtime.installed ? "已检测" : "未检测"}</dd></div>
            <div><dt>进程</dt><dd className={bootstrap.runtime.running ? "good" : ""}>{bootstrap.runtime.running ? "运行中" : "未运行"}</dd></div>
            <div><dt>版本</dt><dd>{bootstrap.runtime.version ?? "未知"}</dd></div>
            <div><dt>Provider</dt><dd>{bootstrap.runtime.activeProviderId ?? "openai"}</dd></div>
          </dl>
        </div>

        <div className="inspector-section">
          <span className="eyebrow">SAFETY</span>
          <h3>切换保护</h3>
          <ul className="check-list">
            <li><Check size={15} />配置与认证自动备份</li>
            <li><Check size={15} />JSONL 与 SQLite 同步迁移</li>
            <li><Check size={15} />失败自动回滚</li>
            <li><Check size={15} />API Key 日志脱敏</li>
          </ul>
        </div>

        <div className="inspector-section grow">
          <span className="eyebrow">LAST TRANSACTION</span>
          <h3>最近执行</h3>
          {props.lastResult ? (
            <div className="step-list">
              {props.lastResult.steps.map((item) => (
                <div className="step-row" key={`${item.phase}-${item.message}`}>
                  <span className={`step-icon ${item.status}`}>{item.status === "success" ? <Check size={12} /> : item.status === "failure" ? <X size={12} /> : "–"}</span>
                  <div><strong>{phaseLabel(item.phase)}</strong><span>{item.message}</span></div>
                </div>
              ))}
            </div>
          ) : (
            <div className="quiet-empty">本次启动尚未执行切换。</div>
          )}
        </div>
      </aside>
    </div>
  );
}

function BackupView({ bootstrap, onRestore, onRefresh }: { bootstrap: SwitcherBootstrap; onRestore: (id: string) => void; onRefresh: () => void }) {
  return (
    <div className="single-view">
      <ViewHeader eyebrow="RECOVERY" title="备份与恢复" description="每次切换前都会保存配置、认证、模型目录、会话和状态数据库。" actions={<>
        <button className="button secondary" onClick={() => void switcherApi.openBackupDirectory()} type="button"><FolderOpen size={16} />打开目录</button>
        <IconButton label="刷新" onClick={onRefresh}><RefreshCw size={16} /></IconButton>
      </>} />
      <div className="table-shell">
        <div className="backup-head"><span>创建时间</span><span>类型</span><span>Provider</span><span>文件</span><span>状态</span><span /></div>
        {bootstrap.backups.map((backup) => (
          <div className="backup-row" key={backup.id}>
            <span>{formatDate(backup.createdAtUtc)}</span>
            <span>{backup.kind}</span>
            <span>{backup.profileName ?? "官方配置"}</span>
            <span>{backup.fileCount}</span>
            <span className={`state-label ${backup.status}`}>{backup.status}</span>
            <button className="icon-button" aria-label="恢复备份" title="恢复备份" onClick={() => onRestore(backup.id)} type="button"><ArchiveRestore size={16} /></button>
          </div>
        ))}
        {bootstrap.backups.length === 0 && <div className="table-empty">还没有备份记录。</div>}
      </div>
    </div>
  );
}

function LogView({ logs }: { logs: SwitcherLog[] }) {
  return (
    <div className="single-view">
      <ViewHeader eyebrow="AUDIT TRAIL" title="运行日志" description="敏感字段和 Token 在写入前会自动脱敏。" actions={<button className="button secondary" onClick={() => void switcherApi.openLogDirectory()} type="button"><FolderOpen size={16} />打开目录</button>} />
      <div className="log-stream">
        {logs.map((log, index) => (
          <div className="log-row" key={`${log.timestamp}-${index}`}>
            <span className={`log-level ${log.level}`}>{log.level}</span>
            <time>{formatDate(log.timestamp)}</time>
            <strong>{log.action}</strong>
            <span>{log.message}</span>
          </div>
        ))}
        {logs.length === 0 && <div className="table-empty">暂无日志。</div>}
      </div>
    </div>
  );
}

function SettingsView({ bootstrap, onSaved }: { bootstrap: SwitcherBootstrap; onSaved: (data: SwitcherBootstrap) => void }) {
  const [settings, setSettings] = useState(bootstrap.settings);
  const [saving, setSaving] = useState(false);
  return (
    <div className="single-view settings-view">
      <ViewHeader eyebrow="PREFERENCES" title="设置" description="控制 CODEX_HOME 和本地备份保留策略。全部本地对话始终受到保护。" actions={<button className="button" disabled={saving} onClick={() => {
        setSaving(true);
        void switcherApi.saveSettings(settings).then((data) => {
          onSaved(data);
          toast.success("设置已保存");
        }).catch((error) => toast.error(errorText(error))).finally(() => setSaving(false));
      }} type="button">{saving ? <LoaderCircle className="spin" size={16} /> : <Save size={16} />}保存设置</button>} />
      <div className="settings-form">
        <Field label="CODEX_HOME 覆盖路径" hint={`留空使用环境变量或默认路径：${bootstrap.runtime.codexHome}`}>
          <input value={settings.codexHomeOverride ?? ""} onChange={(event) => setSettings({ ...settings, codexHomeOverride: event.target.value || null })} placeholder="%USERPROFILE%\.codex" />
        </Field>
        <Field label="保留备份数量" hint="超过后自动删除最旧备份">
          <div className="number-field"><input min={1} max={50} type="number" value={settings.backupRetention} onChange={(event) => setSettings({ ...settings, backupRetention: Number(event.target.value) })} /><span>份</span></div>
        </Field>
        <Field label="新 Provider 默认行为">
          <label className="toggle-row">
            <input checked={settings.injectModelsDefault} onChange={(event) => setSettings({ ...settings, injectModelsDefault: event.target.checked })} type="checkbox" />
            <span className="toggle" />
            <span><strong>默认启用模型菜单注入</strong><small>仅影响之后新建的 Provider</small></span>
          </label>
        </Field>
        <div className="settings-paths">
          <button className="path-button" onClick={() => void switcherApi.openDataDirectory()} type="button"><PanelLeft size={17} /><span><strong>应用数据</strong><small>Provider 数据库与事务检查点</small></span><ChevronRight size={16} /></button>
          <button className="path-button" onClick={() => void switcherApi.openBackupDirectory()} type="button"><DatabaseBackup size={17} /><span><strong>备份目录</strong><small>配置、会话和 SQLite 快照</small></span><ChevronRight size={16} /></button>
        </div>
      </div>
    </div>
  );
}

function ModelRow({ model, onChange, onRemove }: { model: ModelEntry; onChange: (model: ModelEntry) => void; onRemove: () => void }) {
  return (
    <div className="model-row">
      <input value={model.id} onChange={(event) => onChange({ ...model, id: event.target.value })} placeholder="model-id" />
      <input value={model.displayName} onChange={(event) => onChange({ ...model, displayName: event.target.value })} placeholder="显示名称" />
      <input min={1} type="number" value={model.contextWindow ?? ""} onChange={(event) => onChange({ ...model, contextWindow: event.target.value ? Number(event.target.value) : null })} placeholder="128000" />
      <IconButton danger label="删除模型" onClick={onRemove}><X size={15} /></IconButton>
    </div>
  );
}

function Titlebar({ version }: { version: string }) {
  const appWindow = getCurrentWindow();
  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar-title" data-tauri-drag-region>
        <SquareTerminal size={15} />
        <span>Codex API Switcher</span>
        <small>v{version}</small>
      </div>
      <div className="window-actions">
        <button aria-label="最小化" onClick={() => void appWindow.minimize()} type="button"><Minimize2 size={15} /></button>
        <button aria-label="最大化" onClick={() => void appWindow.toggleMaximize()} type="button"><Maximize2 size={14} /></button>
        <button className="close" aria-label="关闭" onClick={() => void appWindow.close()} type="button"><X size={16} /></button>
      </div>
    </header>
  );
}

function NavButton({ active, icon, label, onClick }: { active: boolean; icon: React.ReactNode; label: string; onClick: () => void }) {
  return <button className={active ? "active" : ""} onClick={onClick} type="button">{icon}<span>{label}</span></button>;
}

function IconButton({ children, label, onClick, danger = false }: { children: React.ReactNode; label: string; onClick: () => void; danger?: boolean }) {
  return <button className={`icon-button ${danger ? "danger" : ""}`} aria-label={label} title={label} onClick={onClick} type="button">{children}</button>;
}

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return <label className="field"><span className="field-label">{label}</span>{children}{hint && <small>{hint}</small>}</label>;
}

function ViewHeader({ eyebrow, title, description, actions }: { eyebrow: string; title: string; description: string; actions?: React.ReactNode }) {
  return <div className="view-header"><div><span className="eyebrow">{eyebrow}</span><h1>{title}</h1><p>{description}</p></div><div className="toolbar-actions">{actions}</div></div>;
}

function PreviewDialog({ preview, busy, onCancel, onConfirm }: { preview: SwitchPreview; busy: boolean; onCancel: () => void; onConfirm: () => void }) {
  return (
    <div className="modal-backdrop">
      <div className="modal">
        <div className="modal-icon"><RotateCcw size={20} /></div>
        <h2>切换到 {preview.profileName}</h2>
        <p>应用将完整备份 Codex 数据，同步全部活动与归档对话到新 Provider，然后重启 Codex。</p>
        <dl className="preview-grid">
          <div><dt>Provider ID</dt><dd>{preview.providerId}</dd></div>
          <div><dt>CODEX_HOME</dt><dd>{preview.codexHome}</dd></div>
          <div><dt>模型注入</dt><dd>{preview.willInjectModels ? "启用" : "关闭"}</dd></div>
          <div><dt>当前进程</dt><dd>{preview.codexRunning ? "将关闭并重启" : "切换后启动"}</dd></div>
        </dl>
        {preview.warnings.length > 0 && <div className="warning-box"><CircleAlert size={17} /><div>{preview.warnings.map((warning) => <span key={warning}>{warning}</span>)}</div></div>}
        <div className="modal-actions">
          <button className="button tertiary" disabled={busy} onClick={onCancel} type="button">取消</button>
          <button className="button" disabled={busy} onClick={onConfirm} type="button">{busy ? <LoaderCircle className="spin" size={16} /> : <Play size={16} />}确认切换</button>
        </div>
      </div>
    </div>
  );
}

function ConfirmDialog({ title, body, confirmLabel, danger, onCancel, onConfirm }: { title: string; body: string; confirmLabel: string; danger?: boolean; onCancel: () => void; onConfirm: () => void }) {
  return (
    <div className="modal-backdrop">
      <div className="modal compact">
        <div className={`modal-icon ${danger ? "danger" : ""}`}><CircleAlert size={20} /></div>
        <h2>{title}</h2><p>{body}</p>
        <div className="modal-actions"><button className="button tertiary" onClick={onCancel} type="button">取消</button><button className={`button ${danger ? "danger-button" : ""}`} onClick={onConfirm} type="button">{confirmLabel}</button></div>
      </div>
    </div>
  );
}

function errorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  const failure = error as ApiFailure;
  return [failure?.message, failure?.detail].filter(Boolean).join("：") || "操作失败";
}

function formatDate(value: string): string {
  return new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit" }).format(new Date(value));
}

function phaseLabel(phase: string): string {
  const labels: Record<string, string> = {
    preflight: "预检",
    closeCodex: "关闭 Codex",
    backup: "创建备份",
    writeConfig: "写入配置",
    migrateSessions: "迁移会话",
    restartCodex: "重启 Codex",
    injectModels: "注入模型",
    restoreOfficial: "恢复官方",
  };
  return labels[phase] ?? phase;
}
