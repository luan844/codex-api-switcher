const fs = require("node:fs");
const path = require("node:path");

const ALLOW_MARKER = "sensitive-path-check: allow";
const MAX_RESULTS = 50;
const repoRoot = process.cwd();
const repoName = path.basename(repoRoot).toLowerCase();
const safeUserSegments = new Set([
  "<用户名>",
  "{user_name}",
  "${user_name}",
  "demo-user",
  "workspace",
  "analyst",
  "username",
  "user",
  "example",
  "sample",
  "tester",
  "test-user",
]);
const textExtensions = new Set([
  ".cjs",
  ".css",
  ".html",
  ".js",
  ".json",
  ".jsonc",
  ".md",
  ".mjs",
  ".ps1",
  ".rs",
  ".scss",
  ".sh",
  ".toml",
  ".ts",
  ".tsx",
  ".txt",
  ".yaml",
  ".yml",
]);
const entryPoints = [
  ".github",
  "docs",
  "tooling",
  "src",
  "src-tauri",
  "tests",
  "README.md",
  "CHANGELOG.md",
  "CONTRIBUTING.md",
  "LICENSE",
  "SECURITY.md",
  "THIRD_PARTY_NOTICES.md",
  "components.json",
  "index.html",
  "package.json",
  "tsconfig.json",
  "tsconfig.node.json",
  "vite.config.ts",
  "vitest.config.ts",
];
const ignoredDirectories = new Set([
  ".git",
  "dist",
  "node_modules",
  "target",
]);

const repoPathPatterns = [
  {
    label: "检测到包含仓库目录名的绝对 Windows 路径",
    regex: new RegExp(
      String.raw`\b[A-Za-z]:[\\/][^\s"'<>|]*[\\/]${escapeRegExp(repoName)}(?=[\\/\s"'<>]|$)`,
      "g",
    ),
  },
  {
    label: "检测到包含仓库目录名的绝对 Unix/WSL 路径",
    regex: new RegExp(
      String.raw`/(?:mnt/[a-z]/)?[^\s"'<>]*/${escapeRegExp(repoName)}(?=[/\s"'<>]|$)`,
      "gi",
    ),
  },
];

const homePathPatterns = [
  {
    label: "检测到具体用户目录下的绝对 Windows 路径",
    regex: /\b[A-Za-z]:[\\/](?:Users|Documents and Settings)[\\/](?<user>[^\\/\s"'<>]+)[\\/][^\s"'<>]*/g,
  },
  {
    label: "检测到具体用户目录下的绝对 Unix 路径",
    regex: /\/(?:Users|home)\/(?<user>[^/\s"'<>]+)\/[^\s"'<>]*/g,
  },
  {
    label: "检测到具体用户目录下的绝对 WSL 挂载路径",
    regex: /\/mnt\/[a-z]\/Users\/(?<user>[^/\s"'<>]+)\/[^\s"'<>]*/gi,
  },
];

const secretPatterns = [
  {
    label: "检测到疑似 OpenAI API Key",
    regex: /\bsk-[A-Za-z0-9_-]{20,}\b/g,
  },
  {
    label: "检测到疑似 GitHub Token",
    regex: /\b(?:ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{30,})\b/g,
  },
  {
    label: "检测到疑似 Bearer Token",
    regex: /\bBearer\s+[A-Za-z0-9._~+\/-]{24,}={0,2}\b/gi,
  },
  {
    label: "检测到疑似 JWT",
    regex: /\beyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{10,}\b/g,
  },
  {
    label: "检测到私钥内容",
    regex: /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/g,
  },
];

const findings = [];
const scannedFiles = [];

for (const entryPoint of entryPoints) {
  const absolutePath = path.join(repoRoot, entryPoint);
  if (!fs.existsSync(absolutePath)) {
    continue;
  }

  walkPath(absolutePath);
}

for (const filePath of scannedFiles) {
  const relativePath = normalizeRelativePath(filePath);
  const content = fs.readFileSync(filePath, "utf8");
  const lines = content.split(/\r?\n/u);

  lines.forEach((line, index) => {
    if (line.includes(ALLOW_MARKER)) {
      return;
    }

    checkRepoPathPatterns(relativePath, index + 1, line);
    checkHomePathPatterns(relativePath, index + 1, line);
    checkSecretPatterns(relativePath, index + 1, line);
  });
}

if (findings.length > 0) {
  console.error(`敏感路径检查失败，共发现 ${findings.length} 处可疑位置。`);
  findings.slice(0, MAX_RESULTS).forEach((finding) => {
    console.error(`- ${finding.file}:${finding.line} ${finding.reason}`);
    console.error(`  ${maskSensitivePathSnippets(finding.content)}`);
  });
  if (findings.length > MAX_RESULTS) {
    console.error(`- 其余 ${findings.length - MAX_RESULTS} 处结果已省略。`);
  }
  console.error("修复建议：");
  console.error("1. 用 %USERPROFILE%、%LOCALAPPDATA% 或相对路径替代本机绝对路径。");
  console.error("2. 文档示例改为占位符或运行时拼接，不要写具体用户名或盘符目录。");
  console.error("3. 日志、测试样例和错误消息统一使用 demo-user、<用户名> 这类中性占位值。");
  console.error("4. 确需保留的安全测试样例，请改成通用占位名，避免真实目录名进入仓库。");
  process.exitCode = 1;
} else {
  console.log(`敏感路径检查通过。已扫描 ${scannedFiles.length} 个文本文件。`);
}

function walkPath(targetPath) {
  const stat = fs.statSync(targetPath);
  if (stat.isDirectory()) {
    const directoryName = path.basename(targetPath);
    if (ignoredDirectories.has(directoryName)) {
      return;
    }

    for (const child of fs.readdirSync(targetPath)) {
      walkPath(path.join(targetPath, child));
    }
    return;
  }

  if (!shouldScanFile(targetPath)) {
    return;
  }

  scannedFiles.push(targetPath);
}

function shouldScanFile(filePath) {
  const extension = path.extname(filePath).toLowerCase();
  if (!textExtensions.has(extension)) {
    return false;
  }

  const relativePath = normalizeRelativePath(filePath);
  return !relativePath.startsWith("src-tauri/icons/");
}

function checkRepoPathPatterns(relativePath, lineNumber, line) {
  for (const pattern of repoPathPatterns) {
    for (const match of line.matchAll(pattern.regex)) {
      if (!match[0]) {
        continue;
      }
      const prefix = line.slice(0, match.index);
      if (
        /https?:\/?$/iu.test(prefix) ||
        /https?:\/\/[^\s"'<>]*$/iu.test(prefix)
      ) {
        continue;
      }

      findings.push({
        file: relativePath,
        line: lineNumber,
        reason: pattern.label,
        content: line.trim(),
      });
    }
  }
}

function checkHomePathPatterns(relativePath, lineNumber, line) {
  for (const pattern of homePathPatterns) {
    for (const match of line.matchAll(pattern.regex)) {
      const userSegment = match.groups?.user ?? "";
      if (isSafeUserSegment(userSegment)) {
        continue;
      }

      findings.push({
        file: relativePath,
        line: lineNumber,
        reason: pattern.label,
        content: line.trim(),
      });
    }
  }
}

function checkSecretPatterns(relativePath, lineNumber, line) {
  for (const pattern of secretPatterns) {
    for (const match of line.matchAll(pattern.regex)) {
      if (!match[0]) {
        continue;
      }

      findings.push({
        file: relativePath,
        line: lineNumber,
        reason: pattern.label,
        content: line.trim(),
      });
    }
  }
}

function normalizeRelativePath(filePath) {
  return path.relative(repoRoot, filePath).split(path.sep).join("/");
}

function isSafeUserSegment(value) {
  const normalized = value.trim().toLowerCase();
  if (!normalized) {
    return true;
  }

  if (!/^[a-z0-9._-]+$/iu.test(normalized)) {
    return true;
  }

  if (safeUserSegments.has(normalized)) {
    return true;
  }

  return /[<>{}$]/u.test(normalized);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function maskSensitivePathSnippets(content) {
  let masked = content;

  for (const pattern of [...repoPathPatterns, ...homePathPatterns]) {
    const regex = new RegExp(pattern.regex.source, pattern.regex.flags);
    masked = masked.replace(regex, "[已隐藏路径]");
  }

  for (const pattern of secretPatterns) {
    const regex = new RegExp(pattern.regex.source, pattern.regex.flags);
    masked = masked.replace(regex, "[已隐藏密钥]");
  }

  return masked;
}
