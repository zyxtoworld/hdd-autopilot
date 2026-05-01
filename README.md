# hdd-autopilot

面向号多多AI公益站的多账号自动化运行器。Rust 根包和发布产物统一使用 `hdd-autopilot`，源码中 crate 导入名为 `hdd_autopilot`；根包负责 CLI 菜单、账号缓存、批量玩法、运行时路径和终端日志视图；`crates/mining` 负责挖矿控制面、CPU/GPU 后端调优、奖励码提交和结果保存；CUDA / OpenCL / Metal 原生计算核心位于 `native/`，并通过对应 `*-sys` crate 接入 Rust。

## 项目规范

项目按 Cargo workspace 的常规布局组织：根包放在 `src/`，内部工具放在 `src/bin/`，示例程序放在 `examples/`，可复用挖矿能力拆到 `crates/mining` 及平台 `*-sys` crate。Rust 代码使用标准模块命名：文件和模块为 `snake_case`，类型为 `UpperCamelCase`，函数和变量为 `snake_case`，常量为 `SCREAMING_SNAKE_CASE`。

开发基线：

- `cargo fmt --check`：统一 rustfmt 风格。
- `cargo clippy --workspace --all-targets -- -D warnings`：按 Clippy 惯用法收敛实现。
- `cargo test --workspace`：验证主业务、DTO 兼容、求解器、UI 和挖矿辅助逻辑。
- `cargo check --examples`：保证示例程序可编译。

## 快速运行

源码运行：

```bash
cargo run --release --bin hdd-autopilot
```

打包后运行 `dist/` 下对应平台的产物：

- `dist/hdd-autopilot-x86_64-pc-windows-msvc.exe`
- `dist/hdd-autopilot-x86_64-apple-darwin`
- `dist/hdd-autopilot-aarch64-apple-darwin`
- `dist/hdd-autopilot-x86_64-unknown-linux-gnu`
- `dist/hdd-autopilot-aarch64-unknown-linux-gnu`

macOS / Linux 包是自解压 shell wrapper，可用 `sh dist/hdd-autopilot-x86_64-unknown-linux-gnu` 这类命令运行；如果需要 `./dist/hdd-autopilot-x86_64-unknown-linux-gnu` 直接执行，先 `chmod +x`。Linux 包优先按 glibc 2.17 目标构建，可覆盖大多数仍在维护的 glibc 发行版；非 glibc 系统如 Alpine/musl 不在这个包的兼容范围内。

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

默认走自动调优模式，不要求用户手动选择 CPU / GPU。CLI 调用 `run_auto_tuned_with_config_and_cancel`，会保留 CPU 兜底；普通自动调优会同时启动 CPU 调优和 GPU 调优。GPU 调优会读取显卡显存、单次最大分配、计算单元、线程组限制、本地/共享内存、子组/warp 大小，以及 OpenCL/Metal 暴露的统一内存、低功耗和外接属性，先生成适配当前任务内存成本和显卡形态的候选 batch / segment / precompute 策略，再实测选择最快配置。运行时会选择最快 CPU，并把去重后的多张 GPU 一起参与挖矿，避免同一张卡同时被 CUDA/OpenCL 或 Metal/OpenCL 重复占用。

当前 Rust 接入的计算后端：

- CPU：纯 Rust portable fallback，所有平台可用。
- CUDA：Windows x86_64 原生后端；缺少 CUDA / nvcc / MSVC 等环境时降级为不可用后端。
- OpenCL：Windows / macOS / Linux 原生后端；枚举 OpenCL 暴露的 GPU / Accelerator 设备，覆盖 Apple GPU、AMD / NVIDIA / Intel OpenCL 设备，以及 macOS 可被系统驱动暴露的外接 GPU。
- Metal：macOS 原生后端；仅 macOS target + macOS host 构建时启用。

挖矿运行时会输出后端探测、测速、选择、降级和运行状态。GPU 不可用、测速失败或运行期失败时会回退到 CPU 或切换到其他可用后端。实际挖矿会按设备实测速度选择多张去重后的 GPU；同一张显卡如果同时被多个 API 暴露，只保留实测最快的那条后端路径。

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

1. 全自动运行所有白嫖玩法
2. 自动签到
3. 自动羊了个羊
4. 自动谜题2048
5. 自动记忆翻牌
6. 自动华容道
7. 自动数独
8. 返回上一级菜单
9. 退出脚本

白嫖玩法的通用规则：

- 每个玩法先查 `config` 和 `me`，读取难度、规则、剩余次数和 `active_session`。
- 有残局先续玩残局，残局结束后再开新局。
- 新局从最简单难度开始，一个难度次数用完后再进入下一个难度。
- `won` 记成功；`lost`、`failed`、`game_over` 记失败。
- `pending`、`running`、`active` 只表示进行中，不算成功或失败。
- 单局失败不会停止后续次数；只要 `me` 里还有剩余次数，就继续开下一局。
- 自动流程不会主动放弃当前局，不主动调用 abandon。
- 白嫖玩法接口使用 10 秒连接超时和 30 秒请求总超时；网络错误、408、425、429、5xx 等临时错误最多重试 60 次，每次间隔 500ms。
- 单个接口连续重试耗尽后会把本次玩法/线程判定为未跑完并立即重新进入流程，重新读取 `me`、续 `active_session` 和剩余次数；这类接口异常不会被记成游戏失败。

赌狗玩法菜单：

1. 自动随机刮刮乐
2. 返回上一级菜单
3. 退出脚本

批量页会刷新账号余额和状态，按账号、邮箱、余额、状态列对齐显示，并在列表下一行输出所有已成功刷新账号的余额汇总。文件不存在或为空会初始化默认配置；损坏 JSON 等真实错误会直接提示并退出。

### 自动签到

自动签到按账号并发执行。每个账号会先尝试恢复登录状态：cookies、token、密码重登依次兜底。流程会查询今日签到状态，已签到时报告失败状态，不重复领取；成功领取后按余额变化或奖励字段计算本次增加值。

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

### 自动谜题2048

自动谜题2048按账号并发执行。每个账号会读取谜题配置和账号状态，先续玩未结束残局，再按难度顺序处理当天剩余次数：

1. 入门 `mini`：3x3，目标 512
2. 经典 `classic`：4x4，目标 2048
3. 挑战 `jumbo`：5x5，目标 4096

每一步都会用 `/puzzle2048-api/move` 返回的最新棋盘重新求下一步，不在本地推测服务端生成的新数字。求解器使用合法移动枚举、expectimax 搜索和空格数、可合并数、蛇形单调性、平滑度、最大块角落等启发式评分；接口返回成功后立即进入下一步。流程不会主动 abandon 当前局。

日志写入：

- `var/log/puzzle_2048/<sanitized-email>.log`

### 自动记忆翻牌

自动记忆翻牌按账号并发执行。每个账号会读取 `/memory-api/config` 和历史记录，优先续玩 `pending` 残局，再按难度顺序处理当天剩余次数：

1. 简单 `easy`
2. 普通 `normal`
3. 困难 `hard`
4. 地狱 `hell`

求解器会记录已经翻开的卡牌和符号，发现已知对子时优先消除；没有已知对子时继续探测未知格，并避开刚刚失败的组合。每次 `/memory-api/flip` 返回成功后立即根据最新状态继续下一步。

日志写入：

- `var/log/memory/<sanitized-email>.log`

### 自动华容道

自动华容道按账号并发执行。每个账号会读取 `/puzzle15-api/config` 和历史记录，优先续玩 `pending` 残局，再按难度顺序处理当天剩余次数：

1. 入门 `easy`：3x3
2. 经典 `classic`：4x4
3. 挑战 `hard`：5x5

求解器拿到开局或残局棋盘后一次性计算完整移动序列，再按顺序调用 `/puzzle15-api/move`。接口里的方向按“数字块向空格移动”的语义处理；每步只等待接口返回成功，不再做额外间隔。

日志写入：

- `var/log/puzzle_15/<sanitized-email>.log`

### 自动数独

自动数独按账号并发执行。每个账号会读取 `/sudoku-api/config` 和历史记录，优先续玩 `pending` 残局，再按难度顺序处理当天剩余次数：

1. 入门 `easy`
2. 普通 `normal`
3. 困难 `hard`
4. 专家 `expert`

求解器根据 `givens` 计算完整答案，对空格和可编辑错误格直接提交正确值；如果残局里存在冲突，流程会按接口返回状态继续修正，必要时使用 `value: null` 清除后重填。每次 `/sudoku-api/fill` 返回成功后立即提交下一格。

日志写入：

- `var/log/sudoku/<sanitized-email>.log`

### 全自动运行所有白嫖玩法

全自动白嫖玩法当前包含：

1. 自动签到
2. 自动羊了个羊
3. 自动谜题2048
4. 自动记忆翻牌
5. 自动华容道
6. 自动数独

不同账号之间并发执行；同一个账号内部的不同白嫖玩法也会并发执行。账号状态会在全部线程完成后合并并保存一次，并在所有账号都完成后输出总完成提示。

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

挖矿、自动签到、自动羊了个羊、自动谜题2048、自动记忆翻牌、自动华容道、自动数独、全自动白嫖玩法和自动随机刮刮乐都使用应用内日志视图。

交互模式行为：

- 顶部固定显示工具标题、当前任务和操作提示。
- 中间区域保存本次任务完整日志，可回看超过窗口高度的历史内容。
- `↑` / `↓` 单行滚动。
- `PgUp` / `PgDn` 翻页。
- `Home` / `End` 跳到顶部或底部。
- 鼠标滚轮可查看历史日志。
- 用户停在历史位置时不会被新日志强制拉回底部；按 `End` 回到底部后继续跟随最新日志。
- 运行中按 `ESC` 请求停止后台任务；任务结束后按 `ESC` 返回上一级菜单。

非交互模式下任务同步执行并输出到 stdout。设置 `HDD_AUTOPILOT_SMOKE_AUTO_RETURN=1` 时，完成后会自动返回，供 smoke 脚本使用。

## 目录结构

```text
hdd-autopilot/
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
│  │  ├─ auth.rs               # 登录状态与账号缓存 DTO
│  │  ├─ checkin.rs            # 签到 DTO
│  │  ├─ memory.rs             # 记忆翻牌 DTO
│  │  ├─ puzzle_15.rs          # 华容道 DTO
│  │  ├─ puzzle_2048.rs        # 谜题2048 DTO
│  │  ├─ scratch.rs            # 刮刮乐 DTO
│  │  ├─ sheepmatch.rs         # 羊了个羊 DTO
│  │  └─ sudoku.rs             # 数独 DTO
│  ├─ workflows/
│  │  ├─ mod.rs                # 业务流程模块入口
│  │  ├─ common.rs             # 多账号流程共享状态、认证重试、时间和日志辅助
│  │  ├─ free_play.rs          # 白嫖玩法组合调度
│  │  ├─ checkin/
│  │  │  ├─ mod.rs             # 批量签到入口
│  │  │  ├─ auth.rs            # 登录状态校验与重登
│  │  │  ├─ run.rs             # 单账号签到执行与结果归并
│  │  │  └─ log.rs             # 签到日志
│  │  ├─ scratch/
│  │  │  ├─ mod.rs             # 刮刮乐批量入口
│  │  │  ├─ auth.rs            # 登录状态校验与重登
│  │  │  ├─ round.rs           # 刮刮乐轮次执行、补开奖、历史同步
│  │  │  └─ log.rs             # 刮刮乐日志
│  │  ├─ memory/
│  │  │  ├─ mod.rs             # 记忆翻牌批量入口
│  │  │  ├─ round.rs           # 残局续玩、新局执行和翻牌策略
│  │  │  ├─ types.rs           # 记忆翻牌流程内部类型
│  │  │  └─ log.rs             # 记忆翻牌日志
│  │  ├─ puzzle_15/
│  │  │  ├─ mod.rs             # 华容道批量入口
│  │  │  ├─ round.rs           # 残局续玩、新局执行和移动提交
│  │  │  ├─ types.rs           # 华容道流程内部类型
│  │  │  └─ log.rs             # 华容道日志
│  │  ├─ puzzle_2048/
│  │  │  ├─ mod.rs             # 谜题2048批量入口
│  │  │  ├─ round.rs           # 残局续玩、新局执行和棋盘求解
│  │  │  ├─ types.rs           # 谜题2048流程内部类型
│  │  │  ├─ log.rs             # 谜题2048日志
│  │  │  └─ tests.rs           # 谜题2048内部测试
│  │  ├─ sheepmatch/
│  │  │  ├─ mod.rs             # 羊了个羊批量入口
│  │  │  ├─ auth.rs            # 登录状态校验与重登
│  │  │  ├─ round.rs           # 对局执行、点击重试、轮次汇总
│  │  │  ├─ snapshot.rs        # 棋盘快照与固定点击队列
│  │  │  ├─ log.rs             # 羊了个羊日志
│  │  │  └─ tests.rs           # 羊了个羊内部测试
│  │  └─ sudoku/
│  │     ├─ mod.rs             # 数独批量入口
│  │     ├─ round.rs           # 残局续玩、新局执行和填数提交
│  │     ├─ types.rs           # 数独流程内部类型
│  │     └─ log.rs             # 数独日志
│  ├─ storage/
│  │  ├─ mod.rs                # 账号缓存导出
│  │  ├─ cache.rs              # 账号缓存读写、旧格式兼容
│  │  └─ normalize.rs          # 登录状态归一化与合并
│  ├─ runtime/
│  │  ├─ mod.rs                # 运行时路径导出
│  │  └─ paths.rs              # var/、dist/、legacy 数据迁移与查找
│  ├─ solver/
│  │  ├─ mod.rs                # 独立搜索模块入口
│  │  ├─ search.rs             # 羊了个羊深搜与道具边界规划
│  │  ├─ memory/               # 记忆翻牌已知对子和探测策略
│  │  ├─ puzzle_15/            # 华容道可解性校验和加权 A* 搜索
│  │  ├─ puzzle_2048/          # 谜题2048棋盘模拟和 expectimax 策略
│  │  └─ sudoku/               # 数独回溯求解和填数计划
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
│  ├─ mining-cuda-sys/         # Windows x86_64 CUDA FFI；失败时降级
│  ├─ mining-opencl-sys/       # Windows / macOS / Linux OpenCL FFI；失败时降级
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
- `api`：HTTP client、连接/请求超时、cookie、接口边界、接口错误文案。
- `model`：共享 DTO / serde 结构，兼容接口里的字符串数字和可选字段。
- `workflows`：签到、刮刮乐、羊了个羊、谜题2048、记忆翻牌、华容道、数独、白嫖组合流程；`common.rs` 承担小游戏共享认证重试、接口重试上限、批量状态、账号日志路径、时间格式和日志分句工具。
- `storage`：账号缓存读写、旧格式兼容、登录状态归一化。
- `runtime`：路径解析、打包产物查找、legacy 数据迁移。
- `solver`：羊了个羊搜索辅助模块，以及谜题2048、记忆翻牌、华容道、数独的自动求解器。
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
- `var/log/puzzle_2048/<sanitized-email>.log`
- `var/log/memory/<sanitized-email>.log`
- `var/log/puzzle_15/<sanitized-email>.log`
- `var/log/sudoku/<sanitized-email>.log`

兼容性约定：

- 启动时会尝试把根目录旧 `auth.json`、`invite-codes.txt`、`balance-codes.txt` 迁移到 `var/data/`。
- 当前挖矿新输出使用 `var/data/mining/` 子目录。
- 账号缓存兼容旧单账号格式和旧 `sessions` 字段。
- 读取时会自动归一化，保存时统一写回当前格式。
- `token_type` 保留在账号级，默认归一为 `Bearer`。

清理约定：

- `target/`、`.claude/`、`native/*/build/`、`var/log/` 都是本地临时或运行日志，可按需删除。
- `dist/*.log` 和 `dist/*.status` 是打包过程状态文件，可删除；`dist/hdd-autopilot-*` 是正式包。
- `var/data/auth.json` 保存账号登录状态，清理项目时不要误删。

## 打包产物布局

打包产物和构建状态平铺在 `dist/` 根目录：

- `dist/hdd-autopilot-x86_64-pc-windows-msvc.exe`
- `dist/hdd-autopilot-x86_64-pc-windows-msvc.build.log`
- `dist/hdd-autopilot-x86_64-pc-windows-msvc.status`
- `dist/hdd-autopilot-x86_64-apple-darwin`
- `dist/hdd-autopilot-x86_64-apple-darwin.log`
- `dist/hdd-autopilot-x86_64-apple-darwin.status`
- `dist/hdd-autopilot-aarch64-apple-darwin`
- `dist/hdd-autopilot-aarch64-apple-darwin.log`
- `dist/hdd-autopilot-aarch64-apple-darwin.status`
- `dist/hdd-autopilot-x86_64-unknown-linux-gnu`
- `dist/hdd-autopilot-x86_64-unknown-linux-gnu.log`
- `dist/hdd-autopilot-x86_64-unknown-linux-gnu.status`
- `dist/hdd-autopilot-aarch64-unknown-linux-gnu`
- `dist/hdd-autopilot-aarch64-unknown-linux-gnu.log`
- `dist/hdd-autopilot-aarch64-unknown-linux-gnu.status`

运行时查找打包文件时，按顺序检查：

1. 可执行文件所在目录
2. 当前工作目录
3. workspace 根目录下的 `dist/`
4. 旧的 `artifacts/`（兼容旧布局）

## 构建与打包

### 单平台构建

Windows x86_64：

```bat
scripts\build-x86_64-pc-windows-msvc.bat
```

macOS x86_64：

```bash
bash scripts/build-x86_64-apple-darwin.sh
```

macOS aarch64：

```bash
bash scripts/build-aarch64-apple-darwin.sh
```

Linux x86_64：

```bash
bash scripts/build-x86_64-unknown-linux-gnu.sh
```

Linux aarch64：

```bash
bash scripts/build-aarch64-unknown-linux-gnu.sh
```

macOS / Linux 脚本支持 `--check` 和 `--orchestrated`，release 脚本内部会使用 orchestrated 模式。

### 默认 release

Windows 下：

```bat
scripts\release.bat
```

bash 下：

```bash
bash scripts/release.sh
```

默认会依次尝试五个平台：

- Windows x86_64
- macOS x86_64
- macOS aarch64
- Linux x86_64
- Linux aarch64

release 脚本会在全部目标都处理完后统一输出汇总。

状态语义：

- `built`：该平台完整打包成功。
- `built_degraded`：该平台基础包成功，但某个可选 GPU 原生后端缺环境或构建失败，运行时会走剩余后端/CPU fallback。
- `failed`：该平台基础构建失败；其他平台仍继续尝试。

如果任一平台是 `failed`，release 最终以失败退出；如果没有 `failed` 但存在 `built_degraded`，release 成功退出并提示部分平台为降级包。

### GitHub Actions 自动打包

仓库包含 `.github/workflows/release.yml`。推送 `v*` tag 或手动触发 workflow 时，会分别构建并上传：

- `hdd-autopilot-x86_64-pc-windows-msvc.exe`
- `hdd-autopilot-x86_64-apple-darwin`
- `hdd-autopilot-aarch64-apple-darwin`
- `hdd-autopilot-x86_64-unknown-linux-gnu`
- `hdd-autopilot-aarch64-unknown-linux-gnu`

tag 触发时会把上述正式包上传到 GitHub Release；手动触发时只生成 Actions artifacts，方便先验证。GitHub Actions 会校验每个平台的 `.status` 必须是 `built`，如果可选 GPU 原生后端缺环境导致 `built_degraded`，该平台自动打包会直接失败，避免发布降级包；本地脚本仍保留原来的降级容错逻辑。

### 原生 GPU 后端环境

- Windows CUDA / OpenCL：CUDA 需要 Windows x86_64 host/target、CUDA Toolkit、`nvcc`、MSVC 工具链；OpenCL 需要 OpenCL headers 和 `OpenCL.lib`，可来自 CUDA Toolkit、`OPENCL_ROOT` 或 vcpkg。缺失时禁用对应后端，不影响本地 CPU 包。
- macOS OpenCL / Metal：需要 macOS host/target 和 Apple SDK；Metal 覆盖系统可用 GPU，OpenCL 会枚举系统暴露的 GPU / Accelerator，包括 Apple GPU、AMD eGPU，以及安装了可用 OpenCL 驱动后被系统暴露的其他厂商设备；缺失时禁用对应后端，不影响本地基础包。
- Linux OpenCL：需要 Linux x86_64 或 aarch64 host/target、本机 C/C++ 工具链、OpenCL headers 和 ICD loader；运行机器还需要 NVIDIA / AMD / Intel 等厂商驱动或 OpenCL runtime 才能实际枚举显卡。
- macOS cross/non-macOS 构建路径可使用 `cargo-zigbuild` 与 `zig`；脚本会根据环境探测可行路径，但 OpenCL / Metal 原生后端只在 macOS host/target 启用。
- Linux x86_64：本机 Linux x86_64 构建会尝试编入 OpenCL 后端；脚本优先使用 `cargo-zigbuild` + `zig` 构建 glibc 2.17 兼容包，没有 zig 时才退回 Linux x86_64 host 本机 C 编译器。
- Linux aarch64：本机 Linux aarch64 构建会尝试编入 OpenCL 后端；脚本优先使用 `cargo-zigbuild` + `zig` 构建 glibc 2.17 兼容包，没有 zig 时才退回 Linux aarch64 host 本机 C 编译器。

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

Clippy 惯用法检查：

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

示例程序检查：

```bash
cargo check --examples
```

benchmark 辅助工具：

```bash
cargo run --release --bin benchcmp
cargo run --release --bin benchcuda
```

当前测试主要覆盖：

- 账号缓存旧格式兼容、归一化和保存目录创建。
- cookie 解析、合并和请求头构建。
- API endpoint 标签、本地化错误、连接超时和请求超时。
- runtime root、`var/`、`dist/`、legacy 数据迁移。
- prompt / render / UI 滚动计算。
- 自动签到结果和日志语义。
- 刮刮乐、签到、羊了个羊、谜题2048、记忆翻牌、华容道、数独 DTO 兼容解析。
- 全自动白嫖玩法的账号间并发、账号内多玩法并发和保存行为。
- 羊了个羊点击队列、认证重试、HTTP 重试上限、槽位满处理、剩余次数和快照更新。
- 挖矿后端选择、奖励类型解析和基础行为。
- 谜题2048移动合并规则、合法移动、直接胜利步和 3x3 / 4x4 / 5x5 求解器入口。
- 记忆翻牌已知对子、失败组合规避和残局识别。
- 华容道可解性校验、方向语义和 3x3 / 4x4 / 5x5 路径搜索。
- 数独回溯求解、错误可编辑格覆盖、冲突残局修复和填数计划。

## 批量菜单烟测

Windows 下可运行：

```powershell
powershell -NoLogo -ExecutionPolicy Bypass -File scripts/smoke-batch-menu.ps1
```

默认 smoke 会构建并驱动 `dist/hdd-autopilot-x86_64-pc-windows-msvc.exe`，覆盖：

- 自动签到
- 自动羊了个羊
- 自动谜题2048
- 全自动运行所有白嫖玩法

自动随机刮刮乐、自动记忆翻牌、自动华容道、自动数独的独立入口默认跳过，避免额外消耗次数；需要完整 smoke 时运行：

```powershell
powershell -NoLogo -ExecutionPolicy Bypass -File scripts/smoke-batch-menu.ps1 -SkipScratch:$false -SkipMemory:$false -SkipPuzzle15:$false -SkipSudoku:$false
```

smoke 会设置 `HDD_AUTOPILOT_SMOKE_AUTO_RETURN=1`，并检查：

- 完成提示出现。
- `ESC` 返回上一级菜单可用。
- 对应日志文件有增长。
- 每个 flow 的会话日志写到 `.tmp-smoke-batch/`。
