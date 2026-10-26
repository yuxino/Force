# 发布

版本以根 Cargo.toml 的 workspace.package.version 为准，Cargo.lock 中的 force-app 和 force-core 版本须一致。发布说明放在 docs/releases/v<版本>.md。

先完成 scripts/check.sh、UI 回归和当前平台原生检查，再合并 main。将 v<版本> 标签指向已检查的 main 提交并推送，Release 工作流会构建 Apple 芯片/Intel Mac、Windows x86_64 和 Linux x86_64 安装包。四个包完整后才发布 GitHub Release，附 SHA256SUMS。失败任务修复后可用现有标签手动重跑；不要移动已发布标签。

macOS CI 使用临时签名而不需要私钥，安装包未公证。当前本机正式应用仍使用原有私钥签名和 bundle identity。应用只在用户点击“检查更新”时访问公开 GitHub Release，不上传任务或记录。下载按钮只接受当前仓库、版本和系统对应的完整安装包地址；安装由用户完成。
