# hdd

`hdd` 是号多多脚本整合工具的 Rust workspace。根包 `hdd` 负责 CLI 菜单、账号缓存、批量玩法、运行时路径和终端日志视图；`crates/mining` 负责挖矿控制面、CPU/GPU 后端调优、奖励码提交和结果保存；CUDA / OpenCL / Metal 原生计算核心位于 `native/`，并通过对应 `*-sys` crate 接入 Rust。

## 快速运行

源码运行：

```bash
cargo run --release --bin hdd
```

打包后运行 `dist/` 下对应平台的产物：

- `dist/hdd-win-x64.exe`
- `dist/hdd-macos-amd64`
- `dist/hdd-macos-arm64`

程序启动时会准备 `var/` 运行时目录，并尝试迁移旧版根目录数据文件。

## 功能入口

程序启动后显示：

```text
欢迎使用号多多脚本整合工具。
```

一级菜单：

1. 挖矿
2. 需要登录的多账号批量操作功能
3. 退出脚本

### 挖矿

挖矿菜单：

1. 先挖邀请码再挖余额码
2. 先挖余额码再挖邀请码
3. 只挖邀请码
4. 只挖余额码
5. 返回上一级菜单
6. 退出脚本

默认走自动调优模式，不要求用户手动选择 CPU / GPU。CLI 调用 `run_auto_tuned_with_config_and_cancel`，会保留 CPU 兜底，并在可用时选择 CPU 与最快 GPU 后端并发参与挖矿。

当前 Rust 接入的计算后端：

- CPU：纯 Rust portable fallback，所有平台可用。
- CUDA：Windows x64 原生后端；缺少 CUDA / nvcc / MSVC 等环境时降级为不可用后端。
- OpenCL：macOS 原生后端；仅 macOS target + macOS host 构建时启用。
- Metal：macOS 原生后端；仅 macOS target + macOS host 构建时启用。

挖矿运行时会输出后端探测、测速、选择、降级和运行状态。GPU 不可用、测速失败或运行期失败时会回退到 CPU 或切换到其他可用后端。

奖励码处理：

- 邀请码保存到 `var/data/mining/invite-codes.txt`。
- 余额码和并发码保存到 `var/data/mining/balance-codes.txt`。
- 提交余额类奖励后会根据接口返回区分余额兑换码和并发码。
- 余额兑换码会显示可增加多少余额；并发码会显示可增加几并发。
- 保存行包含请求的奖励类型、实际返回的奖励类型、奖励码和时间。

`MiningClient::reset_session()` 是业务约束：挖到奖励码并提交后必须重建 HTTP client/session，不能为了复用连接把它优化掉。

### 多账号批量功能

批量入口菜单：

1. 添加账号
2. 账号添加完成，选择脚本功能
3. 返回上一级菜单
4. 退出脚本

添加账号会要求输入邮箱和密码，立即登录，成功后保存账号缓存。账号缓存可能包含邮箱、密码、 bearer token、access token 和 session cookies。

功能 hub：

1. 白嫖玩法
2. 赌狗玩法
3. 返回上一级菜单
4. 退出脚本

白嫖玩法菜单：

1. 全自动完成所有白嫖玩法
2. 自动签到
3. 自动羊了个羊
4. 返回上一级菜单
5. 退出脚本

赌狗玩法菜单：

1. 自动随机刮刮乐
2. 返回上一级菜单
3. 退出脚本

批量页会刷新账号余额和状态。文件不存在或为空会初始化默认配置；损坏 JSON 等真实错误会直接提示并退出。

### 自动签到

自动签到按账号并发执行。每个账号会先尝试恢复登录态：cookies、token、密码重登依次兜底。流程会查询今日签到状态，已签到时报告失败状态，不重复领取；成功领取后按余额变化或奖励字段计算本次增加值。

日志写入：

- `var/log/checkin/checkin.log`

### 自动羊了个羊

自动羊了个羊按账号并发执行。每个账号会读取 tile 配置和账号状态，恢复未完成对局，然后按难度顺序处理剩余次数：

1. 简单
2. 普通
3. 困难
4. 地狱

当前自动流程使用棋盘快照里的固定点击队列推进：按 tile id 降序尝试，跳过陈旧点击错误，遇到槽位满等业务错误时结束该局；`solver/` 中的搜索模块目前不是自动羊了个羊主流程的执行路径。

日志写入：

- `var/log/sheepmatch/<sanitized-email>.log`

### 全自动完成所有白嫖玩法

全自动白嫖玩法当前包含：

1. 自动签到
2. 自动羊了个羊

不同账号之间并发执行；同一个账号内部先签到，再羊了个羊。账号状态会在全部线程完成后合并并保存一次。

### 自动随机刮刮乐

自动随机刮刮乐按账号并发执行。每个账号会先补结算历史未开奖轮次，然后随机选择玩法并持续执行，直到当天剩余次数用完。

随机玩法包括：

- lucky numbers
- three-kind
- icon-match
- treasure-chests
- progress-run

流程会等待开奖时间、揭奖、重试同步历史，并记录每轮成本、奖励和累计统计。

日志写入：

- `var/log/scratch/<sanitized-email>.log`

## 长任务日志视图

挖矿、自动签到、自动羊了个羊、全自动白嫖玩法和自动随机刮刮乐都使用应用内日志视图。

交互模式行为：

- 顶部固定显示工具标题、当前任务和操作提示。
- 中间区域保存本次任务完整日志，可回看超过窗口高度的历史内容。
- `↑` / `↓` 单行滚动。
- `PgUp` / `PgDn` 翻页。
- `Home` / `End` 跳到顶部或底部。
- 鼠标滚轮可查看历史日志。
- 用户停在历史位置时不会被新日志强制拉回底部；按 `End` 回到底部后继续跟随最新日志。
- 运行中按 `ESC` 请求停止后台任务；任务结束后按 `ESC` 返回上一级菜单。

非交互模式下任务同步执行并输出到 stdout。设置 `HDD_SMOKE_AUTO_RETURN=1` 时，完成后会自动返回，供 smoke 脚本使用。

## 目录结构

```text
hdd/
├─ src/
│  ├─ main.rs                  # 主程序入口：迁移旧数据、准备控制台、进入 CLI
│  ├─ lib.rs                   # 根库模块声明
│  ├─ bin/
│  │  ├─ benchcmp.rs           # CPU/Rust benchmark 辅助工具
│  │  └─ benchcuda.rs          # GPU benchmark 辅助工具
│  ├─ cli/
│  │  ├─ mod.rs                # 主菜单入口与接线
│  │  ├─ batch.rs              # 批量账号菜单与功能入口
│  │  ├─ mining.rs             # 挖矿菜单接线
│  │  └─ prompt.rs             # 账号输入、密码遮罩、交互式选项读取
│  ├─ api/
│  │  ├─ mod.rs                # API client 门面、默认 base URL、接口路径常量
│  │  ├─ client.rs             # blocking reqwest HTTP 客户端
│  │  ├─ cookies.rs            # Cookie 解析、归并、请求头构建
│  │  └─ endpoints.rs          # 接口分组、错误文案、本地化辅助
│  ├─ model/
│  │  ├─ mod.rs                # 共享 DTO 导出
│  │  ├─ auth.rs               # 登录态与账号缓存 DTO
│  │  ├─ checkin.rs            # 签到 DTO
│  │  ├─ scratch.rs            # 刮刮乐 DTO
│  │  └─ sheepmatch.rs         # 羊了个羊 DTO
│  ├─ workflows/
│  │  ├─ mod.rs                # 业务流程模块入口
│  │  ├─ free_play.rs          # 白嫖玩法组合调度
│  │  ├─ checkin/
│  │  │  ├─ mod.rs             # 批量签到入口
│  │  │  ├─ auth.rs            # 登录态校验与重登
│  │  │  ├─ run.rs             # 单账号签到执行与结果归并
│  │  │  └─ log.rs             # 签到日志
│  │  ├─ scratch/
│  │  │  ├─ mod.rs             # 刮刮乐批量入口
│  │  │  ├─ auth.rs            # 登录态校验与重登
│  │  │  ├─ round.rs           # 刮刮乐轮次执行、补开奖、历史同步
│  │  │  └─ log.rs             # 刮刮乐日志
│  │  └─ sheepmatch/
│  │     ├─ mod.rs             # 羊了个羊批量入口
│  │     ├─ auth.rs            # 登录态校验与重登
│  │     ├─ round.rs           # 对局执行、点击重试、轮次汇总
│  │     ├─ snapshot.rs        # 棋盘快照与固定点击队列
│  │     ├─ log.rs             # 羊了个羊日志
│  │     └─ tests.rs           # 羊了个羊内部测试
│  ├─ storage/
│  │  ├─ mod.rs                # 账号缓存导出
│  │  ├─ cache.rs              # 账号缓存读写、旧格式兼容
│  │  └─ normalize.rs          # 登录态归一化与合并
│  ├─ runtime/
│  │  ├─ mod.rs                # 运行时路径导出
│  │  └─ paths.rs              # var/、dist/、legacy 数据迁移与查找
│  ├─ solver/
│  │  ├─ mod.rs                # 独立搜索模块入口
│  │  └─ search.rs             # 深搜与道具边界规划
│  └─ ui/
│     ├─ mod.rs                # 控制台准备、固定顶部、日志视图、ESC 中断
│     ├─ render.rs             # 单行渲染、宽字符裁剪、完成提示
│     ├─ windows.rs            # Windows 控制台准备
│     └─ other.rs              # 非 Windows 控制台准备
├─ crates/
│  ├─ mining/
│  │  └─ src/
│  │     ├─ lib.rs             # 挖矿公开 API 门面
│  │     ├─ client.rs          # 矿池 HTTP 客户端
│  │     ├─ config.rs          # 模式、配置、输出 sink
│  │     ├─ error.rs           # 错误类型
│  │     ├─ gpu.rs             # GPU 发现与 benchmark 入口
│  │     ├─ messages.rs        # 挖矿文案与本地化
│  │     ├─ protocol.rs        # 挖矿协议 DTO 与兼容解析
│  │     ├─ backend/
│  │     │  ├─ mod.rs          # 计算后端导出与 nonce 分配
│  │     │  ├─ cpu.rs          # Rust CPU 后端
│  │     │  ├─ cuda.rs         # CUDA 后端封装
│  │     │  ├─ metal.rs        # Metal 后端封装
│  │     │  ├─ opencl.rs       # OpenCL 后端封装
│  │     │  └─ types.rs        # 后端共享类型
│  │     └─ runner/
│  │        ├─ mod.rs          # 挖矿主循环、心跳、提交、会话重建
│  │        ├─ gpu.rs          # GPU 候选筛选、调优与 failover
│  │        └─ support.rs      # 输出文件、奖励保存、共享辅助类型
│  ├─ mining-cuda-sys/         # Windows x64 CUDA FFI；失败时降级
│  ├─ mining-opencl-sys/       # macOS OpenCL FFI；失败时降级
│  └─ mining-metal-sys/        # macOS Metal FFI；失败时降级
├─ native/
│  ├─ mining-cuda/             # Rust CUDA sys crate 使用的 CUDA 计算核心
│  ├─ mining-opencl/           # Rust OpenCL sys crate 使用的 OpenCL 计算核心
│  ├─ mining-metal/            # Rust Metal sys crate 使用的 Metal 计算核心
│  ├─ mining-cuda-common/      # 独立 CUDA 工具共享代码
│  ├─ mining-invite-cuda/      # 独立原生邀请码 CUDA 工具
│  └─ mining-balance-cuda/     # 独立原生余额码 CUDA 工具
├─ scripts/                    # 构建、release、smoke 脚本
├─ var/                        # 运行时数据与日志，git 忽略
├─ dist/                       # 打包产物、构建日志、状态文件，git 忽略
├─ Cargo.toml                  # 根包 + workspace 清单
├─ Cargo.lock                  # workspace 锁文件
└─ README.md
```

## 模块职责

- `cli`：命令行菜单、选项输入、账号添加、业务流程接线。
- `api`：HTTP client、cookie、接口边界、接口错误文案。
- `model`：共享 DTO / serde 结构，兼容接口里的字符串数字和可选字段。
- `workflows`：签到、刮刮乐、羊了个羊、白嫖组合流程。
- `storage`：账号缓存读写、旧格式兼容、登录态归一化。
- `runtime`：路径解析、打包产物查找、legacy 数据迁移。
- `solver`：独立羊了个羊搜索辅助模块；当前自动玩法主流程不调用它。
- `ui`：终端准备、固定顶部、应用内滚动日志视图、ESC 中断、单行渲染。
- `crates/mining`：挖矿协议、CPU/GPU 后端选择、自动调优、心跳、提交、奖励保存。
- `*-sys` crates：按目标平台尝试构建原生 GPU 后端，失败时只禁用该后端，不中断整个 Rust build。

## 运行时文件布局

运行时文件统一放在 `var/`：

- `var/data/auth.json`
- `var/data/mining/invite-codes.txt`
- `var/data/mining/balance-codes.txt`
- `var/log/checkin/checkin.log`
- `var/log/scratch/<sanitized-email>.log`
- `var/log/sheepmatch/<sanitized-email>.log`

兼容性约定：

- 启动时会尝试把根目录旧 `auth.json`、`invite-codes.txt`、`balance-codes.txt` 迁移到 `var/data/`。
- 当前挖矿新输出使用 `var/data/mining/` 子目录。
- 账号缓存兼容旧单账号格式和旧 `sessions` 字段。
- 读取时会自动归一化，保存时统一写回当前格式。
- `token_type` 保留在账号级，默认归一为 `Bearer`。

## 打包产物布局

打包产物和构建状态平铺在 `dist/` 根目录：

- `dist/hdd-win-x64.exe`
- `dist/hdd-win-x64.build.log`
- `dist/hdd-win-x64.status`
- `dist/hdd-macos-amd64`
- `dist/hdd-macos-amd64.log`
- `dist/hdd-macos-amd64.status`
- `dist/hdd-macos-arm64`
- `dist/hdd-macos-arm64.log`
- `dist/hdd-macos-arm64.status`

运行时查找打包文件时，按顺序检查：

1. 可执行文件所在目录
2. 当前工作目录
3. workspace 根目录下的 `dist/`
4. 旧的 `artifacts/`（兼容旧布局）

## 构建与打包

### 单平台构建

Windows x64：

```bat
scripts\build-win-x64.bat
```

macOS amd64：

```bash
bash scripts/build-macos-amd64.sh
```

macOS arm64：

```bash
bash scripts/build-macos-arm64.sh
```

macOS 脚本支持 `--check` 和 `--orchestrated`，release 脚本内部会使用 orchestrated 模式。

### 默认 release

Windows 下：

```bat
scripts\release.bat
```

bash 下：

```bash
bash scripts/release.sh
```

默认会依次尝试三个平台：

- Windows x64
- macOS amd64
- macOS arm64

release 脚本会在全部目标都处理完后统一输出汇总。

状态语义：

- `built`：该平台完整打包成功。
- `built_degraded`：该平台基础包成功，但某个可选 GPU 原生后端缺环境或构建失败，运行时会走剩余后端/CPU fallback。
- `failed`：该平台基础构建失败；其他平台仍继续尝试。

如果任一平台是 `failed`，release 最终以失败退出；如果没有 `failed` 但存在 `built_degraded`，release 成功退出并提示部分平台为降级包。

### 原生 GPU 后端环境

- Windows CUDA：需要 Windows x64 host/target、CUDA Toolkit、`nvcc`、MSVC 工具链；缺失时禁用 CUDA 后端，不影响 CPU 包。
- macOS OpenCL / Metal：需要 macOS host/target 和 Apple SDK；缺失时禁用对应后端，不影响基础包。
- macOS cross/non-macOS 构建路径可使用 `cargo-zigbuild` 与 `zig`；脚本会根据环境探测可行路径。

## 开发与测试

格式检查：

```bash
cargo fmt --check
```

快速编译检查：

```bash
cargo check --workspace
```

完整测试：

```bash
cargo test --workspace
```

benchmark 辅助工具：

```bash
cargo run --release --bin benchcmp
cargo run --release --bin benchcuda
```

当前测试主要覆盖：

- 账号缓存旧格式兼容、归一化和保存目录创建。
- cookie 解析、合并和请求头构建。
- API endpoint 标签和本地化错误。
- runtime root、`var/`、`dist/`、legacy 数据迁移。
- prompt / render / UI 滚动计算。
- 自动签到结果和日志语义。
- 刮刮乐、签到、羊了个羊 DTO 兼容解析。
- 全自动白嫖玩法的账号内顺序、账号间并发和保存行为。
- 羊了个羊点击队列、认证重试、HTTP 重试、槽位满处理、剩余次数和快照更新。
- 挖矿后端选择、奖励类型解析和基础行为。

## 批量菜单烟测

Windows 下可运行：

```powershell
powershell -NoLogo -ExecutionPolicy Bypass -File scripts/smoke-batch-menu.ps1
```

默认 smoke 会构建并驱动 `dist/hdd-win-x64.exe`，覆盖：

- 自动签到
- 自动羊了个羊
- 全自动完成所有白嫖玩法

自动随机刮刮乐默认跳过；需要完整 smoke 时运行：

```powershell
powershell -NoLogo -ExecutionPolicy Bypass -File scripts/smoke-batch-menu.ps1 -SkipScratch:$false
```

smoke 会设置 `HDD_SMOKE_AUTO_RETURN=1`，并检查：

- 完成提示出现。
- `ESC` 返回上一级菜单可用。
- 对应日志文件有增长。
- 每个 flow 的会话日志写到 `.tmp-smoke-batch/`。
