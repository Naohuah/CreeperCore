import { useEffect, useRef, useState } from "react";
import { Loader2, Palette, RotateCcw, Upload } from "lucide-react";
import type { SkinViewer } from "skinview3d";
import { useAppStore } from "../store";
import { api } from "../lib/bridge";
import { errorMessage } from "../lib/format";
import { notifySkinChanged } from "../lib/useAccountSkin";
import type { AccountProfile } from "../lib/types";
import {
  readCape,
  readLocalSkin,
  readOnlineSkin,
  removeLocalSkin,
  writeCape,
  writeLocalSkin,
  writeOnlineSkin,
} from "../lib/skins";
import { CreeperMark } from "./CreeperMark";
import { SkinDialog } from "./SkinDialog";

const MAX_SKIN_BYTES = 1.5 * 1024 * 1024;

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error("无法读取文件"));
    reader.readAsDataURL(file);
  });
}

function readImageSize(dataUrl: string): Promise<{ width: number; height: number }> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve({ width: image.naturalWidth, height: image.naturalHeight });
    image.onerror = () => reject(new Error("无法解析 PNG"));
    image.src = dataUrl;
  });
}

function isSupportedSkin(width: number, height: number): boolean {
  return (
    (width === 64 && (height === 64 || height === 32)) ||
    (width === 128 && height === 128)
  );
}

export function SkinPreview({ account }: { account: AccountProfile }) {
  const toast = useAppStore((s) => s.toast);
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const viewerRef = useRef<SkinViewer | null>(null);
  const [localSkin, setLocalSkin] = useState<string | null>(null);
  const [onlineSkin, setOnlineSkin] = useState<string | null>(null);
  const [cape, setCape] = useState<string | null>(null);
  const [manageOpen, setManageOpen] = useState(false);
  const [ready, setReady] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const skin = localSkin ?? (account.kind === "msa" ? onlineSkin : null);
  const custom = localSkin !== null;

  useEffect(() => {
    const stored = readLocalSkin(account);
    setLocalSkin(stored);
    setOnlineSkin(readOnlineSkin(account));
    setCape(readCape(account));
    setError(null);
    if (stored) {
      void api.saveLocalSkin(account.id, stored).catch(() => {
        // Launch-time skin sync will report a real error if this persists.
      });
    }
    if (account.kind !== "msa") return;
    let active = true;
    void api
      .msaProfile(account.id)
      .then((profile) => {
        if (!active) return;
        if (profile.skinDataUrl) {
          writeOnlineSkin(account, profile.skinDataUrl);
          setOnlineSkin(profile.skinDataUrl);
        }
        const activeCape = profile.capes.find((item) => item.active);
        writeCape(account, activeCape?.dataUrl ?? null);
        setCape(activeCape?.dataUrl ?? null);
      })
      .catch(() => {
        // Keep the cached skin/cape if the profile request fails.
      });
    return () => {
      active = false;
    };
  }, [account]);

  useEffect(() => {
    let disposed = false;
    let observer: ResizeObserver | undefined;
    const host = hostRef.current;
    const canvas = canvasRef.current;
    if (!host || !canvas) return;

    setLoading(true);
    setError(null);
    (async () => {
      try {
        const { SkinViewer: Viewer, IdleAnimation } = await import("skinview3d");
        if (disposed) return;
        const viewer = new Viewer({
          canvas,
          width: Math.max(1, host.clientWidth),
          height: Math.max(1, host.clientHeight),
          model: "auto-detect",
          enableControls: true,
          zoom: 0.82,
        });
        // Default is a stationary idle pose. Rotation is user-driven via drag.
        viewer.autoRotate = false;
        viewer.animation = new IdleAnimation();
        viewer.animation.speed = 0.8;
        viewer.controls.enableZoom = false;
        viewer.controls.enablePan = false;
        viewer.controls.enableDamping = true;
        viewer.controls.dampingFactor = 0.08;
        viewer.cameraLight.intensity = 0.7;
        viewer.globalLight.intensity = 2.2;
        viewerRef.current = viewer;

        observer = new ResizeObserver(() => {
          if (disposed) return;
          const width = Math.max(1, host.clientWidth);
          const height = Math.max(1, host.clientHeight);
          viewer.setSize(width, height);
          viewer.adjustCameraDistance();
        });
        observer.observe(host);
        setReady(true);
        setLoading(false);
      } catch (err) {
        if (!disposed) {
          setError(err instanceof Error ? err.message : "3D 预览无法加载");
          setLoading(false);
        }
      }
    })();

    return () => {
      disposed = true;
      observer?.disconnect();
      viewerRef.current?.dispose();
      viewerRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (!ready) return;
    const viewer = viewerRef.current;
    if (!viewer) return;
    if (!skin) {
      viewer.resetSkin();
      setError(null);
      return;
    }
    viewer.loadSkin(skin, { model: "auto-detect" }).catch((err) => {
      setError(err instanceof Error ? err.message : "皮肤加载失败");
    });
    if (cape) {
      void viewer.loadCape(cape, { backEquipment: "cape" }).catch(() => {
        setCape(null);
      });
    } else {
      viewer.resetCape();
    }
    viewer.resetCameraPose();
  }, [skin, cape, ready]);

  const chooseSkin = () => fileRef.current?.click();

  const importSkin = async () => {
    const file = fileRef.current?.files?.[0];
    if (!file) return;
    try {
      if (!file.name.toLowerCase().endsWith(".png") && file.type !== "image/png") {
        throw new Error("请选择 PNG 皮肤文件");
      }
      if (file.size > MAX_SKIN_BYTES) {
        throw new Error("皮肤文件不能超过 1.5 MB");
      }
      const dataUrl = await readFileAsDataUrl(file);
      const size = await readImageSize(dataUrl);
      if (!isSupportedSkin(size.width, size.height)) {
        throw new Error("皮肤尺寸应为 64×64、64×32 或 128×128");
      }
      writeLocalSkin(account, dataUrl);
      setLocalSkin(dataUrl);
      notifySkinChanged();
      setError(null);
      await api.saveLocalSkin(account.id, dataUrl).catch((err) => {
        toast(
          "warning",
          "皮肤已保存到启动器",
          `写入离线皮肤文件失败：${errorMessage(err)}`,
        );
      });
      toast(
        "success",
        "启动器皮肤已更新",
        `${account.username} · ${size.width}×${size.height} · 离线启动时应用`,
      );
    } catch (err) {
      toast("error", "皮肤导入失败", err instanceof Error ? err.message : String(err));
    } finally {
      if (fileRef.current) fileRef.current.value = "";
    }
  };

  const resetSkin = () => {
    removeLocalSkin(account);
    setLocalSkin(null);
    notifySkinChanged();
    setError(null);
    void api.deleteLocalSkin(account.id).catch(() => {
      // The next launch will also clear the resource pack.
    });
    toast("success", account.kind === "msa" && onlineSkin ? "已恢复在线皮肤" : "已移除本地皮肤");
  };

  const resetLabel =
    account.kind === "msa" && onlineSkin ? "恢复在线皮肤" : "移除本地皮肤";

  return (
    <div className={`skin-preview ${custom ? "has-custom" : ""}`}>
      <div className="skin-preview-head">
        <span>皮肤</span>
        <div className="skin-preview-actions">
          <button
            type="button"
            className="icon-button subtle"
            title="管理皮肤与披风"
            aria-label="管理皮肤与披风"
            onClick={() => setManageOpen(true)}
          >
            <Palette size={12} />
          </button>
          {custom && (
            <button
              type="button"
              className="icon-button subtle"
              title={resetLabel}
              aria-label={resetLabel}
              onClick={resetSkin}
            >
              <RotateCcw size={12} />
            </button>
          )}
          <button
            type="button"
            className="icon-button subtle"
            title="导入本地皮肤"
            aria-label="导入本地皮肤"
            onClick={chooseSkin}
          >
            <Upload size={12} />
          </button>
        </div>
      </div>
      <div className="skin-stage" ref={hostRef}>
        <canvas
          ref={canvasRef}
          className="skin-canvas"
          aria-label={`${account.username} 的 3D 皮肤预览，拖动旋转`}
        />
        {loading && (
          <div className="skin-overlay">
            <Loader2 size={15} className="spin" />
            <span>加载皮肤</span>
          </div>
        )}
        {!loading && !error && !skin && (
          <div className="skin-overlay skin-empty">
            <CreeperMark size={36} muted />
            <strong>未设置皮肤</strong>
            <span>
              {account.kind === "msa"
                ? "该角色没有可用的在线皮肤"
                : "导入皮肤，离线启动时应用到游戏"}
            </span>
            <button type="button" className="text-button" onClick={chooseSkin}>
              导入本地皮肤
            </button>
          </div>
        )}
        {error && !loading && (
          <div className="skin-overlay skin-error">
            <CreeperMark size={32} muted />
            <span>无法显示 3D 预览</span>
            <button
              type="button"
              className="text-button"
              onClick={() => window.location.reload()}
            >
              重试
            </button>
          </div>
        )}
      </div>
      <span className="skin-hint">
        {custom
          ? "拖动旋转 · 离线启动时应用"
          : skin
            ? "拖动旋转"
            : "支持 64×64 / 128×128 PNG"}
      </span>
      <input
        ref={fileRef}
        type="file"
        accept="image/png,.png"
        hidden
        onChange={() => void importSkin()}
      />
      <SkinDialog
        open={manageOpen}
        account={account}
        onClose={() => {
          setManageOpen(false);
          setLocalSkin(readLocalSkin(account));
          setOnlineSkin(readOnlineSkin(account));
          setCape(readCape(account));
        }}
      />
    </div>
  );
}
