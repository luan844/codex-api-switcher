import {
  ArrowLeftRight,
  BookTemplate,
  DatabaseBackup,
  LayoutDashboard,
  Logs,
  Settings2,
  SlidersHorizontal,
  type LucideIcon,
} from "lucide-react";

import type { AppRoute } from "@/store/app-shell-store";

export const routeMeta: Record<
  AppRoute,
  {
    label: string;
    shortLabel: string;
    description: string;
  }
> = {
  "/": {
    label: "工作台",
    shortLabel: "控制台",
    description: "集中查看当前激活组合、目标环境、最近切换和备份概况。",
  },
  "/profiles": {
    label: "配置管理",
    shortLabel: "配置",
    description: "以列表加详情方式维护 Profile，支持筛选、复制、编辑和删除。",
  },
  "/switch": {
    label: "切换执行",
    shortLabel: "切换",
    description: "先预检后执行，固定展示主流程步骤与当前切换结果。",
  },
  "/templates": {
    label: "模板与连接测试",
    shortLabel: "模板",
    description: "维护模板文本，并对 APIKEY 组合执行批量连接测试。",
  },
  "/backups": {
    label: "备份与恢复",
    shortLabel: "备份",
    description: "查看目标备份概况、按组恢复并直接打开对应目录。",
  },
  "/settings": {
    label: "系统设置",
    shortLabel: "设置",
    description: "配置写入目标、WSL 覆盖、迁移策略与界面偏好。",
  },
  "/logs": {
    label: "运行日志 / 关于",
    shortLabel: "日志",
    description: "查看应用版本、运行说明以及最近结构化日志记录。",
  },
};

export const navigationItems: Array<{
  route: AppRoute;
  label: string;
  icon: LucideIcon;
}> = [
  { route: "/", label: "工作台", icon: LayoutDashboard },
  { route: "/profiles", label: "配置管理", icon: SlidersHorizontal },
  { route: "/switch", label: "切换执行", icon: ArrowLeftRight },
  { route: "/templates", label: "模板与连接测试", icon: BookTemplate },
  { route: "/backups", label: "备份与恢复", icon: DatabaseBackup },
  { route: "/settings", label: "系统设置", icon: Settings2 },
  { route: "/logs", label: "运行日志 / 关于", icon: Logs },
];
