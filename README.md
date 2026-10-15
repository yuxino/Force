<p align="center"><img src="assets/icon.svg" width="88" alt="Force" /></p>

# Force

一个使用 Rust + GPUI 构建的悬浮专注计时器。轻巧常驻，离线可用，数据保存在本机。

- 悬浮计时条，点开设置任务、时长和标签。
- 倒计时与自由计时，支持暂停、继续和重启恢复。
- 同一任务可分多轮专注，到时可继续或休息。
- 每日记录、专注统计与 CSV 导出；macOS 和 Windows 支持置顶。

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

CI 配置覆盖 macOS、Windows 和 Ubuntu。Windows 与 Linux 尚未完成实机交互验收，最低系统版本仍待验证；Linux 置顶由窗口管理器控制。

[MIT License](LICENSE)
