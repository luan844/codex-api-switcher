# Contributing

感谢参与 Codex API Switcher。

## 开发流程

1. 从 `main` 创建功能分支。
2. 保持修改聚焦，不提交 `artifacts`、`dist`、Rust `target`、日志、认证文件或本机路径。
3. 提交前运行：

```powershell
npm ci
npm run verify
```

4. 涉及配置、认证、会话迁移或备份恢复的修改必须补充 Rust 回归测试。
5. Pull Request 中说明行为变化、测试结果和可能的恢复方式。

真实 Codex 配置测试应使用临时 `CODEX_HOME`。不要在 issue、日志或测试夹具中提交 API Key、Bearer Token、ChatGPT Token、用户名目录或真实 `auth.json`。
