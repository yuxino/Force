<p align="center">
  <a href="assets/readme-cover.png#gh-light-mode-only">
    <img src="assets/readme-cover.png" width="100%" alt="Force — 时间会把微小的坚持，变成意想不到的改变。下方小字：鸡蛋从外面打破是食物，从里面打破才是新生。我的未来居然全部取决于我自己吗？会不会有点太不靠谱了？右侧是一只奶油色短毛、灰蓝眼睛的坐姿暹罗猫。" />
  </a>
  <a href="assets/readme-cover-dark.png#gh-dark-mode-only">
    <img src="assets/readme-cover-dark.png" width="100%" alt="Force — 时间会把微小的坚持，变成意想不到的改变。下方小字：鸡蛋从外面打破是食物，从里面打破才是新生。我的未来居然全部取决于我自己吗？会不会有点太不靠谱了？右侧是一只奶油色短毛、灰蓝眼睛的坐姿暹罗猫。" />
  </a>
</p>

Force 是一款本地优先的桌面日历与计时工具，用 Rust 和 GPUI 构建。一只独立悬浮的猫钟陪你计时，主窗口用来安排任务、回看记录和了解时间分配。

无需账号，日常使用可离线完成，任务和记录保存在自己的电脑上。

[下载最新版](https://github.com/yuxino/Force/releases/latest) · [更新说明](https://github.com/yuxino/Force/releases) · [反馈问题](https://github.com/yuxino/Force/issues)

## 可以做什么

- **猫钟计时**：正计时或倒计时，随时暂停、继续。macOS 和 Windows 可将猫钟拖到屏幕左右边缘，收起为猫爪或计时标签，点击展开控制。
- **日历安排**：在同一天查看计划、实际计时、任务完成和随手记。计划与实际记录分开保存，随手记不计入时长。
- **记录回看**：通过活动年历和日期查找记录，按标题、活动或分类筛选；支持补记、修改已结束的计时，以及导出 CSV。
- **时间分析**：查看近 7、30、90 天的专注趋势、活动投入和每日明细；可在设置中设定每日专注目标。
- **活动管理**：为活动设置分类和显示颜色，让计时、日历和统计使用同一套活动信息。

## 下载与安装

在 [GitHub Releases](https://github.com/yuxino/Force/releases/latest) 中选择对应系统的文件：

| 系统 | 文件名后缀 | 安装方式与要求 |
| --- | --- | --- |
| macOS · Apple 芯片 | `macos-aarch64.dmg` | macOS 15 或更新版本；打开后将 Force 拖入应用程序文件夹 |
| macOS · Intel | `macos-x86_64.dmg` | macOS 15 或更新版本；打开后将 Force 拖入应用程序文件夹 |
| Windows · x86_64 | `windows-x86_64.zip` | 解压后运行 `Force.exe`，需要 [Visual C++ v14 x64 运行库](https://learn.microsoft.com/zh-cn/cpp/windows/latest-supported-vc-redist) |
| Linux · x86_64 | `linux-x86_64.tar.gz` | 解压后运行 `force`；基于 Ubuntu 24.04 构建，需要系统图形、音频库及可用图形驱动 |

GitHub 上的 macOS 安装包未公证。Linux 目前不支持猫钟贴边收纳，置顶由窗口管理器控制，关闭主窗口时会最小化。

更新前退出 Force，再替换应用，原有任务和记录会保留。从 v0.1.2 或更早版本升级会更新本地数据格式，之后这些旧版本无法读取；建议更新前备份本地数据。

## 开始使用

1. 在设置中添加常用活动。点击“新任务”，选择活动，按需填写说明，选择正计时或倒计时，然后点击“开始计时”。也可以先“保存任务”，稍后再开始。
2. 在猫钟或任务页面暂停、继续或结束计时。“结束计时”保存这一段投入；同一任务可以“再计一次”。需要结束任务时，单独选择“完成任务”。
3. 在“日历”中安排和回看一天，在“记录”中补记或修正已结束的计时，在“分析”中查看时间分配。

暂停期间不累计时长；关闭管理窗口不会停止计时，重启后可恢复未结束的任务。电脑睡眠不会自动暂停计时，需要休息时请先暂停。

以下快捷键在应用内使用，macOS 使用 `Cmd`，Windows / Linux 使用 `Ctrl`：

| 操作 | 快捷键 |
| --- | --- |
| 新任务 | `Cmd/Ctrl + N` |
| 随手记 | `Cmd/Ctrl + Shift + N` |
| 开始 / 暂停 / 继续 | `Cmd/Ctrl + P` |
| 结束本次计时 | `Cmd/Ctrl + Shift + P` |
| 显示 / 隐藏主窗口 | `Cmd/Ctrl + Shift + M` |
| 设置 | `Cmd/Ctrl + ,` |

## 数据与隐私

数据保存在本机 SQLite 数据库中，无云同步。设置中可“打开数据文件夹”；完整备份时，先退出 Force，再复制该文件夹。记录页的 CSV 导出用于查看计时会话，不包含全部日历安排和随手记，不能代替完整备份。

只有主动点击“检查更新”时，应用才会请求公开的 GitHub Releases，不发送本地任务、记录或路径。新版需下载后手动安装。

## 本地开发

需要稳定版 Rust。macOS 还需安装并选用完整 Xcode（含 macOS / Metal 工具链），并准备可用的代码签名身份；Windows 需要 MSVC C++ 构建工具。Debian / Ubuntu 可先运行 `./scripts/linux-deps.sh` 安装系统依赖，其他发行版需安装对应的软件包。

```sh
git clone https://github.com/yuxino/Force.git
cd Force
```

macOS 使用独立的开发应用与数据目录：

```sh
./scripts/build-macos.sh
open "build/Force Dev.app"
```

脚本默认选择唯一的 Apple Development 签名身份，也可通过 `FORCE_SIGN_IDENTITY` 指定现有身份。

Windows / Linux：

```sh
cargo run --bin force --locked
```

提交前运行 `./scripts/check.sh`。核心持久化和计时逻辑位于 `crates/force-core`，原生界面和平台适配位于 `crates/force-app`。发布流程见 [发布指南](docs/releases/README.md)。

## 许可证

[MIT](LICENSE)
