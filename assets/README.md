# Force 品牌资产

自然、略显呆萌的海豹重点色暹罗猫。奶油色短毛、深褐色重点色与灰蓝眼睛；正常的猫咪比例和安静神态。毛色仅用于角色，应用界面继续使用黑、白和中性灰。

- `siamese-cat.png`：坐姿角色画稿。`siamese-blink.png` 是同一构图的闭眼画稿；动画只显示闭眼图的眼部局部，身体保持固定。
- `siamese-portrait.png`：同角色的头部画稿，用于小标志与应用图标。
- `cat-clock.png` / `cat-blink.png`：按相同取景框导出的 304 × 448 图像，在猫钟中按 76 × 112 点显示。使用保色图像渲染，不能当作单色图标蒙版。
- `logo.png`：方形头像。`wordmark.png` / `wordmark-dark.png`：头像与原有 FORCE 几何字形；明暗版只切换字色。
- `wordmark-lettering.svg`：原有 FORCE 字形路径，不依赖字体。
- `icon.png` / `Force.icns` / `Force.ico`：头像配白色圆角底的应用图标；ICO 包含 16、32、48、256 px。
- `cat-side-expanded-clock.svg` / `cat-side-expanded-clock-outline.svg`：独立的中性深色控制面板及轮廓，展开时不拉伸猫的身体。
- `readme-cover.png` / `readme-cover-dark.png`：以桌面猫钟的坐姿角色为参考的封面，配两行现代黑体引句。README 不额外重复 Logo 或句子。

角色和封面画稿由图像生成工具制作。修改角色时更新源画稿，再运行以下命令导出应用资源：

```sh
swift scripts/generate-brand-assets.swift
swift scripts/generate-icons.swift
```

第一步统一取景与尺寸并生成字标，第二步导出平台图标；不手工重画派生图。眨眼画稿需与坐姿画稿保持相同尺寸和位置，检查眼部覆盖与计时文字在真实猫钟里的效果。封面以坐姿原稿为参考，保留短毛、脸型、灰蓝眼睛、深色前爪与右侧卷尾；不另画不同体型或笔触的猫。明暗版保持同一句文案与排版。

README 明暗图片通过链接上的 `#gh-light-mode-only` / `#gh-dark-mode-only` 跟随 GitHub 页面主题。验收需覆盖系统主题与 GitHub 主题相反的组合；仅检查本地 `prefers-color-scheme` 不能证明 GitHub 显示正确。
