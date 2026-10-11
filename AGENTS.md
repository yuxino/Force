# Force

个人私有 macOS 应用，使用 Wails 2、Go、React/TypeScript。延续黑白灰视觉；只给负向、错误、离线等明确状态使用语义色。

- 分支按实际改动命名：`feat/...`、`fix/...`、`docs/...`。小改验证后合并 main；大改需用户确认是否合并。
- 专注任务与 Nichijou 共用记录。计时口径见 `docs/plans/2026-10-12-force-design.md`；不要擅自改为单任务、睡眠自动暂停或并行去重。
- 不为测试修改真实专注记录。写入验收使用 `scripts/fixture_server.py`；真实服务只读验证。
- 保持 `app.yuxino.force`、稳定安装路径和现有私钥签名身份。只运行经过项目构建脚本签名的原生包。
- `internal/notch` 是 AppKit 桥。保持 WailsWindow 的运行时类，不对 NSWindow 使用 object_setClass；macOS 27 的计算属性元数据会因此在启动时断言失败。
- 界面状态成功同步后才允许写入；停止操作是明确的停止意图，不能根据远端最新状态反向启动任务。
- UI 验收使用 ego-browser / ego-lite；原生刘海和窗口行为须在实际签名包里验收。
- 刘海布局先核对官方演示与实现，具体依据见 `docs/notch-interaction-research.md`。保持轮廓连续变形与菜单栏避让，不用矩形跳变代替衔接。
- 不提交用户记录、凭据、安装包、node_modules 或构建输出。完成后清理本次一次性环境和过期产物，保留源码、当前应用及必要证据。
