import { useEffect, useState } from "react";
import { Modal } from "./Modal";
import { SkinHead } from "./SkinHead";
import { useAppStore } from "../store";
import { api } from "../lib/bridge";
import { accountKindLabel, errorMessage } from "../lib/format";
import { removeLocalSkin, writeCape, writeOnlineSkin } from "../lib/skins";
import {
  accountSkin,
  notifySkinChanged,
  useAccountSkin,
} from "../lib/useAccountSkin";
import { SkinDialog } from "./SkinDialog";
import {
  Check,
  Copy,
  ExternalLink,
  Globe2,
  Link2,
  Loader2,
  Palette,
  Plus,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import type { AccountProfile } from "../lib/types";

export function AccountDialog() {
  const open = useAppStore((s) => s.accountDialogOpen);
  const setOpen = useAppStore((s) => s.setAccountDialogOpen);
  const boot = useAppStore((s) => s.boot);
  const refresh = useAppStore((s) => s.refresh);
  const toast = useAppStore((s) => s.toast);
  const askConfirm = useAppStore((s) => s.askConfirm);
  const [tab, setTab] = useState<"msa" | "offline">("offline");
  const [username, setUsername] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [msaBusy, setMsaBusy] = useState(false);
  const [msaError, setMsaError] = useState<string | null>(null);
  const [skinOpen, setSkinOpen] = useState(false);

  const accounts = boot?.config.accounts ?? [];
  const activeId = boot?.config.activeAccountId;
  const active = accounts.find((item) => item.id === activeId);
  const msaAccounts = accounts.filter((item) => item.kind === "msa");
  const offlineAccounts = accounts.filter((item) => item.kind !== "msa");
  const activeMsa = active?.kind === "msa" ? active : msaAccounts[0];
  const activeOffline =
    active && active.kind !== "msa" ? active : offlineAccounts[0];
  const shown = tab === "msa" ? activeMsa : activeOffline;
  const items = tab === "msa" ? msaAccounts : offlineAccounts;
  const heroSkin = useAccountSkin(shown);

  useEffect(() => {
    if (!open) return;
    setTab(active?.kind === "msa" ? "msa" : "offline");
    setError(null);
    setMsaError(null);
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps

  const create = async () => {
    if (busy || username.trim().length < 3) return;
    setBusy(true);
    setError(null);
    try {
      await api.createAccount(username.trim());
      setUsername("");
      await refresh(true);
      toast("success", "离线账户已创建");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const select = async (id: string) => {
    if (id === activeId) return;
    setBusy(true);
    setError(null);
    try {
      await api.selectAccount(id);
      await refresh(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const startMsa = async () => {
    if (msaBusy) return;
    setMsaError(null);
    setMsaBusy(true);
    try {
      const result = await api.msaEmbeddedLogin();
      if (!result.config) {
        throw new Error("Microsoft 登录没有返回账户信息");
      }
      const accountId = result.config.activeAccountId;
      const profile = result.config.accounts.find((item) => item.id === accountId);
      if (profile) {
        writeOnlineSkin(profile, result.skinDataUrl ?? null);
        removeLocalSkin(profile);
        notifySkinChanged();
      }
      setMsaBusy(false);
      await refresh(true);
      setOpen(false);
      if (result.skinDataUrl) {
        toast("success", "Microsoft 登录成功", "已同步正版在线皮肤");
      } else {
        toast("warning", "Microsoft 登录成功", "该角色没有在线皮肤");
      }
    } catch (err) {
      const message = errorMessage(err);
      setMsaBusy(false);
      if (message.includes("登录窗口已关闭")) {
        setMsaError(null);
        toast("info", "已取消 Microsoft 登录");
        return;
      }
      setMsaError(message);
    }
  };

  const remove = async (profile: AccountProfile) => {
    const accepted = await askConfirm({
      title: `删除 ${profile.username}？`,
      description: "只会删除启动器中的身份记录，不会删除存档或世界。",
      confirmLabel: "删除账户",
      danger: true,
    });
    if (!accepted) return;
    setBusy(true);
    try {
      await api.deleteAccount(profile.id);
      removeLocalSkin(profile);
      writeOnlineSkin(profile, null);
      writeCape(profile, null);
      void api.deleteLocalSkin(profile.id).catch(() => {});
      await refresh(true);
      toast("success", "账户已删除");
    } catch (err) {
      toast("error", "无法删除账户", errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const closeDialog = () => {
    setMsaBusy(false);
    setMsaError(null);
    setError(null);
    setSkinOpen(false);
    setOpen(false);
  };

  const copyUuid = async () => {
    if (!shown) return;
    try {
      await navigator.clipboard.writeText(shown.uuid);
      toast("success", "UUID 已复制");
    } catch {
      toast("warning", "复制失败", "当前环境不允许访问剪贴板");
    }
  };

  const heroTitle = shown
    ? shown.username
    : tab === "msa"
      ? "未登录 Microsoft"
      : "未创建离线账户";
  const heroHint = shown
    ? accountKindLabel(shown.kind)
    : tab === "msa"
      ? "登录后同步正版档案与皮肤"
      : "输入名字创建离线档案";

  return (
    <Modal
      open={open}
      onClose={closeDialog}
      title="玩家档案"
      size="sm"
      className="account-modal"
    >
      <div className="account-dialog">
        <div className="account-tabs" role="tablist" aria-label="账户类型">
          <button
            type="button"
            role="tab"
            aria-selected={tab === "msa"}
            className={tab === "msa" ? "is-active" : undefined}
            onClick={() => setTab("msa")}
          >
            <ShieldCheck size={14} />
            正版
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "offline"}
            className={tab === "offline" ? "is-active" : undefined}
            onClick={() => setTab("offline")}
          >
            <Link2 size={14} />
            离线
          </button>
        </div>

        <div className="account-hero">
          <div className="account-hero-head">
            <SkinHead skin={heroSkin} size={64} label={heroTitle} />
          </div>
          <strong>{heroTitle}</strong>
          <span>{heroHint}</span>
          {shown && (
            <div className="account-hero-actions">
              <button
                type="button"
                className="icon-button subtle"
                title="皮肤与披风"
                aria-label="管理皮肤与披风"
                onClick={() => setSkinOpen(true)}
              >
                <Palette size={14} />
              </button>
              {tab === "msa" && (
                <button
                  type="button"
                  className="icon-button subtle"
                  title="重新登录"
                  aria-label="重新登录 Microsoft"
                  disabled={msaBusy}
                  onClick={() => void startMsa()}
                >
                  <RefreshCw size={14} />
                </button>
              )}
              <button
                type="button"
                className="icon-button subtle"
                title="删除账户"
                aria-label={`删除 ${shown.username}`}
                disabled={busy}
                onClick={() => void remove(shown)}
              >
                <Trash2 size={14} />
              </button>
            </div>
          )}
          {shown && (
            <div className="account-uuid-row">
              <code>{shown.uuid}</code>
              <button
                type="button"
                className="icon-button subtle"
                title="复制 UUID"
                aria-label="复制 UUID"
                onClick={() => void copyUuid()}
              >
                <Copy size={12} />
              </button>
            </div>
          )}
        </div>

        {tab === "msa" ? (
          <>
            {shown ? (
              <button
                type="button"
                className="text-button account-add-button"
                disabled={msaBusy}
                onClick={() => void startMsa()}
              >
                {msaBusy ? (
                  <Loader2 size={13} className="spin" />
                ) : (
                  <Plus size={13} />
                )}
                {msaBusy ? "等待 Microsoft 登录…" : "添加另一个 Microsoft 账户"}
              </button>
            ) : (
              <>
                <button
                  type="button"
                  className="btn btn-primary account-main-button"
                  disabled={msaBusy}
                  onClick={() => void startMsa()}
                >
                  {msaBusy ? (
                    <Loader2 size={15} className="spin" />
                  ) : (
                    <Globe2 size={15} />
                  )}
                  {msaBusy ? "等待 Microsoft 登录…" : "登录 Microsoft 账户"}
                </button>
                <p className="account-footnote">
                  在启动器内打开 Microsoft 官方页面，密码不会经过启动器。
                </p>
              </>
            )}
            {msaError && (
              <div className="account-error">
                <span>{msaError}</span>
                {(msaError.includes("Minecraft API") ||
                  msaError.includes("Invalid app registration")) && (
                  <button
                    type="button"
                    className="text-button"
                    onClick={() =>
                      void api.openExternal("https://aka.ms/mce-reviewappid")
                    }
                  >
                    <ExternalLink size={12} />
                    申请页
                  </button>
                )}
              </div>
            )}
          </>
        ) : (
          <>
            <div className="account-create-line">
              <input
                value={username}
                maxLength={16}
                placeholder="新离线账户名"
                aria-label="新离线账户名"
                onChange={(event) => {
                  setUsername(event.target.value.replace(/[^A-Za-z0-9_]/g, ""));
                  setError(null);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void create();
                }}
              />
              <button
                type="button"
                className="btn btn-secondary"
                disabled={busy || username.trim().length < 3}
                onClick={() => void create()}
              >
                <Plus size={14} />
                创建
              </button>
            </div>
            {error && <p className="form-error">{error}</p>}
            <p className="account-footnote">
              本地皮肤会在启动离线版本时应用到游戏。
            </p>
          </>
        )}

        {items.length > 0 && (
          <div className="account-mini-list">
            {items.map((profile) => (
              <button
                type="button"
                key={profile.id}
                className={profile.id === activeId ? "is-active" : undefined}
                disabled={busy}
                onClick={() => void select(profile.id)}
              >
                <SkinHead
                  skin={accountSkin(profile)}
                  size={24}
                  label={profile.username}
                />
                <span className="account-mini-copy">
                  <strong>{profile.username}</strong>
                  <small>{accountKindLabel(profile.kind)}</small>
                </span>
                {profile.id === activeId && <Check size={13} />}
              </button>
            ))}
          </div>
        )}
      </div>
      {shown && (
        <SkinDialog
          open={skinOpen}
          account={shown}
          onClose={() => setSkinOpen(false)}
        />
      )}
    </Modal>
  );
}
