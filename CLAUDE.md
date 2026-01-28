# WP-Motor 项目开发规范

## CHANGELOG 更新流程

更新 CHANGELOG.md 时，遵循以下步骤：

### 1. 确认当前版本状态

```bash
# 查看最近的 tag
git log --oneline --decorate -10 | grep tag

# 查看已发布版本的 CHANGELOG 内容
git show <tag-commit>:CHANGELOG.md | head -50
```

### 2. 版本分类原则

- **已发布版本**：有 git tag 的版本，格式为 `## [x.y.z] - YYYY-MM-DD`
- **未发布版本**：当前开发中的改动，格式为 `## [x.y.z Unreleased]`

### 3. 更新步骤

1. **检查 git tag**：确认最新已发布的版本号
2. **获取发布日期**：`git show -s --format='%ci' <tag-commit>`
3. **分离内容**：
   - 已发布版本的改动保留在对应版本段落，添加发布日期
   - 新改动归入下一个 Unreleased 版本
4. **保持格式一致**：
   - 使用 `### Changed`、`### Added`、`### Fixed`、`### Removed` 等标准分类
   - 每条记录以 `- **模块名**:` 开头，描述改动内容

### 4. 示例结构

```markdown
## [1.10.5 Unreleased]

### Changed
- **Module A**: Description of change

### Fixed
- **Module B**: Description of fix


## [1.10.4] - 2026-01-27

### Changed
- **Module C**: Description of change
```

### 5. 注意事项

- 发布新版本时，将 `Unreleased` 改为具体日期
- 新开发的改动始终添加到最顶部的 Unreleased 版本
- 不要将新改动混入已发布版本的记录中
