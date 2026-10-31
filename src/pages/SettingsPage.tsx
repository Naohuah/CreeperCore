import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Check,
  FolderOpen,
  Image as ImageIcon,
  Save,
  RefreshCw,
  Settings2,
  UserRound,
  Wrench,
} from "lucide-react";
import { accountKindLabel } from "../lib/format";
import { ACCENTS, prepareBackground } from "../lib/appearance";
import { useAppStore } from "../store";
import { api, isTauri, pickDirectory, pickJavaFile } from "../lib/bridge";
import {
  cx,
  errorMessage,
  formatMemory,
  shortPath,
  versionTitle,
} from "../lib/format";
import { CreeperMark } from "../components/CreeperMark";
import { PageHeader, StatusPill, Toggle } from "../components/Ui";
import type { JavaInfo, SettingsUpdate } from "../lib/types";

interface SettingsForm {
  gameDir: string;
  javaPath: string;
  memoryMb: number;
  jvmArgs: string;
  width: number;
  height: number;
  fullscreen: boolean;
  downloadSource: string;
  communitySource: string;
  downloadConcurrency: number;
  isolateVersions: boolean;
  minimizeOnLaunch: boolean;
  autoBackupBeforeLaunch: boolean;
  showLogOnCrash: boolean;
}

export function SettingsPage() {
  const boot = useAppStore((s) => s.boot);
  const refresh = useAppStore((s) => s.refresh);
  const toast = useAppStore((s) => s.toast);
  const appearance = useAppStore((s) => s.appearance);
  const setAppearance = useAppStore((s) => s.setAppearance);
  const backgroundRef = useRef<HTMLInputElement>(null);
  const [backgroundBusy, setBackgroundBusy] = useState(false);
  const setAccountDialogOpen = useAppStore((s) => s.setAccountDialogOpen);
  const [form, setForm] = useState<SettingsForm | null>(null);
  const [javas, setJavas] = useState<JavaInfo[]>(boot?.javas ?? []);
  const [scanningJava, setScanningJava] = useState(false);
  const [requiredJava, setRequiredJava] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!boot) return;
    setJavas(boot.javas);
    setForm({
      gameDir: boot.config.gameDir,
      javaPath: boot.config.javaPath ?? "",
      memoryMb: boot.config.memoryMb,
      jvmArgs: boot.config.jvmArgs,
      width: boot.config.width,
      height: boot.config.height,
      fullscreen: boot.config.fullscreen,
      downloadSource: boot.config.downloadSource,
      communitySource: boot.config.communitySource,
      downloadConcurrency: boot.config.downloadConcurrency,
      isolateVersions: boot.config.isolateVersions,
      minimizeOnLaunch: boot.config.minimizeOnLaunch,
      autoBackupBeforeLaunch: boot.config.autoBackupBeforeLaunch,
      showLogOnCrash: boot.config.showLogOnCrash,
    });
    setDirty(false);
  }, [boot?.config, boot?.javas]);

  const update = <K extends keyof SettingsForm>(key: K, value: SettingsForm[K]) => {
    setForm((current) => (current ? { ...current, [key]: value } : current));
    setDirty(true);
    setError(null);
  };

  const activeAccount = boot?.config.accounts.find(
    (item) => item.id === boot.config.activeAccountId,
  );
  const selectedVersion = boot?.versions.find(
    (item) => item.id === boot.config.selectedVersion,
  );
  const memoryMax = useMemo(() => {
    const physical = boot?.system.totalMemoryMb ?? 16384;
    return Math.max(2048, Math.min(32768, Math.floor(physical * 0.8)));
  }, [boot?.system.totalMemoryMb]);

  const scanJava = useCallback(async () => {
    setScanningJava(true);
    try {
      const result = await api.scanJava();
      setJavas(result.javas);
      setRequiredJava(result.requiredMajor);
      toast("success", "扫描完成", `发现 ${result.javas.length} 个 Java 运行时`);
    } catch (err) {
      toast("error", "扫描 Java 失败", errorMessage(err));
    } finally {
      setScanningJava(false);
    }
  }, [toast]);

  const browseDirectory = async () => {
    if (!isTauri()) {
      toast("info", "浏览器预览模式", "目录选择器只在桌面版中可用");
      return;
    }
    const selected = await pickDirectory("选择游戏目录", form?.gameDir);
    if (selected) update("gameDir", selected);
  };

  const browseJava = async () => {
    if (!isTauri()) {
      toast("info", "浏览器预览模式", "文件选择器只在桌面版中可用");
      return;
    }
    const selected = await pickJavaFile();
    if (selected) update("javaPath", selected);
  };

  const importBackground = async (file: File | undefined) => {
    if (!file || backgroundBusy) return;
    setBackgroundBusy(true);
    try {
      const dataUrl = await prepareBackground(file);
      setAppearance({ background: dataUrl });
      toast("success", "启动页背景已更新");
    } catch (err) {
      toast("error", "无法使用这张背景", errorMessage(err));
    } finally {
      setBackgroundBusy(false);
      if (backgroundRef.current) backgroundRef.current.value = "";
    }
  };

  const save = async () => {
    if (!form || saving) return;
    if (form.memoryMb < 512) {
      setError("内存至少需要 512 MB");
      return;
    }
    setSaving(true);
    setError(null);
    const payload: SettingsUpdate = {
      ...form,
      javaPath: form.javaPath.trim() ? form.javaPath.trim() : null,
      closeAfterLaunch: false,
      backgroundAnimation: false,
    };
    try {
      await api.saveSettings(payload);
      await refresh(true);
      setDirty(false);
      toast("success", "设置已保存");
    } catch (err) {
      const message = errorMessage(err);
      setError(message);
      toast("error", "保存失败", message);
    } finally {
      setSaving(false);
    }
  };

  if (!boot || !form) return null;

  const selectedJava = javas.find(
    (java) => java.path.toLowerCase() === form.javaPath.toLowerCase(),
  );

  return (
    <div className="page settings-page">
      <PageHeader
        title="设置"
        description="配置账户、Java、游戏目录与启动参数。"
        actions={
          <button
            type="button"
            className={cx("btn", dirty ? "btn-primary" : "btn-secondary")}
            disabled={saving}
            onClick={() => void save()}
          >
            <Save size={15} />
            {saving ? "保存中…" : "保存设置"}
          </button>
        }
      />

      {error && (
        <div className="inline-alert error">
          <Wrench size={16} />
          <div>
            <strong>无法保存设置</strong>
            <span>{error}</span>
          </div>
        </div>
      )}

      <section className="panel settings-section">
        <div className="section-head">
          <h2>账户</h2>
          <button type="button" className="btn btn-sm btn-secondary" onClick={() => setAccountDialogOpen(true)}>
            <UserRound size={14} />
            管理账户
          </button>
        </div>
        <div className="settings-account">
          <CreeperMark size={34} />
          <div>
            <strong>{activeAccount?.username ?? "未选择账户"}</strong>
            <span>{activeAccount ? accountKindLabel(activeAccount.kind) : "未选择"}</span>
          </div>
          <StatusPill tone={activeAccount?.kind === "msa" ? "ok" : "info"}>
            {activeAccount?.kind === "msa" ? "Microsoft 登录" : "离线模式"}
          </StatusPill>
        </div>
      </section>

      <section className="panel settings-section">
        <div className="section-head">
          <h2>Java 运行时</h2>
          <div className="section-actions">
            <button type="button" className="btn btn-sm btn-ghost" onClick={() => void scanJava()} disabled={scanningJava}>
              <RefreshCw size={14} className={scanningJava ? "spin" : undefined} />
              重新扫描
            </button>
            <button type="button" className="btn btn-sm btn-ghost" onClick={() => void browseJava()}>
              <FolderOpen size={14} />
              手动选择
            </button>
          </div>
        </div>
        <div className="settings-row">
          <span className="settings-label">当前 Java</span>
          <div className="settings-control">
            <div className="select-wrap">
              <select
                value={form.javaPath}
                onChange={(event) => update("javaPath", event.target.value)}
              >
                {javas.length === 0 && <option value="">未找到 Java</option>}
                {form.javaPath &&
                  !javas.some((java) => java.path.toLowerCase() === form.javaPath.toLowerCase()) && (
                    <option value={form.javaPath}>自定义 Java</option>
                  )}
                {javas.map((java) => (
                  <option key={java.path} value={java.path}>
                    Java {java.major} · {java.vendor}
                  </option>
                ))}
              </select>
            </div>
            <span className="field-hint">
              {selectedJava
                ? `${selectedJava.path} · ${selectedJava.is64bit ? "64 位" : "32 位"}`
                : form.javaPath || "启动器会自动选择兼容的 Java"}
            </span>
          </div>
        </div>
        {requiredJava && (
          <p className="section-hint">当前版本需要 Java {requiredJava} 或更高。</p>
        )}
      </section>

      <section className="panel settings-section">
        <div className="section-head">
          <h2>游戏目录</h2>
          <button type="button" className="btn btn-sm btn-ghost" onClick={() => void browseDirectory()}>
            <FolderOpen size={14} />
            更改目录
          </button>
        </div>
        <div className="settings-row">
          <span className="settings-label">游戏目录</span>
          <div className="settings-control">
            <code className="path-text">{shortPath(form.gameDir, 86)}</code>
            <span className="field-hint">更改目录后会重新扫描版本，账户与设置不会丢失。</span>
          </div>
        </div>
        <div className="settings-row">
          <span className="settings-label">版本隔离</span>
          <div className="settings-control settings-control-inline">
            <div>
              <strong>每个版本使用独立目录</strong>
              <span className="field-hint">模组、存档和配置互不影响。</span>
            </div>
            <Toggle
              checked={form.isolateVersions}
              onChange={(value) => update("isolateVersions", value)}
              label="版本隔离"
            />
          </div>
        </div>
      </section>

      <section className="panel settings-section">
        <div className="section-head">
          <h2>外观</h2>
        </div>
        <div className="settings-row">
          <span className="settings-label">启动页背景</span>
          <div className="settings-control">
            <div className="appearance-background">
              <div className="appearance-background-preview">
                {appearance.background ? (
                  <img src={appearance.background} alt="启动页背景预览" />
                ) : (
                  <>
                    <ImageIcon size={18} />
                    <span>默认</span>
                  </>
                )}
              </div>
              <div className="appearance-background-actions">
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  disabled={backgroundBusy}
                  onClick={() => backgroundRef.current?.click()}
                >
                  {backgroundBusy ? "处理中…" : appearance.background ? "更换图片" : "选择图片"}
                </button>
                {appearance.background && (
                  <button
                    type="button"
                    className="text-button"
                    onClick={() => {
                      setAppearance({ background: null });
                      toast("success", "已恢复默认背景");
                    }}
                  >
                    移除
                  </button>
                )}
              </div>
            </div>
            <span className="field-hint">
              会压缩后保存在本机，仅用于启动页；图片不会上传。
            </span>
          </div>
        </div>
        <div className="settings-row">
          <span className="settings-label">背景暗化</span>
          <div className="settings-control settings-control-inline">
            <input
              type="range"
              min={0}
              max={80}
              step={5}
              value={Math.round(appearance.dim * 100)}
              onChange={(event) =>
                setAppearance({ dim: Number(event.target.value) / 100 })
              }
              aria-label="背景暗化"
            />
            <span className="field-hint">{Math.round(appearance.dim * 100)}%</span>
          </div>
        </div>
        <div className="settings-row">
          <span className="settings-label">强调色</span>
          <div className="settings-control">
            <div className="accent-row">
              {ACCENTS.map((preset) => (
                <button
                  type="button"
                  key={preset.key}
                  className={cx(
                    "accent-swatch",
                    appearance.accent === preset.key && "is-active",
                  )}
                  style={{ background: preset.color }}
                  title={preset.label}
                  aria-label={preset.label}
                  onClick={() => setAppearance({ accent: preset.key })}
                >
                  {appearance.accent === preset.key && <Check size={13} />}
                </button>
              ))}
            </div>
          </div>
        </div>
        <input
          ref={backgroundRef}
          type="file"
          accept="image/*"
          hidden
          onChange={(event) =>
            void importBackground(event.target.files?.[0])
          }
        />
      </section>

      <section className="panel settings-section">
        <div className="section-head">
          <h2>启动设置</h2>
          <span className="section-hint">
            当前版本{" "}
            {selectedVersion
              ? versionTitle(
                  selectedVersion.id,
                  selectedVersion.inheritsFrom,
                  selectedVersion.loader,
                )
              : "未选择"}
          </span>
        </div>
        <div className="settings-row">
          <span className="settings-label">内存</span>
          <div className="settings-control">
            <div className="memory-row">
              <input
                type="range"
                min={512}
                max={memoryMax}
                step={512}
                value={form.memoryMb}
                onChange={(event) => update("memoryMb", Number(event.target.value))}
                aria-label="内存分配"
              />
              <div className="memory-value">
                <input
                  type="number"
                  min={512}
                  max={memoryMax}
                  step={512}
                  value={form.memoryMb}
                  onChange={(event) =>
                    update(
                      "memoryMb",
                      Math.max(512, Math.min(memoryMax, Number(event.target.value) || 512)),
                    )
                  }
                />
                <span>MB</span>
              </div>
            </div>
            <span className="field-hint">
              物理内存 {formatMemory(boot.system.totalMemoryMb)} · 建议 4096–8192 MB
            </span>
          </div>
        </div>
        <div className="settings-row">
          <span className="settings-label">窗口尺寸</span>
          <div className="settings-control settings-control-inline">
            <div className="inline-fields">
              <input
                type="number"
                value={form.width}
                min={640}
                onChange={(event) => update("width", Number(event.target.value) || 854)}
              />
              <span>×</span>
              <input
                type="number"
                value={form.height}
                min={480}
                onChange={(event) => update("height", Number(event.target.value) || 480)}
              />
            </div>
            <div className="check-label">
              <Toggle
                checked={form.fullscreen}
                onChange={(value) => update("fullscreen", value)}
                label="全屏启动"
              />
              全屏启动
            </div>
          </div>
        </div>
        <div className="settings-row">
          <span className="settings-label">JVM 参数</span>
          <div className="settings-control">
            <textarea
              className="code-input"
              value={form.jvmArgs}
              onChange={(event) => update("jvmArgs", event.target.value)}
              placeholder="每行或空格分隔，例如 -XX:+UseG1GC"
              rows={3}
            />
            <span className="field-hint">留空使用默认参数。错误的参数会导致 Java 无法启动。</span>
          </div>
        </div>
        <div className="settings-row">
          <span className="settings-label">行为</span>
          <div className="settings-control settings-toggles">
            <div className="check-label">
              <Toggle
                checked={form.minimizeOnLaunch}
                onChange={(value) => update("minimizeOnLaunch", value)}
                label="启动后最小化"
              />
              启动游戏后最小化启动器；关闭启动器不会结束游戏
            </div>
            <div className="check-label">
              <Toggle
                checked={form.autoBackupBeforeLaunch}
                onChange={(value) => update("autoBackupBeforeLaunch", value)}
                label="启动前自动备份"
              />
              启动前自动备份存档
            </div>
            <div className="check-label">
              <Toggle
                checked={form.showLogOnCrash}
                onChange={(value) => update("showLogOnCrash", value)}
                label="崩溃时显示日志"
              />
              崩溃时显示日志面板
            </div>
          </div>
        </div>
      </section>

      <section className="panel settings-section">
        <div className="section-head">
          <h2>下载与缓存</h2>
        </div>
        <div className="settings-row">
          <span className="settings-label">下载源</span>
          <div className="settings-control">
            <div className="select-wrap settings-select">
              <select
                value={form.downloadSource}
                onChange={(event) => update("downloadSource", event.target.value)}
              >
                <option value="official">Mojang 官方</option>
                <option value="bmclapi">BMCLAPI 镜像</option>
              </select>
            </div>
            <span className="field-hint">官方源失败时会自动回退到镜像。</span>
          </div>
        </div>
        <div className="settings-row">
          <span className="settings-label">并行下载数</span>
          <div className="settings-control settings-control-inline">
            <input
              type="number"
              className="small-number"
              min={1}
              max={64}
              value={form.downloadConcurrency}
              onChange={(event) =>
                update(
                  "downloadConcurrency",
                  Math.max(1, Math.min(64, Number(event.target.value) || 1)),
                )
              }
            />
            <span className="field-hint">默认 16，建议 8–24。</span>
          </div>
        </div>
      </section>

      <section className="panel settings-section">
        <div className="section-head">
          <h2>社区资源</h2>
        </div>
        <div className="settings-row">
          <span className="settings-label">来源</span>
          <div className="settings-control">
            <div className="select-wrap settings-select">
              <select
                value={form.communitySource}
                onChange={(event) => update("communitySource", event.target.value)}
              >
                <option value="official">Modrinth 官方</option>
                <option value="mcim">MCIM 镜像</option>
              </select>
            </div>
            <span className="field-hint">
              镜像代理 Modrinth 的搜索与版本元数据；文件仍从 Modrinth CDN 下载。
            </span>
          </div>
        </div>
      </section>

      <section className="panel settings-section about-section">
        <div className="section-head">
          <h2>关于</h2>
        </div>
        <div className="about-row">
          <CreeperMark size={32} />
          <div>
            <strong>苦力怕启动器 · Creeper Core {boot.system.launcherVersion}</strong>
            <p>
              本地优先的 Minecraft 启动器。非 Mojang / Microsoft 官方产品，不附带游戏文件。
              使用正版账号支持 Minecraft。
            </p>
          </div>
          <span className="cell-muted">
            <Settings2 size={13} /> Rust + Tauri
          </span>
        </div>
        <p className="section-hint section-hint-bottom">
          存档、账户与日志都保存在本机。
        </p>
      </section>

      <div className="settings-footer">
        <span>{dirty ? "有未保存的更改" : "所有设置已保存"}</span>
        <button type="button" className="btn btn-primary" disabled={saving} onClick={() => void save()}>
          <Save size={15} />
          {saving ? "保存中…" : "保存设置"}
        </button>
      </div>
    </div>
  );
}
