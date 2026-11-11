import pkg from "../../package.json";

/** Версия приложения из package.json; в окне приоритетнее версия, которую вернул бэкенд. */
export const APP_VERSION: string = pkg.version;
