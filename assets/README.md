# Force logo

几何 F，使用 `#141414` 与 `#FAFAF8`。

- `logo.svg`：透明标记，64 × 64 坐标。
- `wordmark.svg`：矢量字标，无字体依赖。
- `icon.svg` / `icon.png`：应用图标，PNG 为 1024 × 1024。
- `Force.icns` / `Force.ico`：macOS / Windows 图标；ICO 包含 16、32、48、256 px。

macOS 运行 `swift scripts/generate-icons.swift` 重新生成平台图标。修改形状时，同步 SVG 与脚本的路径坐标。
