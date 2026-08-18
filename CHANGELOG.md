# Changelog

本项目遵循语义化版本。GitHub 上的第一个发布版本从 `0.1.2` 开始。

## 0.1.2 - 2026-08-01

### Added

- 多 Provider 管理、模型发现、Responses API 连接测试和默认模型选择。
- Windows DPAPI 密钥保护、本地凭据助手、事务备份、回滚和日志脱敏。
- Codex Desktop 检测、关闭、重启、会话迁移和可选模型菜单注入。
- 从 Codex 原生模型缓存继承完整思考等级和模型能力元数据。

### Fixed

- 获取模型前允许 Provider 草稿暂时没有默认模型。
- 保留带 `/v1` 的 Base URL，避免拼接出重复路径。
- 官方恢复固定使用内置 `openai` Provider，并恢复 ChatGPT 订阅认证。
- 修复第三方 Provider 被误存为官方快照后登录状态和会话列表不可见的问题。
