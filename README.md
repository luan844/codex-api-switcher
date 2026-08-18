# Codex API Switcher

[![Windows Tauri Build](https://github.com/luan844/codex-api-switcher/actions/workflows/windows-tauri-release.yml/badge.svg)](https://github.com/luan844/codex-api-switcher/actions/workflows/windows-tauri-release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Windows 10/11 x64 桌面工具，用于在 Codex 中管理和切换自定义 Responses API Provider。所有 Provider 的 API 地址、API Key、模型列表和默认模型均可直接在软件界面修改，无需手工编辑 Codex 配置文件。

> 本项目是非官方社区工具，与 OpenAI、Codex 或 AIVR 无隶属或授权关系。使用前请确认第三方 API 服务可信，并妥善保存切换器创建的备份。

## 功能

- 新建、编辑、复制和删除多个自定义 Provider
- 在软件内编辑 Base URL、API Key、默认模型和模型目录
- 从 `GET /v1/models` 获取模型，并用最小 `POST /v1/responses` 请求测试连接
- 使用 Windows DPAPI 加密本工具保存的 API Key
- 结构化更新 Codex `config.toml`、`auth.json` 和模型目录
- 保留与本工具无关的 MCP、Skills、沙箱及其他 TOML 配置
- 切换前备份配置、认证、模型目录、会话 JSONL 和 `state_5.sqlite`
- 迁移最近指定天数内会话的 `model_provider`
- 支持冲突检查、失败回滚、备份恢复和官方 OpenAI 模式恢复
- 恢复官方模式时保留 ChatGPT 订阅认证和历史会话索引
- 检测、关闭和重启 Codex Desktop
- 可选通过本地 CDP 将自定义模型注入 Codex 模型菜单
- 日志自动遮蔽 API Key、Bearer Token 和用户目录

本项目不包含 AIVR 的闭源代码、商标、图标、教程或更新服务。

## 下载

可从仓库的 [Releases](../../releases) 页面下载安装包。未签名构建可能触发 Windows SmartScreen 的“未知发布者”提示，请仅从本仓库发布页下载并核对发布说明。

## 使用

1. 打开“Provider 工作台”，新建一个 Provider。
2. 填写 Provider ID、Base URL、API Key、默认模型和模型列表。
3. 先执行“获取模型”或“测试连接”。
4. 点击“切换并重启 Codex”，确认预检和备份范围。
5. 需要撤销时，在“备份恢复”中恢复指定备份，或执行“恢复官方 OpenAI”。

远程 Base URL 必须使用 HTTPS。为方便本地代理开发，`localhost`、`127.0.0.1` 和 `::1` 可使用 HTTP。

## 数据位置

| 数据 | 默认位置 |
| --- | --- |
| Codex 配置 | `%USERPROFILE%\.codex`，或环境变量 `CODEX_HOME` |
| 应用数据库 | `%LOCALAPPDATA%\CodexApiSwitcher\switcher.json` |
| 备份 | `%LOCALAPPDATA%\CodexApiSwitcher\backups` |
| 日志 | `%LOCALAPPDATA%\CodexApiSwitcher\logs` |

API Key 的 DPAPI 密文只可由同一 Windows 用户解密。自定义 Provider 通过本工具的本地凭据助手按需向 Codex 提供密钥，不把第三方 API Key 明文写入 `auth.json`。切换前仍会备份 `auth.json`，以便恢复官方 ChatGPT 订阅认证。

## 开发

要求：

- Windows 10/11 x64
- Node.js 20 或更高版本
- Rust stable MSVC
- Visual Studio C++ Build Tools 和 Windows SDK

```powershell
npm install
npm run verify
npm run tauri dev
```

构建 Windows 版本：

```powershell
npm run build:windows
```

输出位置：

- 便携主程序：`src-tauri\target\release\codex-api-switcher.exe`
- NSIS 安装包：`src-tauri\target\release\bundle\nsis\*.exe`

## 验证

```powershell
npm run test
npm run typecheck
npm run test:rust
npm run check:rust
npm run verify
```

涉及真实 Codex 配置的开发测试应优先设置临时 `CODEX_HOME`。恢复备份前必须关闭 Codex，避免覆盖正在使用的 SQLite 和会话文件。

## 开源说明

本项目是在 MIT 许可项目 [Rain-Of-Stars/Codex-Switch-Tauri2](https://github.com/Rain-Of-Stars/Codex-Switch-Tauri2) 基础上进行的独立品牌二次开发；该项目又基于或参考 [54xzh/codex-switch](https://github.com/54xzh/codex-switch)。上游版权和许可证文本见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

本项目使用 MIT License，详见 [LICENSE](LICENSE)。

问题反馈和代码贡献请参阅 [CONTRIBUTING.md](CONTRIBUTING.md)；安全问题请参阅 [SECURITY.md](SECURITY.md)。版本变化记录见 [CHANGELOG.md](CHANGELOG.md)。
