# 灵栖 Lingqi · 阶段小结

> 本文件记录灵栖相对上游「岛仔」做了什么、为什么这么做、实测数据如何，以及下一步要做什么。
> 目标读者：接手这个项目的人（含未来的我自己）。

## 1. 定位

上游「岛仔」是一台**信息聚合岛**：邮件、日历、AI 额度、音乐歌词、蓝牙、消息、天气什么都有。
灵栖要做的是**桌宠 + 灵动岛 + AI 助手**：功能不必比它多，但**性能与体验要比它强**，并且能真正用起来。

三条产品决策：

1. **只留用得上的**：剪贴板、系统状态、天气、计时待办、AI。邮件/日历同步/各家 AI 额度/音乐歌词/蓝牙/消息通知/插件市集（约 41k 行）逐步下线。
2. **吉祥物不靠美术素材**：改成**程序生成的精灵**（纯几何 + 时间轴），既是原创资产，也省掉了 16 帧图集。
3. **AI 是主角**：在灵动岛里用一句话办事，模型只做分类与参数抽取，真实数据由本机执行器给出。

## 2. 已完成的改动

| 改动 | 文件 | 说明 |
| --- | --- | --- |
| 改名与身份 | `Cargo.toml` `app.rc` `src/model.rs` `src/tray.rs` `installer/*` `scripts/package.ps1` | 包名/二进制名/数据目录/互斥体/窗口类名/更新源全部换成灵栖；删除指向原厂夸克网盘的兜底链接 |
| 图标与 Logo | `assets/lingqi.ico` `assets/lingqi-logo.png` | 玻璃罩里的薄荷精灵，与程序化精灵同一套形象 |
| 原厂收款码 | `src/extras.rs` `src/supporters.rs` `assets/support/` | 「支持作者」区块（原厂微信/支付宝收款码与远端感谢墙）整块移除 |
| AI 助手 | `src/ai/*` | 流式对话（SSE）、凭据管理器、上下文采集、提示词、意图、抽屉界面 |
| AI 设置与入口 | `src/extras.rs` `src/tray.rs` `src/main.rs` | 设置页「AI 助手」分页、托盘菜单入口 |
| 参数化精灵 | `src/sprite.rs` `src/app.rs` `src/paint.rs` | 纯 epaint 形状 + 时间轴；新增 `Card::add_shape` 注入接口 |
| 吉祥物形象设置 | `src/model.rs` `src/extras.rs` | 「图集 / 程序生成」可切换 |
| 网络出口独立 | `src/net.rs` | `http_agent` / `system_proxy` 从 AI 额度模块抽出，天气与更新不再反向依赖将被砍掉的模块 |
| DirectComposition | `src/dcomp.rs` `src/graphics.rs` `Cargo.toml` | 上游把透明直绘锁在 `sdk-material` 实验特性后；灵栖把它变成正式能力并设为 Windows 默认后端 |
| 低占用重绘 | `src/app.rs` `src/main.rs` | 精灵静止后岛体降到 2 帧；帧间等待按实际需要自适应 |
| README | `README.md` | 灵栖自己的产品说明 |

## 3. 性能：问题出在哪

### 3.1 三个真实原因

1. **永远在重绘**：上游的桌宠一直在呼吸，岛体因此永远以 20 帧重绘（`src/app.rs` 的 `repaint_after`）。
2. **忙轮询**：`vendor/eframe/src/native/run.rs` 里 `wait_when_idle` 只在 **DComp 表面存在**时为真，否则事件循环 `ControlFlow::Poll` 一直转。
3. **每帧固定睡 1/60**：应用内部写死帧间隔，安静时也不停。

### 3.2 实测（同一台机器，Windows 11，Intel 核显）

| 场景 | 单核占用 | 内存 |
| --- | --- | --- |
| 上游默认（OpenGL + 分层窗口读回） | 12.9% | 138 MB |
| 仅切换到 DX12（仍 20 帧重绘） | 19.8% | 183 MB |
| 灵栖（DX12 + DComp + 精灵会休息 + 自适应等待） | 见下方「复测」 | |

> 结论：**光换后端不解决问题**。真正的收益来自「没有变化就不重画」+「事件循环能睡着」这两件事的组合。

### 3.3 复测

（构建完成后填入）

## 4. 关键架构决策

### 4.1 为什么保留 epaint 而不是整套重写

`vendor/egui-wgpu/src/renderer.rs` 的 `update_buffers` 吃的是 `&[epaint::ClippedPrimitive]`——**渲染器只依赖 epaint，不依赖 egui 的 UI 框架**。而 epaint 自带 tessellator、字体与排版。
所以「换渲染引擎」可以做成增量替换：抽象一个 `trait Painter`，egui 实现给设置页用，自写 wgpu 实现给岛体用。

已验证的原型：`D:\tools\Daozai\_tmp\engine-spike\src\main.rs`（epaint + 自写 wgpu 渲染器，离屏渲染灵动岛形变，CPU 0.095 ms/帧）。

### 4.2 为什么精灵要输出 epaint 形状

精灵是程序生成的，画出来就是几何。如果它直接依赖 egui 的 widget，换渲染引擎时就得重写；
输出 `epaint::Shape` 则两个后端都能消费。`Card::add_shape` 就是这条缝。

### 4.3 AI 的分工

模型只做**分类与参数抽取**（输出一个 JSON），天气、电量、性能、播放信息一律由本地执行器用真实读数作答，避免模型把本机读数编错。写操作（加待办、设提醒）默认禁止，可在设置里放开。

## 5. 已知限制与风险

| 项 | 说明 |
| --- | --- |
| AI 未做真机联调 | 本机没有任何 DeepSeek 凭据，流式链路只有单元测试覆盖；真实调用需要用户填 Key |
| DX12 为默认后端 | 万一某台机器 DX12 有问题，用 `LINGQI_RENDERER=opengl` 退回 OpenGL |
| vendor 补丁不可丢 | `vendor/wgpu-hal` 与 `vendor/egui-wgpu` 里的 DirectComposition 补丁是透明合成的前提，升级依赖必须重放 |
| 剪贴板上下文默认关闭 | 打开后仍会过滤疑似密钥的内容，但默认不送 |
| 瘦身未完成 | 41k 行业务代码仍在树里，只是暂时不接线 |
| 测试基线 | 本机有 9 个上游自带的失败用例（需要真实 QQ 客户端、任务栏、凭据管理器、真实窗口命中），与灵栖无关，已用原版仓库 A/B 对照确认 |

## 6. 下一步

1. **多时区**：常驻栏同时显示多个时区的钟（`chrono-tz` 已在依赖里）。
2. **瘦身第一批**：邮件 + 日历（同时移除 `imap`/`imap-proto` 两个 vendor 补丁依赖）。
3. **意图执行器落地**：把 `intents.rs` 里定义的十个意图接到真实动作（现在只解析不执行）。
4. **自研渲染器**：按 `src/render/` 的分阶段计划落地岛体渲染，与 egui 后端并存可切换。
5. **安装包与更新链路**：`scripts/package.ps1` + NSIS 换成灵栖身份，GitHub Releases 资产名与 `src/updates.rs` 对齐。
