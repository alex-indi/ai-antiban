import React from "react";
import { RotateCw, Check, Copy, CheckCircle2, FolderArchive, Trash2 } from "lucide-react";
import type { ClaudeBackup } from "../shared/hooks";

interface ClaudeBackupCardProps {
  backup: ClaudeBackup;
}

/** Карточка экспорта и очистки данных Claude (вкладка «Бэкап»). Разметка перенесена из Dashboard без изменений; её перерисует следующая волна. */
export const ClaudeBackupCard: React.FC<ClaudeBackupCardProps> = ({ backup }) => {
  const {
    claudeSummary,
    loadingClaudeSummary,
    claudeExporting,
    claudeExportResult,
    copiedPrompt,
    cleaningClaude,
    cleaningGpt,
    loadClaudeSummary,
    handleExportClaude,
    handleCopyRestorePrompt,
    handleCleanSpecificClientCache,
  } = backup;

  return (
      <div className="bg-[#12141A] rounded-xl border border-white/[0.08] p-3.5 space-y-3 shadow-xs">
        <div className="flex items-center justify-between pb-2 border-b border-white/[0.05]">
          <div className="flex items-center gap-2 min-w-0">
            <div className="w-6 h-6 rounded-md bg-emerald-500/10 border border-emerald-500/25 flex items-center justify-center text-emerald-400 shrink-0">
              <FolderArchive className="w-3.5 h-3.5" />
            </div>
            <div className="min-w-0">
              <div className="text-xs font-bold text-white tracking-tight flex items-center gap-1.5 truncate">
                <span>Экспорт и бэкап данных Claude</span>
                <span className="px-1.5 py-0.2 rounded bg-emerald-500/15 text-emerald-300 font-mono text-[9px] font-bold">
                  ~/.claude
                </span>
              </div>
              <div className="text-[10px] text-zinc-400 truncate">
                Резервное копирование проектов, сессий и контекста в 1 клик
              </div>
            </div>
          </div>

          <button
            type="button"
            disabled={loadingClaudeSummary}
            onClick={loadClaudeSummary}
            className="p-1 rounded-md text-zinc-400 hover:text-white hover:bg-white/[0.06] cursor-pointer transition-colors shrink-0"
            title="Пересканировать директорию ~/.claude"
          >
            <RotateCw
              className={`w-3 h-3 ${
                loadingClaudeSummary ? "animate-spin text-emerald-400" : ""
              }`}
            />
          </button>
        </div>

        <div className="text-[10.5px] text-zinc-300 leading-relaxed bg-[#0c0e14] rounded-lg p-2.5 border border-white/[0.04]">
          Автоматически собирает историю диалогов, проекты и долговременную память Claude из директории <span className="text-zinc-100 font-mono">~/.claude</span>. Позволяет перенести накопленный контекст на другой аккаунт или упаковать защищенный архив на Рабочий стол перед сменой профиля.
        </div>

        {/* Stats summary */}
        <div className="bg-black/50 rounded-lg px-3 py-2 border border-white/[0.04] text-[10px] font-mono tabular-nums text-zinc-300">
          <div className="flex items-center justify-between flex-wrap gap-x-2 gap-y-1">
            <span>
              Проектов:{" "}
              <strong className="text-emerald-300 font-bold">
                {claudeSummary?.projects_count ?? 0}
              </strong>
            </span>
            <span>·</span>
            <span>
              Сессий (.jsonl):{" "}
              <strong className="text-emerald-300 font-bold">
                {claudeSummary?.sessions_count ?? 0}
              </strong>
            </span>
            <span>·</span>
            <span>
              Память:{" "}
              <strong className="text-emerald-300 font-bold">
                {claudeSummary?.memory_files_count ?? 0}
              </strong>
            </span>
            <span>·</span>
            <span>
              Объем:{" "}
              <strong className="text-emerald-300 font-bold">
                {((claudeSummary?.total_size_bytes ?? 0) / (1024 * 1024)).toFixed(1)} МБ
              </strong>
            </span>
          </div>
        </div>

        {/* Export & copy buttons */}
        <div className="grid grid-cols-2 gap-2 pt-0.5">
          <button
            type="button"
            disabled={claudeExporting}
            onClick={handleExportClaude}
            className="py-2 px-3 rounded-lg bg-emerald-500 hover:bg-emerald-400 active:scale-[0.99] text-zinc-950 text-xs font-sans font-bold flex items-center justify-center gap-1.5 cursor-pointer disabled:opacity-50 transition-colors shadow-sm truncate"
          >
            <FolderArchive className="w-3.5 h-3.5 shrink-0" />
            <span className="truncate">
              {claudeExporting ? "Упаковка..." : "Упаковать архив на Рабочий стол"}
            </span>
          </button>

          <button
            type="button"
            disabled={claudeExporting}
            onClick={handleCopyRestorePrompt}
            className="py-2 px-3 rounded-lg bg-zinc-900 hover:bg-zinc-800 active:scale-[0.99] border border-white/[0.1] hover:border-white/[0.2] text-zinc-200 text-xs font-sans font-medium flex items-center justify-center gap-1.5 cursor-pointer disabled:opacity-50 transition-colors truncate"
          >
            {copiedPrompt ? (
              <>
                <Check className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                <span className="text-emerald-300 truncate">Промт скопирован</span>
              </>
            ) : (
              <>
                <Copy className="w-3.5 h-3.5 text-zinc-400 shrink-0" />
                <span className="truncate">Скопировать промт переноса</span>
              </>
            )}
          </button>
        </div>

        {claudeExportResult && (
          <div className="bg-emerald-500/10 border border-emerald-500/25 rounded-lg p-2.5 text-[10px] font-mono text-emerald-300 space-y-1 animate-in fade-in duration-150">
            <div className="font-bold flex items-center gap-1.5 text-xs">
              <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
              <span>Архив успешно сохранен на Рабочем столе:</span>
            </div>
            <div className="text-zinc-300 truncate text-[10px] pl-5">
              {claudeExportResult.zip_archive_path || claudeExportResult.export_dir_path}
            </div>
          </div>
        )}

        {/* Deep Cache Cleaner */}
        <div className="bg-[#0c0e14] rounded-lg p-3 border border-white/[0.06] space-y-2 pt-2.5">
          <div className="text-[11px] font-semibold text-zinc-200 flex items-center justify-between">
            <div className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-rose-400 shrink-0" />
              <span>Глубокий сброс кэша и меток устройства</span>
            </div>
            <span className="text-[9.5px] font-mono text-zinc-500">
              Сброс Device ID и сессий
            </span>
          </div>

          <div className="grid grid-cols-2 gap-2 pt-0.5">
            <button
              type="button"
              disabled={cleaningClaude}
              onClick={() => handleCleanSpecificClientCache("anthropic")}
              className="py-1.5 px-2 rounded-lg bg-zinc-900 hover:bg-zinc-800 active:scale-[0.99] border border-rose-500/30 hover:border-rose-500/50 text-rose-300 text-[10.5px] font-sans font-medium flex items-center justify-center gap-1.5 cursor-pointer disabled:opacity-50 transition-colors truncate"
            >
              <Trash2 className="w-3 h-3 text-rose-400 shrink-0" />
              <span className="truncate">
                {cleaningClaude ? "Очистка..." : "Кэш Claude Desktop"}
              </span>
            </button>

            <button
              type="button"
              disabled={cleaningGpt}
              onClick={() => handleCleanSpecificClientCache("openai")}
              className="py-1.5 px-2 rounded-lg bg-zinc-900 hover:bg-zinc-800 active:scale-[0.99] border border-white/[0.1] hover:border-white/[0.2] text-zinc-300 hover:text-white text-[10.5px] font-sans font-medium flex items-center justify-center gap-1.5 cursor-pointer disabled:opacity-50 transition-colors truncate"
            >
              <Trash2 className="w-3 h-3 text-zinc-400 shrink-0" />
              <span className="truncate">
                {cleaningGpt ? "Очистка..." : "Кэш ChatGPT Desktop"}
              </span>
            </button>
          </div>
        </div>
      </div>
  );
};
