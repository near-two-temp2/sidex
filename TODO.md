# TODO — 最简多文件夹 Workspace 支持

目标：**文件树（资源管理器）能同时显示多个根文件夹**，就这一个核心能力。
不追求 VS Code 完整的多根工作区体验（设置合并、信任模型、工作区编辑器等一概不做）。

## 现状盘点

| 部件 | 状态 |
|---|---|
| Rust `crates/sidex-workspace/src/multi_root.rs` | ✅ 已实现 `.code-workspace` / `.sidex-workspace` 解析与管理，**但没有暴露为 Tauri 命令** |
| 前端入口 `src/main.ts` | ❌ 只处理 `?folder=` 单文件夹（`{ folderUri }`）；`workspaceProvider.open` 收到 workspace 文件请求时直接忽略 |
| VS Code workbench UI（`src/vs/`） | ✅ 多根资源管理器、"Open Workspace from File..." / "Add Folder to Workspace..." 命令、`.code-workspace` 概念全部自带（继承自 VS Code） |
| 文件系统桥 `SideXFileSystemProvider` | ⚠️ 待验证：workbench 的 workspace 服务需要通过它读取 `.code-workspace` 文件内容 |

结论：楼的地基和楼顶都在，缺的是中间两层接线。

## Phase 1 — 能打开多根工作区（核心，先做）

- [ ] `src/main.ts`：支持 `?workspace=<uri>` 启动参数 → `workspace = { workspaceUri: URI.parse(...) }`（对照现有 `?folder=` 的写法）
- [ ] `src/main.ts` `workspaceProvider.open`：处理 `'workspaceUri' in _workspace` 分支 → 带 `workspace` 参数重载页面（对照现有 `navigateToFolder`）
- [ ] 验证 workbench 能通过 `SideXFileSystemProvider` 读到 `.code-workspace` 文件并解析出 `folders` 列表（**关键验证点**，Phase 1 成败在此）
- [ ] File 菜单 "Open Workspace from File..."：接 Tauri dialog（`@tauri-apps/plugin-dialog`，过滤 `.code-workspace` / `.sidex-workspace`）→ 走上面的跳转
- [ ] **验收标准**：手写一个含两个 `folders` 的 `.code-workspace` 文件，打开后文件树显示两个根，两个根下的文件都能打开/编辑/保存

## Phase 2 — 基本的工作区编辑

- [ ] "Add Folder to Workspace..."：
  - 当前是单文件夹 → 提示保存为 `.code-workspace` 文件（选路径）后加入第二个根并重载
  - 当前已是 workspace → 改写 `.code-workspace` 的 `folders` 数组并重载
- [ ] "Save Workspace As..." 命令接线
- [ ] "Remove Folder from Workspace"（资源管理器根节点右键菜单）
- [ ] 读写 `.code-workspace` 用前端已有的 JSONC 解析器（VS Code 自带，注意文件里允许注释，不能用裸 `JSON.parse`）；如需 Rust 侧能力再把 `multi_root.rs` 的 parse/save 包成 Tauri 命令

## Phase 3 — 跨根功能巡检（风险区，逐项验证）

- [ ] 全局搜索：确认 Rust 搜索命令是否假定单根；多根时按根循环调用或改造
- [ ] 文件监听（watcher）：每个根都要有变更通知
- [ ] Git/SCM：每个根各自的仓库要在源代码管理视图分别显示
- [ ] 终端默认 cwd / 任务 / LSP 根目录：多根时取第一个根作为兜底，不崩即可

## 明确不做（超出"最简"范围）

- workspace 级设置与文件夹级设置的合并
- 工作区信任（trust）UI 流程
- 远程 / 虚拟文件系统 workspace
- "最近打开的工作区" 列表
- `.code-workspace` 的图形化编辑器

## 已知风险

1. workbench 在 web 模式下解析 `workspaceUri` 的链路较深（workspace 服务 → configuration 服务 → FS provider），Phase 1 第 3 项验证不通过的话，需要在 boot 阶段自行解析 workspace 文件、把 folders 注入 workspaceProvider——仍然可行，工作量 +1 天左右。
2. Rust 后端各命令（搜索/监听/Git）如果普遍硬编码了单根假设，Phase 3 的工作量会显著上涨；建议 Phase 1 做完先实测再评估。

## 签名与分发（独立于 workspace，按需推进）

现状：fork 构建为 ad-hoc 签名（`signingIdentity: "-"`），用户需 `xattr -cr` 或右键打开放行。

- [ ] 短期（免费）：在 fork 发 GitHub Release，说明写成 curl 一键安装（curl 下载不带 quarantine 标记，双击即开）：
      `curl -L -o ~/Downloads/SideX.dmg "<Release 直链>" && open ~/Downloads/SideX.dmg`
- [ ] 长期（99 美元/年，如需对外正式分发）：注册 Apple Developer Program →
  - [ ] 导出 Developer ID Application 证书 p12，连同 Apple ID 凭据存入 fork 的 GitHub secrets
  - [ ] 把上游 release.yml 的签名步骤（Import Apple signing certificate / Unlock keychain）搬回 build-macos12.yml
  - [ ] `tauri.macos12.conf.json` 的 `signingIdentity` 改为自己的证书名，并启用公证（APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID 环境变量，Tauri 自动走 notarytool）
- 注意：绝不接受 app 内自动更新（指向官方 CDN，官方版在 macOS 12 上不可用）；正式分发前应在 fork 中禁用 updater 或换成自己的更新源

## 参考

- 上游没有任何公开分支/PR 实现此功能（截至 2026-07-24 已核查全部 30 个 PR 与 2 个分支）
- 相关 issue：上游 #31（拖文件夹进工作区无效，维护者回复"用 open folder"）
- 本 fork：https://github.com/near-two-temp2/sidex （分支 `macos12-support`）
