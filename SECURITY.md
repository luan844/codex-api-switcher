# Security Policy

## Supported Version

当前仅维护最新发布版本。

## Reporting

请优先通过 GitHub 的私密安全报告功能提交漏洞，不要在公开 issue 中包含 API Key、Token、`auth.json`、备份文件或可识别的本机路径。

报告应包含受影响版本、复现步骤、预期影响和经过脱敏的日志。请勿上传真实 Codex 用户目录或 `%LOCALAPPDATA%\CodexApiSwitcher` 数据目录。

## Local Data

- Provider API Key 使用 Windows DPAPI 加密，只能由同一 Windows 用户解密。
- 切换器备份可能包含 Codex 认证和会话数据，应按敏感文件处理。
- 本项目不会为第三方 API 服务的隐私、安全性或可用性背书。
