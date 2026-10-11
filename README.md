# Force

把时间放在眼前。

一个浮在工作窗口上方的专注计时应用。标签、正在进行的任务和历史记录与 Nichijou 共用。

当前分支是悬浮形态试用版，尚未合并 main。

- 小计时条显示时间和当前任务，点击展开专注面板
- 拖动左侧手柄或面板标题调整位置，重启后保留
- 小条可收进菜单栏；菜单提供显示和重置位置
- 按标签记录时间，支持同时记录多项任务
- 今日 5 小时目标只累计正向标签
- 日历记录和本周统计在独立窗口查看
- 未计时或负向计时过久时展开提醒，可关闭或调整间隔
- 标签单独滚动，计时和今日目标保持可见

点击正在计时的标签，会结束这一段并保存记录。退出应用和电脑睡眠不会停止服务器计时；结束前请先停止任务。

首版面向个人使用，固定连接 Nichijou 现有专注记录。支持 Apple Silicon、macOS 13 及以上。

## 开发

Wails 2.15.0、Go 1.25+、Node.js 22.12+ 和 Xcode 命令行工具。

```sh
go install github.com/wailsapp/wails/v2/cmd/wails@v2.15.0
cd frontend && npm ci && cd ..
```

检查与签名构建：

```sh
go test -race ./...
cd frontend && npm test && npm run build && cd ..
FORCE_SIGN_IDENTITY='已有签名身份的名称或 SHA-1' zsh scripts/build-macos.sh
```

构建脚本要求已有私钥签名身份，产物在 `build/bin/Force.app`。退出旧版本后复制到固定的 `/Applications/Force.app` 再运行。本地开发包未做公证。

用合成数据验收界面：

```sh
python3 scripts/fixture_server.py
# 另一个终端
cd frontend && npm run dev
# 打开 http://127.0.0.1:3415/?fixture
```

已签名的原生应用也可连接本地测试服务：

```sh
build/bin/Force.app/Contents/MacOS/Force --api-base-url http://127.0.0.1:3459 --inspect-window
```

`--inspect-window` 只输出原生窗口几何，方便核对尺寸与重启位置。`--view history` 或 `--view week` 可在首次启动时打开记录窗口。

[验收记录](docs/verification.md) · [计时口径与设计](docs/plans/2026-10-12-force-design.md) · [悬浮形态依据](docs/floating-interaction-research.md) · [早期刘海研究](docs/notch-interaction-research.md)
