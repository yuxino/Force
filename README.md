<p align="center"><img src="assets/icon.svg" width="88" alt="Force" /></p>

# Force

一个使用 Rust + GPUI 构建的悬浮专注计时器。轻巧常驻，离线可用，数据保存在本机。

- 328 × 68 点悬浮计时条，搭配独立、可调整大小的管理窗口。
- 专注工作台保留今日进度与任务列表；查看任务和开始计时分开操作，开始后窗口保持原样。
- 倒计时与自由计时，支持暂停、继续和重启恢复；同一任务可分多轮专注。
- 记录页用灰阶活动格展示过去一年的真实专注，点选日期查看明细；支持搜索和 CSV 导出。
- 设置集中管理计时条置顶、提示音、每日目标、标签和本地数据。
- 关闭管理窗口后计时继续；macOS 和 Windows 隐藏管理窗口，Linux 当前将其最小化。

## 开发

安装 Rust stable，然后运行：

```sh
cargo run --bin force --locked
./scripts/check.sh
```

| 平台 | 要求 |
| --- | --- |
| macOS 15+ | Xcode Command Line Tools、有效代码签名身份 |
| Windows 64 位 | MSVC C++ 工具链、Windows SDK、CMake、DirectX 11 GPU |
| Linux 64 位 | X11 或 Wayland、Vulkan 驱动；Debian / Ubuntu 先运行 `./scripts/linux-deps.sh` |

macOS 开发应用：

```sh
./scripts/build-macos.sh
open 'build/Force Dev.app'
```

可用 `FORCE_SIGN_IDENTITY` 选择现有签名身份，`--release` 开启优化。开发包与正式安装隔离，本地包未公证。

CI 配置覆盖 macOS、Windows 和 Ubuntu。Windows 与 Linux 尚未完成实机交互验收，最低系统版本仍待验证；Linux 置顶由窗口管理器控制，管理窗口关闭与恢复行为仍需实机验收。

[MIT License](LICENSE)
