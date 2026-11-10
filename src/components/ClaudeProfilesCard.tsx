import React, { useEffect, useState } from "react";
import {
  Users,
  Plus,
  Trash2,
  RefreshCw,
  Pin,
  Check,
  Fingerprint,
  RotateCw,
  X,
  Link2,
  AlertTriangle,
  Lock,
} from "lucide-react";
import { ClaudeProfile } from "../types";
import {
  bindIpToClaudeProfile,
  bindUpstreamToClaudeProfile,
  createClaudeProfile,
  deleteClaudeProfile,
  listClaudeProfiles,
  switchClaudeProfile,
} from "../utils/api";

interface ClaudeProfilesCardProps {
  currentIp: string;
  currentCountry: string;
  activeUpstreamKey?: string | null;
  onProfileSwitched?: (profileName: string, pinnedIp?: string | null) => void;
  onToast?: (
    title: string,
    desc?: string,
    type?: "success" | "warning" | "error" | "info"
  ) => void;
}

export const ClaudeProfilesCard: React.FC<ClaudeProfilesCardProps> = ({
  currentIp,
  currentCountry,
  activeUpstreamKey: _activeUpstreamKey,
  onProfileSwitched,
  onToast,
}) => {
  const [profiles, setProfiles] = useState<ClaudeProfile[]>([]);
  const [activeProfileId, setActiveProfileId] = useState<string>("default");
  const [loading, setLoading] = useState(false);
  const [switchingId, setSwitchingId] = useState<string | null>(null);
  const [bindingId, setBindingId] = useState<string | null>(null);
  const [showNewForm, setShowNewForm] = useState(false);
  const [newProfileName, setNewProfileName] = useState("");
  const [bindCurrentOnCreate, setBindCurrentOnCreate] = useState(true);
  const [bindingProxyId, setBindingProxyId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [guardNoticeProfileId, setGuardNoticeProfileId] = useState<string | null>(null);

  const handleProxyClick = (profileId: string) => {
    setGuardNoticeProfileId((prev) => (prev === profileId ? null : profileId));
    onToast?.(
      "Доступно в следующем обновлении",
      "Привязка отдельного прокси-ключа к профилю временно закрыта: сейчас идёт финальная разработка Telegram-бота автоматической выдачи проверенных Чистых IP. Это защищает шлюз Заставы от поломки из-за некорректных сторонних конфигов.",
      "warning"
    );
  };

  const fetchProfiles = async () => {
    setLoading(true);
    try {
      const res = await listClaudeProfiles();
      setProfiles(res.profiles || []);
      setActiveProfileId(res.active_profile_id || "default");
    } catch (err: any) {
      console.error("Failed to list Claude profiles:", err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchProfiles();
  }, []);

  const handleSwitch = async (profileId: string) => {
    if (profileId === activeProfileId || switchingId) return;
    setSwitchingId(profileId);
    try {
      const res = await switchClaudeProfile(profileId);
      setActiveProfileId(res.active_profile.id);
      await fetchProfiles();
      onToast?.(
        "Профиль Claude переключен",
        res.message || `Активен профиль: ${res.active_profile.name}`,
        "success"
      );
      onProfileSwitched?.(res.active_profile.name, res.pinned_ip_updated);
    } catch (err: any) {
      onToast?.("Ошибка переключения", err?.message || String(err), "error");
    } finally {
      setSwitchingId(null);
    }
  };

  const handleBindCurrentIp = async (profileId: string) => {
    if (!currentIp || currentIp === "--") {
      onToast?.("Не определен IP", "Сначала дождитесь завершения аудита сети", "warning");
      return;
    }
    setBindingId(profileId);
    try {
      const res = await bindIpToClaudeProfile(profileId, currentIp, currentCountry);
      setProfiles(res.profiles || []);
      onToast?.(
        "IP закреплен за профилем",
        `Профиль привязан к ${currentIp} (${currentCountry})`,
        "success"
      );
    } catch (err: any) {
      onToast?.("Ошибка привязки IP", err?.message || String(err), "error");
    } finally {
      setBindingId(null);
    }
  };

  const handleDelete = async (profileId: string, name: string) => {
    if (profileId === activeProfileId) return;
    try {
      const res = await deleteClaudeProfile(profileId);
      setProfiles(res.profiles || []);
      onToast?.("Профиль удален", `Удален изолированный профиль «${name}»`, "info");
    } catch (err: any) {
      onToast?.("Ошибка удаления", err?.message || String(err), "error");
    }
  };

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    const cleanName = newProfileName.trim();
    if (!cleanName) return;

    setCreating(true);
    try {
      const ipToBind = bindCurrentOnCreate && currentIp !== "--" ? currentIp : null;
      const countryToBind =
        bindCurrentOnCreate && currentCountry !== "--" ? currentCountry : null;
      const res = await createClaudeProfile(
        cleanName,
        ipToBind,
        countryToBind,
        null
      );
      setProfiles(res.profiles || []);
      setNewProfileName("");
      setShowNewForm(false);
      onToast?.(
        "Профиль создан",
        `Изолированный профиль «${cleanName}» готов к использованию`,
        "success"
      );
    } catch (err: any) {
      onToast?.("Ошибка создания", err?.message || String(err), "error");
    } finally {
      setCreating(false);
    }
  };

  const handleUnbindProxy = async (profileId: string) => {
    setBindingProxyId(profileId);
    try {
      const res = await bindUpstreamToClaudeProfile(profileId, null);
      setProfiles(res.profiles || []);
      onToast?.(
        "Прокси отвязан",
        "Профиль теперь использует общее сетевое подключение",
        "info"
      );
    } catch (err: any) {
      onToast?.("Ошибка отвязки", err?.message || String(err), "error");
    } finally {
      setBindingProxyId(null);
    }
  };

  return (
    <div className="bg-[#12141A] rounded-xl border border-white/[0.08] p-3.5 space-y-3 shadow-xs font-sans">
      {/* Header */}
      <div className="flex items-center justify-between pb-2 border-b border-white/[0.05]">
        <div className="flex items-center gap-2 min-w-0">
          <div className="w-6 h-6 rounded-md bg-emerald-500/10 border border-emerald-500/25 flex items-center justify-center text-emerald-400 shrink-0">
            <Users className="w-3.5 h-3.5" />
          </div>
          <div className="min-w-0">
            <div className="text-xs font-bold text-white tracking-tight flex items-center gap-1.5 truncate">
              <span>Мульти-аккаунты Claude</span>
              <span className="px-1.5 py-0.2 rounded bg-emerald-500/15 text-emerald-300 font-mono text-[9px] font-bold">
                Песочница
              </span>
            </div>
            <div className="text-[10px] text-zinc-400 truncate">
              Раздельные сессии · Свой Device ID на профиль
            </div>
          </div>
        </div>

        <div className="flex items-center gap-1.5 shrink-0">
          <button
            type="button"
            disabled={loading}
            onClick={fetchProfiles}
            title="Обновить список профилей"
            className="p-1 rounded-md text-zinc-400 hover:text-white hover:bg-white/[0.06] cursor-pointer transition-colors"
          >
            <RotateCw className={`w-3 h-3 ${loading ? "animate-spin text-emerald-400" : ""}`} />
          </button>
          <button
            type="button"
            onClick={() => setShowNewForm((v) => !v)}
            className="flex items-center gap-1 px-2 py-1 rounded-md bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-[10px] font-semibold text-emerald-300 cursor-pointer transition-colors"
          >
            <Plus className="w-3 h-3" />
            <span>Новый</span>
          </button>
        </div>
      </div>

      {/* Explanatory banner */}
      <div className="text-[10px] text-zinc-400 leading-relaxed bg-[#0c0e14] rounded-lg p-2 border border-white/[0.04]">
        Каждый профиль изолирует <span className="text-zinc-200 font-mono">Device ID</span>, токены и кэш сессий в отдельную песочницу. Блокировка одного аккаунта не компрометирует остальные.
      </div>

      {/* Inline Create Form */}
      {showNewForm && (
        <form
          onSubmit={handleCreate}
          className="bg-[#0c0e14] border border-emerald-500/30 rounded-lg p-2.5 space-y-2 animate-in fade-in duration-150"
        >
          <div className="flex items-center justify-between text-[10.5px] font-semibold text-white">
            <span>+ Новый изолированный профиль</span>
            <button
              type="button"
              onClick={() => setShowNewForm(false)}
              className="text-zinc-500 hover:text-zinc-300 cursor-pointer"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>

          <input
            type="text"
            required
            placeholder="Например: Запасной аккаунт (US/NL)..."
            value={newProfileName}
            onChange={(e) => setNewProfileName(e.target.value)}
            className="w-full bg-black/60 border border-white/[0.1] rounded px-2.5 py-1.5 text-xs text-zinc-100 placeholder-zinc-500 focus:outline-hidden focus:border-emerald-500 font-sans"
          />

          <div className="space-y-1">
            <div className="flex items-center justify-between">
              <label className="text-[10px] font-medium text-zinc-300">
                Личный прокси для этого профиля
              </label>
              <span className="text-[9px] font-mono text-amber-400 bg-amber-500/10 px-1.5 py-0.2 rounded border border-amber-500/25">
                Скоро в обновлении
              </span>
            </div>
            <div
              onClick={() => {
                onToast?.(
                  "Доступно в следующем обновлении",
                  "Привязка отдельного прокси-ключа к профилю временно закрыта: сейчас идёт финальная разработка Telegram-бота автоматической выдачи проверенных Чистых IP. Это защищает шлюз Заставы от поломки из-за некорректных сторонних конфигов.",
                  "warning"
                );
              }}
              className="w-full bg-black/40 border border-white/[0.08] rounded px-2.5 py-1.5 text-xs text-zinc-400 cursor-pointer font-mono flex items-center justify-between hover:border-amber-500/40 transition-colors"
            >
              <span className="text-zinc-500 truncate text-[10.5px]">
                Авто-выдача Чистых IP через бота (в разработке)...
              </span>
              <Lock className="w-3 h-3 text-zinc-500 shrink-0 ml-1" />
            </div>
            <p className="text-[9.5px] text-zinc-400 leading-tight">
              Бот автоматической выдачи проверенных Чистых IP для каждого профиля будет доступен в следующем патче.
            </p>
          </div>

          <div className="flex items-center justify-between pt-1">
            <label className="flex items-center gap-1.5 text-[10px] text-zinc-400 cursor-pointer select-none">
              <input
                type="checkbox"
                checked={bindCurrentOnCreate}
                onChange={(e) => setBindCurrentOnCreate(e.target.checked)}
                className="rounded border-zinc-700 bg-zinc-900 text-emerald-500 accent-emerald-500"
              />
              <span>Привязать текущий IP ({currentIp || "..."})</span>
            </label>

            <button
              type="submit"
              disabled={creating || !newProfileName.trim()}
              className="px-2.5 py-1 rounded bg-emerald-500 hover:bg-emerald-400 text-zinc-950 font-bold text-[10.5px] cursor-pointer disabled:opacity-50 transition-colors"
            >
              {creating ? "Создание..." : "Создать профиль"}
            </button>
          </div>
        </form>
      )}

      {/* Profiles List */}
      <div className="space-y-2">
        {profiles.map((p) => {
          const isActive = p.id === activeProfileId;
          const isSwitching = switchingId === p.id;
          const isBinding = bindingId === p.id;
          const shortDeviceId = p.isolated_device_id
            ? `#${p.isolated_device_id.slice(0, 8)}`
            : "#iso-node";

          return (
            <div
              key={p.id}
              className={`p-3 rounded-lg border transition-all space-y-2 ${
                isActive
                  ? "bg-[#0c0e14] border-emerald-500/35 ring-1 ring-emerald-500/20"
                  : "bg-[#0c0e14]/60 border-white/[0.05] hover:border-white/[0.1]"
              }`}
            >
              {/* Row 1: Profile identity on left, Action buttons on right */}
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2 min-w-0">
                  <div
                    className={`w-6 h-6 rounded-md flex items-center justify-center shrink-0 border ${
                      isActive
                        ? "bg-emerald-500/15 border-emerald-500/30 text-emerald-400"
                        : "bg-zinc-900 border-white/[0.06] text-zinc-500"
                    }`}
                  >
                    <Fingerprint className="w-3.5 h-3.5" />
                  </div>

                  <div className="flex items-center gap-1.5 min-w-0">
                    <span className="text-xs font-bold text-white truncate">
                      {p.name}
                    </span>
                    {isActive && (
                      <span className="px-1.5 py-0.5 rounded bg-emerald-500/20 text-emerald-300 font-mono text-[8.5px] font-bold shrink-0">
                        АКТИВЕН
                      </span>
                    )}
                    {p.bound_upstream_key && (
                      <span
                        title={`Закреплен прокси: ${p.bound_upstream_key}`}
                        className="px-1.5 py-0.5 rounded bg-sky-500/15 border border-sky-500/30 text-sky-300 font-mono text-[8.5px] font-bold flex items-center gap-1 shrink-0"
                      >
                        <Link2 className="w-2.5 h-2.5" />
                        <span>Прокси активен</span>
                      </span>
                    )}
                  </div>
                </div>

                {/* Right Action buttons */}
                <div className="flex items-center gap-1.5 shrink-0 whitespace-nowrap">
                  {/* Proxy action button */}
                  {p.bound_upstream_key ? (
                    <button
                      type="button"
                      disabled={bindingProxyId === p.id}
                      onClick={() => handleUnbindProxy(p.id)}
                      title="Отвязать личный прокси от профиля"
                      className="px-2 py-1 rounded bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-[9.5px] font-mono text-zinc-400 hover:text-rose-300 flex items-center gap-1 cursor-pointer transition-colors shrink-0 whitespace-nowrap"
                    >
                      <X className="w-2.5 h-2.5" />
                      <span>Отвязать прокси</span>
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={() => handleProxyClick(p.id)}
                      title="Привязать чистый прокси-ключ"
                      className="px-2 py-1 rounded bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-[9.5px] font-sans text-zinc-300 hover:text-sky-300 flex items-center gap-1 cursor-pointer transition-colors shrink-0 whitespace-nowrap"
                    >
                      <Link2 className="w-2.5 h-2.5 text-sky-400" />
                      <span>Привязать прокси</span>
                    </button>
                  )}

                  {/* Bind current IP button */}
                  {currentIp && currentIp !== "--" && p.bound_ip !== currentIp && (
                    <button
                      type="button"
                      disabled={isBinding}
                      onClick={() => handleBindCurrentIp(p.id)}
                      title={`Привязать текущий IP (${currentIp}) к этому профилю`}
                      className="px-2 py-1 rounded bg-zinc-900 hover:bg-zinc-800 border border-white/[0.08] text-[9.5px] font-sans text-zinc-300 hover:text-white flex items-center gap-1 cursor-pointer transition-colors shrink-0 whitespace-nowrap"
                    >
                      <Pin className="w-2.5 h-2.5 text-emerald-400" />
                      <span>{isBinding ? "..." : "Привязать IP"}</span>
                    </button>
                  )}

                  {/* Switch button */}
                  {!isActive ? (
                    <button
                      type="button"
                      disabled={isSwitching}
                      onClick={() => handleSwitch(p.id)}
                      className="px-2.5 py-1 rounded bg-emerald-500 hover:bg-emerald-400 text-zinc-950 font-bold text-[10px] cursor-pointer transition-colors flex items-center gap-1 shadow-xs shrink-0 whitespace-nowrap"
                    >
                      {isSwitching ? (
                        <>
                          <RefreshCw className="w-2.5 h-2.5 animate-spin" />
                          <span>Смена...</span>
                        </>
                      ) : (
                        <span>Переключить</span>
                      )}
                    </button>
                  ) : (
                    <div className="flex items-center gap-1 px-2 py-1 text-emerald-400 font-sans text-[9.5px] font-semibold shrink-0 whitespace-nowrap">
                      <Check className="w-3 h-3 text-emerald-400" />
                      <span>В работе</span>
                    </div>
                  )}

                  {/* Delete button (only for non-active profiles) */}
                  {!isActive && profiles.length > 1 && (
                    <button
                      type="button"
                      onClick={() => handleDelete(p.id, p.name)}
                      title="Удалить изолированный профиль"
                      className="p-1 rounded text-zinc-500 hover:text-rose-400 hover:bg-rose-500/10 cursor-pointer transition-colors shrink-0"
                    >
                      <Trash2 className="w-3 h-3" />
                    </button>
                  )}
                </div>
              </div>

              {/* Row 2: Full-width clean metadata line */}
              <div className="flex items-center gap-2 pt-1.5 border-t border-white/[0.04] text-[9.5px] font-mono text-zinc-400 tabular-nums whitespace-nowrap overflow-x-auto">
                <span title={`Полный Device ID: ${p.isolated_device_id}`}>
                  ID: <span className="text-zinc-300">{shortDeviceId}</span>
                </span>
                <span className="text-zinc-600">·</span>
                <span>
                  Закреплённый IP:{" "}
                  {p.bound_ip ? (
                    <span className="text-emerald-300 font-semibold">
                      {p.bound_ip} ({p.bound_country || "NL"})
                    </span>
                  ) : (
                    <span className="text-zinc-500">Не привязан</span>
                  )}
                </span>
              </div>

              {/* Safeguard Warning Banner when user clicks "Привязать прокси" */}
              {guardNoticeProfileId === p.id && (
                <div className="mt-2 p-2.5 rounded-lg bg-amber-500/10 border border-amber-500/30 text-amber-200 text-xs space-y-1 animate-in fade-in duration-150">
                  <div className="flex items-center justify-between font-bold text-amber-300 text-[11px]">
                    <div className="flex items-center gap-1.5">
                      <AlertTriangle className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                      <span>Доступно в следующем обновлении</span>
                    </div>
                    <button
                      type="button"
                      onClick={() => setGuardNoticeProfileId(null)}
                      className="text-amber-400 hover:text-white cursor-pointer"
                    >
                      <X className="w-3 h-3" />
                    </button>
                  </div>
                  <p className="text-[10px] text-zinc-300 leading-relaxed font-sans">
                    Привязка отдельного прокси-ключа к профилю временно закрыта: сейчас идёт финальная разработка Telegram-бота автоматической выдачи проверенных Чистых IP. Это защищает шлюз Заставы от поломки из-за некорректных сторонних конфигов.
                  </p>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};
