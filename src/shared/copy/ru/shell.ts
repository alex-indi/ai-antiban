export const tabs = {
  label: "Разделы",
  overview: "Обзор",
  connect: "Подключение",
  checks: "Проверки",
  backup: "Бэкап",
  guides: "Гайды",
} as const;

export const shell = {
  footer: "Работает локально · чаты не читаются",
  pingLabel: (ms: number) => `${ms} мс`,
  menuLabel: "Ещё",
  back: "Назад",
  subscriptionTitle: "Подписка",
  menu: {
    subscription: "Подписка",
    settings: "Настройки",
    events: "Лента событий шлюза",
    rescan: "Перепроверить сеть",
    syncTimezone: "Часовой пояс для ИИ",
    checkUpdates: "Проверить обновления",
    updateTo: (version: string) => `Обновить до v${version}`,
    checkingUpdates: "Проверяем обновления…",
    onboarding: "Пройти знакомство заново",
    support: "Написать в поддержку",
    about: "О программе",
    scenarioNl: "Сценарий: Нидерланды",
    scenarioTr: "Сценарий: Турция",
    scenarioRu: "Сценарий: Россия",
  },
  toast: {
    supportTitle: "Поддержка в Telegram",
    supportText: "Открываем @nomadnocode.",
  },
  intercept: {
    title: (service: string) => `Запрос к ${service} приостановлен`,
    ruText: "Вы вышли в сеть из России без VPN.",
    ipText: (ip: string, country: string) => `Адрес ${ip}${country ? ` · ${country}` : ""}.`,
    allow: "Снять паузу",
    dismiss: "Скрыть",
  },
} as const;

export const timezone = {
  title: "Часовой пояс",
  text: "Часовой пояс компьютера не совпадает со страной вашего IP. Застава может выровнять его для ИИ-инструментов: VS Code, Cursor и терминала.",
  done: "Часовой пояс ИИ синхронизирован.",
  sync: "Синхронизировать для ИИ",
  syncing: "Синхронизируем…",
  osClock: "Открыть часы системы",
  close: "Закрыть",
} as const;

export const updates = {
  available: (version: string) => `Доступно обновление v${version}`,
  upToDate: "Установлена актуальная версия",
  versions: (current: string, latest: string) => `Сейчас v${current} · в релизе v${latest}`,
  whatsNew: "Что нового",
  fullText: "Показать полный текст",
  shortList: "Показать краткий список",
  noNotes: "Описание релиза отсутствует.",
  install: "Обновить",
  installing: "Устанавливаем…",
  openRelease: "Открыть релиз на GitHub",
  ok: "Понятно",
} as const;

export const about = {
  title: "О программе",
  version: "Версия",
  text: "Застава следит за сетевым окружением при работе с ИИ-сервисами и останавливает запросы, если адрес небезопасен.",
  privacy: "Всё работает локально: чаты не читаются, трафик не уходит на сторонние серверы Заставы.",
  support: "Написать в поддержку",
  close: "Закрыть",
} as const;
