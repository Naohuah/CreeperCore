import { useEffect, useRef, useState } from "react";
import {
  Check,
  ImagePlus,
  Loader2,
  Palette,
  RotateCcw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { SkinBody } from "./SkinBody";
import { Modal } from "./Modal";
import { useAppStore } from "../store";
import { api } from "../lib/bridge";
import { cx, errorMessage } from "../lib/format";
import {
  readCape,
  readLocalSkin,
  readOnlineSkin,
  removeLocalSkin,
  writeCape,
  writeLocalSkin,
  writeOnlineSkin,
} from "../lib/skins";
import { notifySkinChanged } from "../lib/useAccountSkin";
import {
  addSkinToLibrary,
  readSkinLibrary,
  removeSkinFromLibrary,
  type LibrarySkin,
} from "../lib/skinLibrary";
import type { AccountProfile, MsaProfileInfo } from "../lib/types";

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
    image.onload = () => resolve({ width: image.width, height: image.height });
    image.onerror = () => reject(new Error("皮肤图片无法解析"));
    image.src = dataUrl;
  });
}

function validSkinSize(width: number, height: number): boolean {
  return (width === 64 && height === 64) || (width === 128 && height === 128);
}

interface SkinDialogProps {
  open: boolean;
  account: AccountProfile;
  onClose: () => void;
}

export function SkinDialog({ open, account, onClose }: SkinDialogProps) {
  const toast = useAppStore((s) => s.toast);
  const isMicrosoft = account.kind === "msa";
  const [tab, setTab] = useState<"skin" | "library" | "cape">("skin");
  const [profile, setProfile] = useState<MsaProfileInfo | null>(null);
  const [skin, setSkin] = useState<string | null>(null);
  const [cape, setCapeImage] = useState<string | null>(null);
  const [library, setLibrary] = useState<LibrarySkin[]>([]);
  const [librarySelection, setLibrarySelection] = useState<string | null>(null);
  const [variant, setVariant] = useState("classic");
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const libraryFileRef = useRef<HTMLInputElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewerRef = useRef<import("skinview3d").SkinViewer | null>(null);
  const librarySkin = library.find((item) => item.id === librarySelection);
  const displaySkin = librarySkin?.dataUrl ?? skin;

  useEffect(() => {
    if (!open) return;
    setTab("skin");
    setError(null);
    setLibrary(readSkinLibrary());
    setLibrarySelection(null);
    const local = readLocalSkin(account);
    const online = readOnlineSkin(account);
    setSkin(local ?? online);
    setCapeImage(readCape(account));
    if (!isMicrosoft) return;
    let active = true;
    setLoading(true);
    void api
      .msaProfile(account.id)
      .then((result) => {
        if (!active) return;
        setProfile(result);
        if (result.skinDataUrl) {
          writeOnlineSkin(account, result.skinDataUrl);
          if (!local) setSkin(result.skinDataUrl);
        }
        if (result.skinVariant) setVariant(result.skinVariant);
        const activeCape = result.capes.find((item) => item.active);
        writeCape(account, activeCape?.dataUrl ?? null);
        setCapeImage(activeCape?.dataUrl ?? null);
      })
      .catch((err) => {
        if (active) setError(errorMessage(err));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [open, account, isMicrosoft]);

  useEffect(() => {
    if (!open) return;
    let disposed = false;
    void import("skinview3d").then(({ IdleAnimation, SkinViewer }) => {
      if (disposed || !canvasRef.current) return;
      const viewer = new SkinViewer({
        canvas: canvasRef.current,
        width: 240,
        height: 280,
        model: "auto-detect",
        enableControls: true,
        zoom: 0.9,
      });
      viewer.animation = new IdleAnimation();
      viewer.autoRotate = false;
      viewerRef.current = viewer;
      if (displaySkin) void viewer.loadSkin(displaySkin, { model: "auto-detect" });
      if (cape) void viewer.loadCape(cape, { backEquipment: "cape" });
    });
    return () => {
      disposed = true;
      viewerRef.current?.dispose();
      viewerRef.current = null;
    };
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const viewer = viewerRef.current;
    if (!viewer || !open) return;
    if (displaySkin) {
      void viewer.loadSkin(displaySkin, { model: "auto-detect" }).catch(() => {
        setError("皮肤图片无法加载");
      });
    } else {
      viewer.resetSkin();
    }
    if (cape) {
      void viewer.loadCape(cape, { backEquipment: "cape" }).catch(() => {
        setCapeImage(null);
      });
    } else {
      viewer.resetCape();
    }
  }, [displaySkin, cape, open]);

  if (!open) return null;

  const applyProfile = (result: MsaProfileInfo) => {
    setProfile(result);
    if (result.skinDataUrl) {
      writeOnlineSkin(account, result.skinDataUrl);
      setSkin(result.skinDataUrl);
    }
    if (result.skinVariant) setVariant(result.skinVariant);
    const active = result.capes.find((item) => item.active);
    writeCape(account, active?.dataUrl ?? null);
    setCapeImage(active?.dataUrl ?? null);
    notifySkinChanged();
  };

  const importSkin = async () => {
    const file = fileRef.current?.files?.[0];
    if (!file) return;
    setBusy("skin-upload");
    setError(null);
    try {
      if (!file.name.toLowerCase().endsWith(".png") && file.type !== "image/png") {
        throw new Error("请选择 PNG 皮肤文件");
      }
      if (file.size > MAX_SKIN_BYTES) throw new Error("皮肤文件不能超过 1.5 MB");
      const dataUrl = await readFileAsDataUrl(file);
      const size = await readImageSize(dataUrl);
      if (!validSkinSize(size.width, size.height)) {
        throw new Error("官方皮肤需要 64×64 或 128×128 PNG");
      }
      setLibrary(
        addSkinToLibrary(
          file.name.replace(/\.[^.]+$/, ""),
          dataUrl,
          variant === "slim" ? "slim" : "classic",
        ),
      );
      if (isMicrosoft) {
        const result = await api.msaUploadSkin(account.id, dataUrl, variant);
        removeLocalSkin(account);
        void api.deleteLocalSkin(account.id).catch(() => {});
        applyProfile(result);
        toast("success", "Microsoft 皮肤已更新", `模型 ${variant}`);
      } else {
        writeLocalSkin(account, dataUrl);
        void api.saveLocalSkin(account.id, dataUrl).catch(() => {});
        setSkin(dataUrl);
        notifySkinChanged();
        toast("success", "本地皮肤已更新", "下次启动离线版本时应用");
      }
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
      if (fileRef.current) fileRef.current.value = "";
    }
  };

  const resetSkin = async () => {
    if (!isMicrosoft) return;
    setBusy("skin-reset");
    setError(null);
    try {
      const result = await api.msaDeleteSkin(account.id);
      applyProfile(result);
      if (!result.skinDataUrl) {
        setSkin(null);
        writeOnlineSkin(account, null);
      }
      removeLocalSkin(account);
      void api.deleteLocalSkin(account.id).catch(() => {});
      toast("success", "已恢复默认皮肤");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const importLibrarySkin = async () => {
    const file = libraryFileRef.current?.files?.[0];
    if (!file) return;
    setBusy("library-import");
    setError(null);
    try {
      if (!file.name.toLowerCase().endsWith(".png") && file.type !== "image/png") {
        throw new Error("请选择 PNG 皮肤文件");
      }
      if (file.size > MAX_SKIN_BYTES) throw new Error("皮肤文件不能超过 1.5 MB");
      const dataUrl = await readFileAsDataUrl(file);
      const size = await readImageSize(dataUrl);
      if (!validSkinSize(size.width, size.height)) {
        throw new Error("皮肤需要 64×64 或 128×128 PNG");
      }
      setLibrary(
        addSkinToLibrary(
          file.name.replace(/\.[^.]+$/, ""),
          dataUrl,
          variant === "slim" ? "slim" : "classic",
        ),
      );
      toast("success", "已加入皮肤库");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
      if (libraryFileRef.current) libraryFileRef.current.value = "";
    }
  };

  const applyLibrarySkin = async (item: LibrarySkin) => {
    setBusy(item.id);
    setError(null);
    try {
      if (isMicrosoft) {
        applyProfile(
          await api.msaUploadSkin(account.id, item.dataUrl, item.variant),
        );
        removeLocalSkin(account);
        void api.deleteLocalSkin(account.id).catch(() => {});
        toast("success", "已上传到 Microsoft 账户", item.name);
      } else {
        writeLocalSkin(account, item.dataUrl);
        void api.saveLocalSkin(account.id, item.dataUrl).catch(() => {});
        setSkin(item.dataUrl);
        notifySkinChanged();
        toast("success", "已应用皮肤库皮肤", item.name);
      }
      setLibrarySelection(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const deleteLibrarySkin = (item: LibrarySkin) => {
    setLibrary(removeSkinFromLibrary(item.id));
    if (librarySelection === item.id) setLibrarySelection(null);
    toast("success", "已从皮肤库移除", item.name);
  };

  const changeCape = async (capeId: string | null) => {
    setBusy(capeId ?? "cape-hide");
    setError(null);
    try {
      applyProfile(await api.msaSetCape(account.id, capeId));
      toast("success", capeId ? "披风已切换" : "披风已隐藏");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="皮肤与披风"
      description={
        isMicrosoft
          ? `${account.username} · Microsoft 账户`
          : `${account.username} · 离线账户`
      }
      size="lg"
      className="skin-dialog"
    >
      <div className="skin-dialog-layout">
        <div className="skin-dialog-preview">
          <canvas
            ref={canvasRef}
            className="skin-dialog-canvas"
            aria-label="皮肤与披风预览"
          />
          {loading && (
            <span className="skin-dialog-loading">
              <Loader2 size={14} className="spin" />
              读取正版档案
            </span>
          )}
        </div>

        <div className="skin-dialog-panel">
          <div className="skin-dialog-tabs">
            <button
              type="button"
              className={cx(tab === "skin" && "is-active")}
              onClick={() => {
                setTab("skin");
                setLibrarySelection(null);
              }}
            >
              <Palette size={14} />
              皮肤
            </button>
            <button
              type="button"
              className={cx(tab === "library" && "is-active")}
              onClick={() => {
                setTab("library");
                setLibrarySelection(null);
              }}
            >
              <ImagePlus size={14} />
              皮肤库
              {library.length > 0 && (
                <span className="skin-tab-count">{library.length}</span>
              )}
            </button>
            <button
              type="button"
              className={cx(tab === "cape" && "is-active")}
              onClick={() => {
                setTab("cape");
                setLibrarySelection(null);
              }}
            >
              <ShieldCheck size={14} />
              披风
              {profile && profile.capes.length > 0 && (
                <span className="skin-tab-count">{profile.capes.length}</span>
              )}
            </button>
          </div>

          {tab === "skin" ? (
            <div className="skin-dialog-section">
              <div className="skin-dialog-heading">
                <strong>{isMicrosoft ? "Microsoft 皮肤" : "本地皮肤"}</strong>
                <span>
                  {isMicrosoft
                    ? "上传后会同步到正版账户"
                    : "离线启动时写入 CreeperCore Skin.zip"}
                </span>
              </div>
              {isMicrosoft && (
                <label className="skin-model-row">
                  <span>模型</span>
                  <select
                    value={variant}
                    onChange={(event) => setVariant(event.target.value)}
                  >
                    <option value="classic">宽臂 Classic</option>
                    <option value="slim">细臂 Slim</option>
                  </select>
                </label>
              )}
              <input
                ref={fileRef}
                type="file"
                accept="image/png"
                hidden
                onChange={() => void importSkin()}
              />
              <div className="skin-dialog-actions">
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={busy !== null}
                  onClick={() => fileRef.current?.click()}
                >
                  {busy === "skin-upload" ? (
                    <Loader2 size={14} className="spin" />
                  ) : (
                    <ImagePlus size={14} />
                  )}
                  {isMicrosoft ? "上传皮肤" : "导入本地皮肤"}
                </button>
                {isMicrosoft && (
                  <button
                    type="button"
                    className="btn btn-ghost"
                    disabled={busy !== null}
                    onClick={() => void resetSkin()}
                  >
                    {busy === "skin-reset" ? (
                      <Loader2 size={14} className="spin" />
                    ) : (
                      <RotateCcw size={14} />
                    )}
                    恢复默认
                  </button>
                )}
              </div>
              <p className="skin-dialog-note">
                PNG 64×64 或 128×128。官方账户上传后，其他玩家也能看到。
              </p>
            </div>
          ) : tab === "library" ? (
            <div className="skin-dialog-section">
              <div className="skin-dialog-heading">
                <strong>皮肤库</strong>
                <span>本机已保存 {library.length} / 24 张皮肤</span>
              </div>
              <div className="skin-dialog-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  disabled={busy !== null}
                  onClick={() => libraryFileRef.current?.click()}
                >
                  {busy === "library-import" ? (
                    <Loader2 size={14} className="spin" />
                  ) : (
                    <ImagePlus size={14} />
                  )}
                  导入到皮肤库
                </button>
              </div>
              <input
                ref={libraryFileRef}
                type="file"
                accept="image/png"
                hidden
                onChange={() => void importLibrarySkin()}
              />
              {library.length === 0 ? (
                <div className="skin-dialog-empty">
                  <ImagePlus size={24} />
                  <strong>皮肤库是空的</strong>
                  <span>导入过的皮肤会自动记录在这里，离线/正版账户都能复用。</span>
                </div>
              ) : (
                <div className="skin-library-grid">
                  {library.map((item) => (
                    <div
                      className={cx(
                        "skin-library-card",
                        librarySelection === item.id && "is-selected",
                      )}
                      key={item.id}
                    >
                      <button
                        type="button"
                        className="skin-library-preview"
                        onClick={() => setLibrarySelection(item.id)}
                        title="预览"
                      >
                        <SkinBody
                          skin={item.dataUrl}
                          label={item.name}
                          slim={item.variant === "slim"}
                          size={42}
                        />
                      </button>
                      <div className="skin-library-copy">
                        <strong>{item.name}</strong>
                        <small>
                          {item.variant === "slim" ? "细臂" : "宽臂"}
                        </small>
                      </div>
                      <div className="skin-library-actions">
                        <button
                          type="button"
                          className="text-button"
                          disabled={busy !== null}
                          onClick={() => void applyLibrarySkin(item)}
                        >
                          {isMicrosoft ? "上传" : "应用"}
                        </button>
                        <button
                          type="button"
                          className="icon-button subtle"
                          title="从皮肤库移除"
                          aria-label={`从皮肤库移除 ${item.name}`}
                          onClick={() => deleteLibrarySkin(item)}
                        >
                          <Trash2 size={12} />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
              <p className="skin-dialog-note">
                皮肤库保存在本机；正版账户点“上传”会替换当前公共皮肤。
              </p>
            </div>
          ) : (
            <div className="skin-dialog-section">
              {isMicrosoft ? (
                profile && profile.capes.length > 0 ? (
                  <>
                    <div className="skin-dialog-heading">
                      <strong>可用披风</strong>
                      <span>点击切换，只对当前账户生效</span>
                    </div>
                    <div className="cape-grid">
                      {profile.capes.map((item) => (
                        <button
                          type="button"
                          key={item.id}
                          className={cx("cape-card", item.active && "is-active")}
                          disabled={busy !== null}
                          onClick={() => void changeCape(item.id)}
                        >
                          <span className="cape-preview">
                            {item.dataUrl ? (
                              <img src={item.dataUrl} alt="" />
                            ) : (
                              <ShieldCheck size={20} />
                            )}
                            {item.active && (
                              <span className="cape-active">
                                <Check size={12} />
                              </span>
                            )}
                          </span>
                          <strong>{item.alias}</strong>
                        </button>
                      ))}
                    </div>
                    {profile.activeCapeId && (
                      <button
                        type="button"
                        className="btn btn-ghost skin-hide-cape"
                        disabled={busy !== null}
                        onClick={() => void changeCape(null)}
                      >
                        <RotateCcw size={14} />
                        隐藏披风
                      </button>
                    )}
                  </>
                ) : (
                  <div className="skin-dialog-empty">
                    <ShieldCheck size={24} />
                    <strong>没有可用披风</strong>
                    <span>披风由 Mojang 发放，启动器不能上传或购买。</span>
                  </div>
                )
              ) : (
                <div className="skin-dialog-empty">
                  <ShieldCheck size={24} />
                  <strong>离线账户没有披风</strong>
                  <span>披风绑定 Microsoft 账户；离线模式只能使用本地皮肤。</span>
                </div>
              )}
              <p className="skin-dialog-note">
                披风由 Mojang 发放，不能通过启动器上传；这里只切换账户已有的披风。
              </p>
            </div>
          )}

          {error && <p className="form-error">{error}</p>}
        </div>
      </div>
    </Modal>
  );
}
