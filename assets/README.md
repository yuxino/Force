# Force 品牌资产

黑猫坐姿标志，使用 `#141414` 与纯白 `#FFFFFF`。应用图标为白底黑猫；README 字标根据明暗主题切换。

- `logo.svg`：透明矢量标记，64 × 64 坐标，无字体依赖。
- `wordmark.svg` / `wordmark-dark.svg`：黑猫与 FORCE 字标，浅色和深色版本共用相同路径。
- `icon.svg`：1024 × 1024 应用图标母版，圆角白底之外透明。
- `icon.png` / `Force.icns` / `Force.ico`：从 `icon.svg` 导出的平台资源。PNG 为 1024 × 1024；ICO 包含 16、32、48、256 px。
- `readme-cover.png` / `readme-cover-dark.png`：1774 × 887 的 README 封面插画。薄纸逐渐累积成台阶，黑猫安坐高处；采用黑白版画质感，以现有猫咪图标作为生图参考。两版均为透明背景；浅色版为白纸黑线，深色版为灰色纸页和带细轮廓的黑猫。文案独立排在图片下方，便于阅读与复制。

修改轮廓时，同步 `logo.svg`、两个字标和 `icon.svg` 中的猫形路径。在仓库根目录运行 `swift scripts/generate-icons.swift`，即可通过系统矢量渲染重新生成 PNG、ICNS 和 ICO，不再另存一套绘制坐标。

README 的明暗图片通过链接上的 `#gh-light-mode-only` / `#gh-dark-mode-only` 跟随 GitHub 页面主题。验收需覆盖系统主题与 GitHub 主题相反的组合；仅检查本地 `prefers-color-scheme` 不能证明 GitHub 显示正确。
