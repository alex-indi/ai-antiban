// Ридер гайдов переехал в src/features/guides. Файл оставлен ради совместимости путей:
// src/features/connect/hostings.ts берёт VPS_PROVIDERS отсюда.
export { VPS_PROVIDERS } from "../features/guides/providers";
export type { VpsProvider, VpnSetupMethod } from "../features/guides/providers";
export { GuidesScreen as GuideViewer } from "../features/guides/GuidesScreen";
