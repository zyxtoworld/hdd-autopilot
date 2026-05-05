# hdd-autopilot

面向号多多 AI 公益站的多账号自动化工具。项目主体是 Rust CLI，包含账号缓存、批量自动玩法、挖矿、GPU 后端调优、终端任务日志、跨平台打包和 GitHub Release 发布脚本。

## 功能概览

- 自动挖矿：支持邀请码、余额兑换码两类奖励，支持邀请码优先、余额码优先、只挖邀请码、只挖余额码四种模式。
- CPU + GPU 自动调优挖矿：CPU 始终可用；GPU 后端按平台尝试 CUDA、OpenCL、Metal，并根据真实 HTTP challenge 参数和设备参数自动筛选最优配置。
- 多账号缓存：添加账号时登录并保存 token；后续需要登录的接口统一使用 `Authorization` token。
- 有次数限制的白嫖玩法：自动签到、扫雷、羊了个羊、谜题 2048、推箱子、点灯、迷宫、数织、连线、记忆翻牌、华容道、数独。
- 无次数限制的白嫖玩法：自动箭头逃离；运行中固定显示所有账号实时总收益，按 `ESC` 停止。
- 全自动入口：有次数限制的白嫖玩法会对每个账号并发运行所有项目；无次数限制的白嫖玩法会持续运行当前菜单下所有项目。
- 赌狗玩法：自动随机刮刮乐。
- 任务日志视图：长任务在 CLI 内显示可滚动日志，运行中可按 `ESC` 请求停止。
- 发布工具：本地脚本和 GitHub Actions 都按规范 target triple 输出跨平台包。

## 快速运行

从源码运行：

```bash
cargo run --release --bin hdd-autopilot
```

打包后运行 `dist/` 里的对应平台文件：

- `dist/hdd-autopilot-x86_64-pc-windows-msvc.exe`
- `dist/hdd-autopilot-x86_64-apple-darwin`
- `dist/hdd-autopilot-aarch64-apple-darwin`
- `dist/hdd-autopilot-x86_64-unknown-linux-gnu`
- `dist/hdd-autopilot-aarch64-unknown-linux-gnu`

macOS / Linux 包是自解压 shell wrapper，可以用：

```bash
sh dist/hdd-autopilot-x86_64-unknown-linux-gnu
```

如果要直接 `./dist/hdd-autopilot-x86_64-unknown-linux-gnu` 运行，先执行：

```bash
chmod +x dist/hdd-autopilot-x86_64-unknown-linux-gnu
```

## 菜单入口

主菜单：

1. 挖矿
2. 需要登录的多账号批量操作功能
3. 退出脚本

挖矿菜单：

1. 先挖邀请码再挖余额码
2. 先挖余额码再挖邀请码
3. 只挖邀请码
4. 只挖余额码
5. 返回上一级菜单
6. 退出脚本

批量功能菜单：

1. 添加账号
2. 账号添加完成，选择脚本功能
3. 返回上一级菜单
4. 退出脚本

脚本功能分为：

- 白嫖玩法：
  - 有次数限制的白嫖玩法：全自动运行所有有次数限制的白嫖玩法、自动扫雷、自动羊了个羊、自动谜题 2048、自动推箱子、自动点灯、自动迷宫、自动数织、自动连线、自动记忆翻牌、自动华容道、自动数独、自动签到。
  - 无次数限制的白嫖玩法：全自动运行所有无次数限制的白嫖玩法、自动箭头逃离。
- 赌狗玩法：自动随机刮刮乐。

## 挖矿逻辑

挖矿流程默认自动调优，不要求用户手动选择 CPU、GPU 或后端。

每轮挖矿会先从矿池 HTTP 接口获取真实 challenge 参数，包括 `seed`、`round_id`、`visitor_id`、`challenge_id`、`session_salt`、`time_cost`、`memory_cost_mb`、`parallelism` 和 `difficulty_bits`。CPU / CUDA / OpenCL / Metal 的 benchmark 和最终筛选都基于这些参数派生出的真实任务数据，避免把旧 challenge 的测速结果套用到新 challenge。

后端选择规则：

- CPU 后端始终参与，作为兜底。
- GPU 后端会读取显存、最大单次分配、计算单元、线程组限制、本地/共享内存、subgroup/warp 大小、统一内存、低功耗和外接设备等信息。
- 每张 GPU 会生成多组 `batch_size`、分段计算、预计算策略候选，先初筛，再实测选择最快配置。
- 同一张卡如果同时被 CUDA/OpenCL 或 Metal/OpenCL 暴露，会按实测速度去重，只保留最快路径。
- 实际挖矿会同时使用最快 CPU 和去重后的可用 GPU；运行中某个后端失败会黑名单该后端并重新选择。

奖励输出：

- 邀请码：`var/data/mining/invite-codes.txt`
- 余额兑换码和并发码：`var/data/mining/balance-codes.txt`

## GPU 后端

| 后端 | 适用平台 | 说明 |
| --- | --- | --- |
| CPU | 全平台 | Rust 实现，永远可用 |
| CUDA | Windows x86_64、Linux x86_64；Linux aarch64 和旧 Intel macOS 可在本机/self-hosted 环境启用 | 需要 CUDA Toolkit / `nvcc` |
| OpenCL | Windows、macOS、Linux | 需要 OpenCL headers、ICD loader 和厂商运行时 |
| Metal | macOS | 仅 macOS host/target 启用 |

本地构建如果缺少某个可选 GPU 环境，会禁用对应后端并继续构建；GitHub Actions 发布包要求 `.status` 必须是 `built`，不会发布 `built_degraded` 降级包。

## 多账号与自动玩法

账号保存在 `var/data/auth.json`。添加账号会保存邮箱、密码和 token，用于后续自动恢复登录状态。该文件包含敏感信息，已被 `.gitignore` 排除，不要手动提交。

白嫖玩法通用策略：

- 任务按账号并发执行。
- 每个账号先恢复登录状态；缓存里的 token 可用时直接复用，token 失效或缺失时会自动重登并更新缓存。
- 所有需要登录的接口调用都会带 `Authorization` token。
- 运行中接口返回 401 或登录状态失效时，会自动重登一次，刷新 token 后继续当前玩法。
- 账号缓存不保存业务登录 Cookie；旧缓存里的 Cookie 字段会被忽略，后续保存时写回 token-only 格式。挖矿模块内部的矿池 HTTP 会话保持原有行为，不写入 `auth.json`。
- 总调度层分为 `workflows/limited_free_play.rs` 和 `workflows/unlimited_free_play.rs`；单个玩法模块只负责自己的接口、求解器、日志和账号状态合并。
- 玩法会按各自接口读取配置和 `/me` 账号状态；免费玩法的剩余次数使用 `daily_plays_remaining` / `daily_plays_used` 或开局响应里的剩余字段，不用 history 条数倒推。
- 如果 `/me` 返回未完成残局，会先继续残局，再按玩法策略开新局。
- 单局失败不会阻止后续次数；只要接口仍允许继续开局，就继续运行。
- 无次数限制的白嫖玩法不会自动停在每日次数上限，运行中固定显示所有账号实时总收益，按 `ESC` 停止。
- 网络错误、408、425、429、5xx 等临时错误会按统一上限重试；400、403、404、409、422 等状态会按统一中文原因记录。
- 结算、点击、移动、填数、翻牌等会改变服务端状态的接口如果遇到超时、5xx、409 或响应丢失，会先回查 `/me` 和 `/history` 的同一对局状态；确认服务端已经推进后直接合并服务端快照继续，避免重复提交同一步或重复结算。
- 单个玩法如果多次重新进入仍无法恢复，会停止该玩法/账号任务并记录原因，避免无法处理的异常阻塞线程。
- `pending`、`running`、`active` 只表示进行中，不会记为成功或失败。

当前自动玩法：

- 自动签到：按网站流程检查账号和今日状态，未签到时领取奖励并记录余额变化；登录会复用并持久化 `auth.json` 中的 token；如果接口临时异常，会按通用重试策略处理，仍失败则记录本次签到失败并继续后续玩法，不会卡住全自动流程。
- 自动扫雷：使用错旗修复、确定性规则、子集/重叠约束和全局雷数加权概率枚举求解；先续 `/me` 返回的当前残局，再按接口奖励从高到低开新局；如果接口返回剩余次数字段会优先使用，否则不依赖只返回最近 12 局的 history，改为直到接口拒绝继续开局。
- 自动羊了个羊：按 `/me.daily_plays_remaining` 分难度处理剩余次数，使用 `/me.active_session` 续残局，并用接口快照推进固定点击队列。
- 自动谜题 2048：支持 3x3 / 4x4 / 5x5，使用合法移动枚举和 expectimax 策略求解。
- 自动推箱子：使用独立 `solver/sokoban` 搜索箱子和玩家状态，带死角剪枝；推箱子、点灯、迷宫、数织、连线都按 `/me.daily_plays_remaining` 查到的每个难度剩余次数依次跑 `easy -> normal -> hard`。
- 自动点灯：使用独立 `solver/lightsout` 对棋盘建立 GF(2) 线性方程，一次求出需要点击的格子。
- 自动迷宫：使用独立 `solver/maze` 按服务端返回的开放边做 BFS，提交最短方向序列。
- 自动数织：使用独立 `solver/nonogram` 生成行列候选并约束传播，最终只提交需要填充的格子。
- 自动连线：使用独立 `solver/flowfree` 为每种颜色搜索端点路径，按颜色提交连线路径。
- 自动记忆翻牌：记录已知卡牌，优先消除已知对子，避免刚失败的组合。
- 自动华容道：校验可解性并使用搜索结果按步提交移动。
- 自动数独：求解完整棋盘，提交空格或可编辑错误格。
- 自动箭头逃离：读取 `/arrow-out-api/me` 的当前局或新开局，按箭头身体占用、障碍物和出口方向计算可清除顺序，结算时提交完整点击序列；单独运行和无次数限制全自动都会固定显示所有账号实时总收益。
- 自动随机刮刮乐：补结算历史未开奖轮次，再随机选择玩法直到当天次数用完；开奖请求如果响应丢失，会回查历史确认是否已经开奖。

## 运行数据

运行时文件统一放在 `var/`：

```text
var/
  data/
    auth.json
    mining/
      invite-codes.txt
      balance-codes.txt
  log/
    YYYYMMDD/
      checkin/checkin.log
      scratch/<sanitized-email>.log
      sheepmatch/<sanitized-email>.log
      puzzle_2048/<sanitized-email>.log
      sokoban/<sanitized-email>.log
      lightsout/<sanitized-email>.log
      maze/<sanitized-email>.log
      nonogram/<sanitized-email>.log
      flowfree/<sanitized-email>.log
      memory/<sanitized-email>.log
      puzzle_15/<sanitized-email>.log
      sudoku/<sanitized-email>.log
      arrow_out/<sanitized-email>.log
```

兼容性说明：

- 启动时会尝试把旧版根目录数据迁移到 `var/data/`。
- 各玩法日志和挖矿奖励码保存日志按系统当前时区记录时间戳，并先分 `YYYYMMDD` 日期目录，再分项目目录；除签到外，每个项目目录下继续按账号拆分日志文件。
- 日志时间使用系统本地 UTC 偏移格式化；读取失败时回退到 UTC，避免因时区环境异常阻塞任务或影响跨平台构建。
- 登录状态校验接口会随请求传入系统当前 IANA 时区；macOS 优先读取 `TZ` 或 `systemsetup -gettimezone`，其他平台读取系统时区数据库，无法识别时回退到 `Etc/UTC`。
- 所有需要登录的玩法共用同一套登录态恢复逻辑：先加载并验证 `auth.json` 中的 token，验证通过后回写最新登录态；旧缓存如果缺 token，会用账号密码重新登录。
- `auth.json` 兼容旧单账号格式和旧 `sessions` 字段，保存时会写回当前扁平格式。
- `dist/`、`target/`、`var/`、`artifacts/`、native build 目录和本地 smoke 产物都被 `.gitignore` 排除。

## 构建与打包

单平台构建：

```bat
scripts\build-x86_64-pc-windows-msvc.bat
```

```bash
bash scripts/build-x86_64-apple-darwin.sh
bash scripts/build-aarch64-apple-darwin.sh
bash scripts/build-x86_64-unknown-linux-gnu.sh
bash scripts/build-aarch64-unknown-linux-gnu.sh
```

本地 release 汇总：

```bat
scripts\release.bat
```

```bash
bash scripts/release.sh
```

状态文件语义：

- `built`：完整包构建成功。
- `built_degraded`：基础包构建成功，但可选 GPU 原生后端缺环境或构建失败。
- `failed`：基础构建失败。

本地打包说明：

- Windows 本机可以用 `scripts\release.bat` 汇总构建 Windows、macOS 和 Linux 基础包。
- 非目标平台构建时，可选 GPU 原生后端可能缺少目标平台 SDK、驱动或 headers，脚本会保留基础包并写 `built_degraded`。
- 要得到 `built` 完整包，需要在对应平台/Runner 上具备原生 GPU 构建环境：Windows CUDA/OpenCL、macOS Metal/OpenCL、Linux CUDA/OpenCL。
- macOS 交叉构建可能出现 `xcrun`/SDK warning；只要最终状态文件不是 `failed`，基础包已经生成。

GitHub Actions：

- workflow：`.github/workflows/release.yml`
- 手动触发：生成 Actions artifacts。
- 推送 `v*` tag：构建五个平台并发布 GitHub Release。
- Windows x86_64 和 Linux x86_64 job 会安装 CUDA Toolkit。
- Linux job 会安装 OpenCL build dependencies。
- 所有 release job 都要求包状态为 `built`，避免自动发布降级包。

## 上传本地包到 GitHub Release

依赖 GitHub CLI：

```bash
gh auth login
```

Windows PowerShell：

```powershell
powershell -NoLogo -ExecutionPolicy Bypass -File scripts/upload-release.ps1 v0.1.0
```

macOS / Linux / Git Bash：

```bash
bash scripts/upload-release.sh v0.1.0
```

指定仓库或 dist 目录：

```bash
bash scripts/upload-release.sh v0.1.0 --repo owner/hdd-autopilot --dist ./dist
```

上传脚本会扫描 `dist/hdd-autopilot-*.status`，默认只上传状态为 `built` 的正式包；只有显式传 `--allow-degraded` 或 PowerShell 的 `-AllowDegraded` 时才允许上传降级包。

## 开发验证

常用检查：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --examples
```

benchmark 辅助工具：

```bash
cargo run --release --bin benchcmp
cargo run --release --bin benchcuda
```

Windows 批量菜单 smoke：

```powershell
powershell -NoLogo -ExecutionPolicy Bypass -File scripts/smoke-batch-menu.ps1
```

默认 smoke 会构建并驱动 `dist/hdd-autopilot-x86_64-pc-windows-msvc.exe`，覆盖主要批量菜单路径。需要额外覆盖刮刮乐、扫雷、推箱子、点灯、迷宫、数织、连线、记忆翻牌、华容道、数独和箭头逃离独立入口时：

```powershell
powershell -NoLogo -ExecutionPolicy Bypass -File scripts/smoke-batch-menu.ps1 -SkipScratch:$false -SkipMemory:$false -SkipPuzzle15:$false -SkipSudoku:$false -SkipArrowOut:$false
```

## 项目结构

```text
hdd-autopilot/
  src/
    api/         HTTP client、接口路径和错误文案
    cli/         CLI 菜单、账号添加、功能入口
    model/       API DTO 和难度/玩法常量
    runtime/     var/dist/artifacts 路径解析和旧数据迁移
    solver/      2048、扫雷、推箱子、点灯、迷宫、数织、连线、记忆翻牌、华容道、数独、箭头逃离求解器
    storage/     账号缓存读写、归一化和旧格式兼容
    ui/          终端日志视图、滚动、ESC 中断
    workflows/   单玩法流程；limited_free_play/unlimited_free_play 负责全自动调度
    bin/         benchcmp、benchcuda 辅助工具
  crates/
    mining/              挖矿控制面、CPU/GPU 选择、调优和奖励保存
    mining-cuda-sys/     CUDA FFI 封装
    mining-opencl-sys/   OpenCL FFI 封装
    mining-metal-sys/    Metal FFI 封装
  native/
    mining-cuda/         CUDA 原生计算核心
    mining-opencl/       OpenCL 原生计算核心
    mining-metal/        Metal 原生计算核心
  scripts/               构建、release、上传和 smoke 脚本
  .github/workflows/     GitHub Actions
```

## 安全注意

- 不要提交 `var/data/auth.json`，里面可能包含邮箱、密码和 access token。
- 不要提交 `dist/`、`target/`、native build 目录或日志文件。
- 公开仓库前可用关键词扫描检查是否误提交真实账号、token、私钥或 `.env`。
