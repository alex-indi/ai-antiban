/**
 * Все строки новых экранов (оболочка, «Обзор», «Подключение», диалог обрыва) и единый словарь статусов.
 * Правила BRAND.md: на «вы», обычный регистр (капс только в overline), кнопка — глагол и объект,
 * без эмодзи, число с единицей через пробел.
 */

/** Единый словарь статусов: один Badge вместо пяти разных словарей. */
export const status = {
  protected: "Защищено",
  available: "Доступен",
  clean: "Чисто",
  review: "Нужна проверка",
  leak: "Утечка",
  off: "Выключено",
  testing: "Тестирование",
  soon: "Скоро",
  demo: "Демо",
  /** Дополнение словаря: сервис удержан шлюзом (критичная проверка или ручной блок). */
  blocked: "Заблокирован",
  /** Дополнение словаря: событие шлюза без ИИ, пошло мимо защиты. */
  direct: "Напрямую",
} as const;

export const tabs = {
  label: "Разделы",
  overview: "Обзор",
  connect: "Подключение",
  checks: "Проверки",
  backup: "Бэкап",
  guides: "Гайды",
} as const;

export const shell = {
  appName: "Застава",
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
    onboardingSoon: "Появится в этом обновлении",
    onboardingSoonText: "Знакомство с Заставой будет доступно в следующей сборке.",
    supportTitle: "Поддержка в Telegram",
    supportText: "Открываем @nomadnocode.",
  },
  intercept: {
    title: (service: string) => `Запрос к ${service} приостановлен`,
    ruText: "Вы вышли в сеть из России без VPN.",
    ipText: (ip: string, country: string) => `Адрес ${ip}${country ? ` · ${country}` : ""}.`,
    allow: "Снять паузу",
    dismiss: "Скрыть",
    dismissLabel: "Скрыть уведомление",
  },
} as const;

export const providers = {
  anthropic: { name: "Claude", domains: "claude.ai · api.anthropic.com" },
  openai: { name: "ChatGPT", domains: "chatgpt.com · openai.com" },
  gemini: { name: "Gemini", domains: "gemini.google.com · aistudio" },
} as const;

export const overview = {
  sectionServices: "Сервисы ИИ",
  sectionProtected: "Под защитой",
  sectionToday: "Сегодня",
  verdict: {
    ok: "Вы защищены",
    attention: "Есть замечания",
    stopped: "Доступ к ИИ остановлен",
    frozen: "Сеть ИИ остановлена",
    off: "Защита выключена",
  },
  addressPending: "Определяем адрес…",
  facts: {
    checks: (passed: number, total: number, word: string) => `${passed} из ${total} ${word}`,
    pinned: "IP закреплён",
    pinChanged: "IP сменился",
    key: "Ключ Заставы",
  },
  guard: {
    title: "Защита",
    onText: "Запросы к ИИ идут только через чистый IP",
    offText: "Запросы к ИИ идут напрямую, без защиты",
    label: "Защита",
  },
  ipMenu: {
    label: "Действия с IP",
    copy: "Скопировать IP",
    pin: "Закрепить IP",
    unpin: "Открепить IP",
    strictOn: "Включить строгий режим",
    strictOff: "Выключить строгий режим",
  },
  service: {
    session: "Блок снят на эту сессию",
    review: "Подробности — в «Проверках»",
    blocked: "Запросы удерживаются шлюзом",
    unblock: "Снять блок",
    allow: "Разрешить на сессию",
    missing: "Данных пока нет",
    checking: "Проверяем",
  },
  browsers: { title: "Браузеры", sub: "Chrome, Safari, Arc, Edge, Яндекс" },
  editors: { title: "Редакторы кода и CLI", sub: "VS Code, Cursor, Windsurf, терминал" },
  stats: {
    requests: "запросов к ИИ проверено",
    leaks: "утечек остановлено",
    direct: "обычных — напрямую",
  },
  events: {
    open: "Открыть ленту событий",
    title: "Лента событий шлюза",
    tunnels: (n: number) => `Активных туннелей ИИ: ${n}`,
    last: "Последний запрос",
    waiting: "Ждём первых запросов к Claude, ChatGPT и Gemini.",
    close: "Закрыть",
  },
  actions: {
    crashTest: "Тест обрыва VPN",
    stopNetwork: "Остановить сеть",
    resumeNetwork: "Возобновить сеть",
  },
  banner: {
    openChecks: "Открыть проверки",
    stopped: {
      title: "Запросы к ИИ удерживаются",
      text: "Найден небезопасный адрес или утечка. Подробности — во вкладке «Проверки».",
    },
    frozen: {
      title: "Сеть ИИ остановлена",
      text: "Все запросы к Claude, ChatGPT и Gemini заморожены, пока вы не возобновите сеть.",
    },
    leak: {
      title: "Возможна утечка адреса",
      text: "Часть трафика может идти мимо защиты. Подробности — во вкладке «Проверки».",
    },
    timezone: {
      title: "Часовой пояс не совпадает со страной IP",
      text: "Это повышает риск дополнительных проверок у ИИ-сервисов.",
      action: "Настроить часовой пояс",
    },
    review: {
      title: "У адреса есть замечания",
      text: "ИИ-сервис может запросить дополнительную проверку. Подробности — во вкладке «Проверки».",
    },
    update: {
      title: (version: string) => `Доступно обновление v${version}`,
      text: "Новая версия Заставы готова к установке.",
      action: "Что нового",
    },
  },
  error: {
    title: "Нет связи с сетью",
    text: "Не удалось проверить соединение. Проверьте интернет и VPN, затем повторите.",
    action: "Повторить проверку",
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

export const connect = {
  vpn: {
    title: "Свой VPN за 15 минут",
    badge: "Бесплатно",
    text: "Личный сервер с IP, которым никто больше не пользуется. Оплата картой РФ или СБП.",
    open: "Открыть инструкцию",
    hostings: {
      hostvds: "Нидерланды · от 190 ₽ в месяц",
      ishosting: "Нидерланды, IPv4 и IPv6 · от 5 $ в месяц",
      timeweb: "Оплата СБП и «Мир» · от 180 ₽ в месяц",
    },
  },
  key: {
    title: "Ключ Заставы",
    badge: status.testing,
    text: "Только трафик ИИ пойдёт через выделенный IP. Банки, рабочие сайты и загрузки — напрямую, на полной скорости.",
    fieldLabel: "Ключ или адрес прокси",
    placeholder: "zastava://…",
    caption: "Функция в тестировании. Ключи выдаются ограниченно.",
    connect: "Подключить",
    connecting: "Подключаем…",
    get: "Получить ключ",
    empty: "Введите ключ или адрес прокси.",
    defaultLabel: "Выделенный шлюз Заставы",
    switchLabel: "Ключ Заставы",
    switchOn: "Трафик ИИ идёт через выделенный IP",
    switchOff: "Ключ на паузе, трафик идёт напрямую",
    exit: "Выходной адрес",
    exitUnknown: "Определяем адрес…",
    disconnect: "Отключить ключ",
    toast: {
      connected: "Ключ подключён",
      connectedText: "Трафик ИИ пойдёт через выделенный IP.",
      connectError: "Не удалось подключить ключ",
      on: "Ключ включён",
      onText: "Трафик ИИ снова идёт через выделенный IP.",
      off: "Ключ на паузе",
      offText: "Трафик идёт напрямую через вашу сеть.",
      toggleError: "Не удалось переключить ключ",
      disconnected: "Ключ отключён",
      disconnectedText: "Настройки шлюза очищены, запросы идут напрямую.",
      disconnectError: "Не удалось отключить ключ",
    },
  },
} as const;

export const alertDialog = {
  title: "Запрос к ИИ приостановлен",
  textDrop: "VPN отключился, и запрос ушёл бы с небезопасного адреса. Застава держит его, пока вы не решите.",
  textRegion: "Выход в сеть идёт из неподдерживаемого региона. Застава остановила запрос, чтобы аккаунт не заблокировали.",
  textRisk: "Обнаружен риск для сессии ИИ. Застава держит запрос, пока вы не вернётесь в доверенную сеть.",
  textDemo: "Демонстрация: сеть не заблокирована, запрос никуда не ушёл.",
  feedbackDefault: "Демонстрация: в реальной ситуации шлюз остановил бы трафик до включения VPN.",
  feedbackAllow: "Демонстрация: сессия была бы разрешена.",
  unknownIp: "Неизвестный IP",
  closeLabel: "Закрыть окно",
  /** Порядок совпадает с клавишами 1 → 2 → 3. */
  openGuide: "Открыть инструкцию",
  allow: "Разрешить",
  block: "Блокировать",
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

export const toast = {
  auditError: "Не удалось проверить соединение",
  ipCopied: "IP скопирован",
  pinned: "IP закреплён",
  pinnedText: (ip: string) => `Адрес ${ip} сохранён как эталонный.`,
  pinError: "Не удалось закрепить IP",
  unpinned: "IP откреплён",
  unpinnedText: "Привязка к эталонному адресу сброшена.",
  genericError: "Что-то пошло не так",
  strictOn: "Строгий режим включён",
  strictOnText: "Любая смена IP будет остановлена до отправки запроса.",
  strictOff: "Строгий режим выключен",
  strictOffText: "Вместо остановки вы будете получать уведомления.",
  accessAllowed: "Блок снят",
  accessAllowedText: (name: string) => `${name} разрешён до конца сессии.`,
  guard: {
    onTitle: "Защита включена",
    onText: "Запросы к ИИ-доменам идут через Заставу (порт 8899).",
    offTitle: "Защита выключена",
    offText: "Системная маршрутизация восстановлена, запросы идут напрямую.",
    error: "Не удалось переключить защиту",
  },
  cli: {
    onTitle: "Защита VS Code, Cursor и CLI включена",
    onText: (targets: string) => `Настроено: ${targets}. При выходе из Заставы прокси отключается сам.`,
    onFallbackTargets: "терминал и редакторы",
    offTitle: "Защита CLI выключена",
    offText: "Маршрутизация удалена из настроек редакторов и терминала.",
    error: "Не удалось настроить CLI",
  },
  killswitch: {
    onTitle: "Сеть ИИ остановлена",
    onText: "Все запросы к ИИ заморожены.",
    offTitle: "Сеть ИИ возобновлена",
    offText: "Защита работает в штатном режиме, доступ к ИИ открыт.",
    error: "Не удалось переключить сеть",
  },
  intercept: {
    allowedTitle: "Пауза снята",
    allowedText: "Трафик к ИИ разрешён на эту сессию.",
    simulateTitle: "Тест перехвата запущен",
    simulateText: "Открыто окно экстренного удержания трафика.",
    simulateError: "Не удалось запустить тест",
  },
  timezone: {
    done: "Часовой пояс ИИ синхронизирован",
    error: "Не удалось синхронизировать часовой пояс",
  },
  updates: {
    available: (version: string) => `Доступно обновление v${version}`,
    availableText: "Откройте «Что нового» и нажмите «Обновить».",
    none: "Обновлений нет",
    noneText: (version: string) => `Установлена актуальная версия v${version}.`,
    checkError: "Не удалось проверить обновления",
    installed: "Обновление установлено",
    ready: "Обновление готово",
    installError: "Не удалось установить обновление",
    autoOn: "Автообновление включено",
    autoOnText: "Застава будет проверять стабильные релизы сама.",
    autoOff: "Автообновление выключено",
    autoOffText: "Автоматическая проверка отключена.",
    autoError: "Не удалось переключить автообновление",
  },
  profile: {
    switched: "Профиль переключён",
    switchedText: "Активный профиль Claude обновлён.",
  },
  backup: {
    exported: "Архив Claude готов",
    exportedText: (path: string) => `Сохранён на рабочем столе: ${path}`,
    exportError: "Не удалось выгрузить данные",
    promptCopied: "Промпт скопирован",
    promptCopiedText: "Вставьте его в Antigravity, Cursor, Windsurf или новый Claude.",
    promptError: "Не удалось скопировать промпт",
    cacheCleaned: (client: string) => `Кэш ${client} сброшен`,
    cacheCleanedText: (files: number, mb: string) => `Удалено файлов: ${files} (${mb} МБ). Cookies и антифрод-реестры очищены.`,
    cacheError: "Не удалось сбросить кэш",
  },
} as const;
