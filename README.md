<p align="center"><img src="assets/icon.svg" width="88" alt="Force" /></p>

# Force

一个使用 Rust + GPUI 构建的桌面专注计时器，离线可用，数据保存在本机。

- 正计时与番茄钟，支持暂停、继续和重启恢复。
- 任务标签与并行会话。
- 每日记录、专注统计、每日目标与 CSV 导出。
- 紧凑计时窗口；macOS 和 Windows 可置顶。

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
