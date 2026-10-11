# 刘海衔接研究

2026-10-12。依据官方产品页面与开源实现；没有安装竞品，也没有把官网展示当作本机验收。

## 观察

| 参考 | 有依据的做法 | 对 Force 的取舍 |
| --- | --- | --- |
| [Boring Notch](https://github.com/TheBoredTeam/boring.notch/blob/main/boringNotch/ContentView.swift) | 黑色轮廓、尺寸与圆角连续变化；内容从顶部缩放并淡入；进入悬停延迟可配置，离开也有延迟 | 使用同一个轮廓连续展开，保持顶部锚点；短暂路过不弹出 |
| [DynamicNotchKit](https://github.com/MrKai77/DynamicNotchKit/blob/main/Sources/DynamicNotchKit/Views/NotchView.swift) | compact 与 expanded 在同一画布转换；为物理刘海保留内容安全区；没有刘海时使用独立 floating 样式 | 物理刘海与无刘海显示器分别处理；保持画布稳定，避免先跳变窗口再播放内容动画 |
| [DynamicLake 计时](https://www.dynamiclake.com/blog/master-the-new-timer-in-dynamic-lake) | 在刘海中持续显示计时状态，支持多个计时器 | 收起形态只保留任务和时间；延续 Nichijou 并行记录，不改为番茄钟倒计时 |
| [NotchNook](https://getnotchnook.com/) | 功能集中在屏幕顶部，官方说明覆盖无刘海与多显示器 | 参考顶部集中入口；本次旧官网无法加载，未凭空归纳其动画参数 |

进一步查看了 [Boring Notch 形状](https://github.com/TheBoredTeam/boring.notch/blob/main/boringNotch/components/Notch/NotchShape.swift)、[尺寸测量](https://github.com/TheBoredTeam/boring.notch/blob/main/boringNotch/sizing/matters.swift) 与 [DynamicNotchKit 过渡配置](https://github.com/MrKai77/DynamicNotchKit/blob/main/Sources/DynamicNotchKit/DynamicNotch/DynamicNotchTransitionConfiguration.swift)。仅研究通用交互原理，没有复制竞品源码或资产。

## Force 的调整

原来的问题来自两个实现选择：接近黑色的背景与硬件黑色之间有接缝；窗口立即从窄条跳成矩形，使展开面板像另贴上去的窗口。

改为纯黑的连续轮廓。收起时只在刘海下沿保留 22 点高的状态条；展开时中间颈部保持原宽度，在菜单栏下方以弧线过渡到面板。尺寸、轮廓和内容一起过渡，正文在尺寸变化后淡入，文字本身不随轮廓拉伸。原生窗口使用固定透明画布，透明区域不拦截底下应用的点击。

成熟作品的两翼布局通常会扩展到菜单栏。Force 已有“不遮挡托盘”的明确反馈，因此没有照搬该布局：整个应用窗口仍在菜单栏下方，菜单栏两侧不进入应用的绘制区域。这是针对 Force 的设计取舍，并非声称所有竞品都这样处理。

悬停 400 毫秒展开；通过悬停打开时，离开 500 毫秒后收起。点击打开或点击面板中的控件后保持，点击外部或 Escape 收起；编辑标签和选择选项时不会因短暂离开而收起。保留可滚动内容，隐藏常驻滚动条。没有刘海的屏幕采用圆角小条，不模拟硬件连接。

本机验证范围与尚未验证的场景记录在 [验收记录](verification.md)。
