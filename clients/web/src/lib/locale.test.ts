import { chooseInitialLocale, lookupLabel, normalizeLocale } from "./locale";
import { formatGreetingTime, formatRelativeAge } from "./format";

function assertEqual<T>(actual: T, expected: T) {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
  }
}

assertEqual(normalizeLocale("zh"), "zh-CN");
assertEqual(normalizeLocale("zh-Hans-CN"), "zh-CN");
assertEqual(normalizeLocale("zh_CN"), "zh-CN");
assertEqual(normalizeLocale("en-US"), "en");
assertEqual(normalizeLocale("ja"), null);

assertEqual(chooseInitialLocale({ saved: "en", browser: "zh-CN" }), "en");
assertEqual(chooseInitialLocale({ saved: "ja", browser: "zh-TW" }), "zh-CN");
assertEqual(chooseInitialLocale({ saved: null, browser: "fr-FR" }), "zh-CN");
assertEqual(chooseInitialLocale({ saved: null, browser: "fr-FR", fallback: "en" }), "en");

assertEqual(lookupLabel("zh-CN", "languageName"), "简体中文");
assertEqual(lookupLabel("en", "topbarInstallation", "demo"), "Installations / demo");
assertEqual(lookupLabel("zh-CN", "installUrlTitle"), "项目在哪里？");
assertEqual(lookupLabel("zh-CN", "installPlanTitle"), "检查安装计划");
assertEqual(lookupLabel("zh-CN", "failureRestartInstallation"), "重启项目");
assertEqual(lookupLabel("zh-CN", "installationFrameBackHome"), "返回首页");
assertEqual(lookupLabel("zh-CN", "homeInstallationPopupBlockedTitle"), "项目标签页被拦截");
assertEqual(lookupLabel("zh-CN", "installationFrameLoadingSurface"), "正在加载项目界面…");
assertEqual(lookupLabel("zh-CN", "homeTimeHoursAgo", 2), "2 小时前");
assertEqual(lookupLabel("en", "installPackagesWillInstall", 2), "2 packages will be installed");

const fixedNow = Date.now;
Date.now = () => new Date("2026-05-26T12:00:00Z").getTime();
assertEqual(
  formatRelativeAge("2026-05-26T10:00:00Z", {
    now: lookupLabel("zh-CN", "homeContinueAgeNow"),
    minutesAgo: (count) => lookupLabel("zh-CN", "homeTimeMinutesAgo", count),
    hoursAgo: (count) => lookupLabel("zh-CN", "homeTimeHoursAgo", count),
    daysAgo: (count) => lookupLabel("zh-CN", "homeTimeDaysAgo", count),
    weeksAgo: (count) => lookupLabel("zh-CN", "homeTimeWeeksAgo", count),
    monthsAgo: (count) => lookupLabel("zh-CN", "homeTimeMonthsAgo", count),
    yearsAgo: (count) => lookupLabel("zh-CN", "homeTimeYearsAgo", count),
  }),
  "2 小时前",
);
Date.now = fixedNow;

if (!formatGreetingTime("zh-CN", new Date("2026-05-26T12:00:00Z"), "工作台").startsWith("工作台 ·")) {
  throw new Error("localized greeting prefix was not used");
}
