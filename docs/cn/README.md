# klean

安全高效的开发环境清理 CLI 工具。

## 安装

**Homebrew（macOS/Linux）**
```sh
brew tap klean-cli/klean
brew install klean
```

**Cargo**
```sh
cargo install klean
```

**Scoop（Windows）**
```sh
scoop bucket add klean https://github.com/klean-cli/scoop-bucket
scoop install klean
```

**直接下载** — 从 [Releases](https://github.com/danil0ws/klean/releases) 页面获取最新二进制文件。

## 使用方法

```
klean                   # 交互式菜单（TUI）
klean .                 # 扫描当前目录
klean ~/projects        # 扫描指定路径
klean globals           # 管理全局安装的软件包
klean update            # 自动更新到最新版本
klean undo              # 从回收站恢复最近删除的会话
klean trash             # 列出回收站中的会话
klean trash --empty     # 永久清空回收站
klean stats             # 显示清理历史
```

### 交互式菜单

不带参数运行 `klean` 将打开 TUI 菜单：

| # | 操作 |
|---|------|
| 1 | 清理构件 — 删除构建缓存、node_modules、dist 等 |
| 2 | 全局包 — 管理 npm/cargo/pip/brew/二进制文件 |
| 3 | 统计信息 — 清理历史 |
| 4 | 更新 *（仅在有新版本时显示）* |
| 4/5 | 退出 |

使用 `↑↓` 或 `jk` 导航，`Enter` 或数字键确认。

### 全局包管理

`u` — 检查过时的包（版本列会显示更新标记）。  
`空格` — 选择，`a` — 全选，`Enter` — 删除选中项。  
`s` — 在管理器和大小之间切换排序。

## 清理内容

klean 删除：`node_modules`、`target`、`.cache`、`dist`、`build`、`.next`、
`.nuxt`、`__pycache__`、`.venv`、`vendor` 等 — 支持 20 多种语言和框架。
可通过 `klean.toml` 进行项目级配置。

## 配置（`klean.toml`）

在项目根目录放置 `klean.toml` 来自定义行为：

```toml
[project]
backup = true          # 移动到回收站而非直接删除
backup_dir = ".trash"  # 自定义回收站位置

[[ignore]]
path = "dist"          # 不处理此目录
```

## 自动更新

```sh
klean update           # 更新到最新版本
klean update --dry-run # 仅预览，不实际更新
```

klean 从 crates.io 检查最新版本，并通过 `cargo install` 安装（如果 cargo 可用），
否则从 GitHub Releases 下载预编译二进制文件。

## 许可证

MIT OR Apache-2.0
