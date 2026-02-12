# update-changelog

更新 CHANGELOG.md，遵循项目 CLAUDE.md 中定义的版本管理规范。

## 工作流程

### 第一步：确认版本状态

执行以下命令获取当前版本信息：

```bash
# 查看最近的 tag
git log --oneline --decorate -10 | grep tag

# 查看已发布版本的 CHANGELOG 内容
git show <tag-commit>:CHANGELOG.md | head -50
```

### 第二步：读取当前 CHANGELOG.md

读取 CHANGELOG.md 的头部，了解当前 Unreleased 段落和最新已发布版本。

### 第三步：分析待记录的改动

检查自上次版本以来的改动：

```bash
# 查看自上次 tag 以来的提交
git log <latest-tag>..HEAD --oneline
```

### 第四步：添加条目

按以下规则添加到 CHANGELOG.md：

- 新改动添加到最顶部的 `## [x.y.z Unreleased]` 段落
- 如果不存在 Unreleased 段落，创建一个（版本号 = 最新已发布版本的下一个合理版本）
- 使用标准分类：`### Added`、`### Changed`、`### Fixed`、`### Removed`
- 每条记录格式：`- **模块名**: 改动描述`
- 不要修改已发布版本（有日期的）的内容

### 格式参考

```markdown
## [x.y.z Unreleased]

### Added
- **Module**: Description

### Changed
- **Module**: Description

### Fixed
- **Module**: Description
```

## 注意事项

- 先询问用户要添加什么改动，或者根据 git log 自动分析
- 模块名使用项目中的 crate 或组件名（如 OML、wp-lang、Engine Config 等）
- 描述使用英文，简洁明了
