# hdd

`hdd` 是号多多脚本整合工具的 Rust workspace 根目录。

仓库根目录就是主包 `hdd`，负责 CLI 菜单、账号缓存、批量玩法、运行时路径和日志；挖矿控制面与后端调优逻辑放在独立的 `crates/mining` crate，CUDA / OpenCL / Metal 的原生实现保留在 `native/` 和对应 `*-sys` crate 中。

## 当前功能

程序启动后会显示：

`欢迎使用号多多脚本整合工具。`

一级菜单：

1. 挖矿
2. 需要登录的多账号批量操作功能
3. 退出脚本

### 挖矿

挖矿菜单支持：

1. 先挖邀请码再挖余额码
2. 先挖余额码再挖邀请码
3. 只挖邀请码
4. 只挖余额码
5. 返回上一级菜单
6. 退出脚本

默认运行自动调优模式，不再要求用户手动选择 CPU / GPU。

进入挖矿后会自动探测可用计算后端，并按当前平台对候选后端测速后自动择优：

- Windows x64：CPU + CUDA（如果本机 NVIDIA 环境可用）
- macOS：CPU + OpenCL + Metal

如果 GPU 后端不可用、测速失败或运行期自检失败，会自动回退到 CPU 或切换到其他可用后端。

CPU 和 GPU 可用时会并发参与挖矿，终端日志会显示当前选中的计算后端、降级原因和运行状态。

余额码提交后会根据接口返回值区分：

- 余额兑换码：终端和 `var/data/mining/balance-codes.txt` 显示可增加多少余额。
- 并发码：终端和 `var/data/mining/balance-codes.txt` 显示可增加几并发。

`MiningClient::reset_session()` 的行为是业务逻辑：挖到奖励码并提交后必须重建 HTTP client/session，不要把它当成可复用会话优化掉。

挖矿运行过程中支持按 `ESC` 停止并返回上一级菜单，终端顶部会固定显示提示。

### 多账号批量功能

统一使用账号缓存文件管理账号。

批量入口菜单：

1. 添加账号
2. 账号添加完成，选择脚本功能
3. 返回上一级菜单
4. 退出脚本

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

特性说明：

- 启动时会读取账号缓存；文件不存在或为空会正常初始化，损坏 JSON 等真实错误会直接提示并退出。
- 批量页会实时刷新账号余额和状态。
- 自动签到写共享日志。
- 自动随机刮刮乐和自动羊了个羊按账号分别写日志。
- 自动羊了个羊按账号并发执行，并在控制台和日志里记录真实轮次进度。
- 全自动白嫖玩法当前包含：自动签到 + 自动羊了个羊。
- 批量业务完成后不会自动退出，而是停留在“按 ESC 返回上一级菜单”的状态。

## 长任务日志视图

挖矿、自动签到、自动羊了个羊、全自动白嫖玩法和自动随机刮刮乐都使用应用内日志视图：

- 顶部固定显示工具标题、当前任务和操作提示。
- 中间区域保存本次任务完整日志，可回看超过窗口高度的历史内容。
- `↑` / `↓` 单行滚动，`PgUp` / `PgDn` 翻页，`Home` / `End` 跳到顶部或底部。
- 支持鼠标滚轮查看历史日志。
- 用户停在历史位置时不会被新日志强制拉回底部；按 `End` 回到底部后继续跟随最新日志。
- 运行中按 `ESC` 会请求停止后台任务；任务结束后按 `ESC` 返回上一级菜单。

非交互环境下仍直接输出到 stdout，`HDD_SMOKE_AUTO_RETURN` 会自动返回，方便 smoke 脚本验证。

## 目录结构

当前实际结构如下：

```text
hdd/
├─ src/
│  ├─ main.rs                  # 主程序入口
│  ├─ lib.rs                   # 根库模块声明
│  ├─ bin/
│  │  ├─ benchcmp.rs           # CPU benchmark 工具
│  │  └─ benchcuda.rs          # GPU benchmark 工具
│  ├─ cli/
│  │  ├─ mod.rs                # 主菜单入口与接线
│  │  ├─ batch.rs              # 批量账号菜单与功能入口
│  │  ├─ mining.rs             # 挖矿菜单接线
│  │  └─ prompt.rs             # 账号输入与交互式选项读取
│  ├─ api/
│  │  ├─ mod.rs                # API 错误、客户端门面、接口路径常量
│  │  ├─ client.rs             # HTTP 客户端实现
│  │  ├─ cookies.rs            # Cookie 解析与归并
│  │  └─ endpoints.rs          # 接口分组与错误文本辅助
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
│  │  │  ├─ mod.rs             # 刮刮乐入口
│  │  │  ├─ auth.rs            # 登录态校验与重登
│  │  │  ├─ round.rs           # 刮刮乐轮次执行与补开奖
│  │  │  └─ log.rs             # 刮刮乐日志
│  │  └─ sheepmatch/
│  │     ├─ mod.rs             # 羊了个羊入口
│  │     ├─ auth.rs            # 登录态校验与重登
│  │     ├─ round.rs           # 对局执行与轮次汇总
│  │     ├─ snapshot.rs        # 棋盘快照与固定点击队列
│  │     ├─ log.rs             # 羊了个羊日志
│  │     └─ tests.rs           # 羊了个羊内部测试
│  ├─ storage/
│  │  ├─ mod.rs                # 账号缓存导出
│  │  ├─ cache.rs              # 账号缓存读写
│  │  └─ normalize.rs          # 登录态归一化与合并
│  ├─ runtime/
│  │  ├─ mod.rs                # 运行时路径导出
│  │  └─ paths.rs              # var/、dist/、legacy 迁移与查找
│  ├─ solver/
│  │  ├─ mod.rs                # 求解模块入口
│  │  └─ search.rs             # 深搜与道具边界规划
│  └─ ui/
│     ├─ mod.rs                # 控制台交互与 ESC 中断
│     ├─ render.rs             # 固定提示与单行渲染
│     ├─ windows.rs            # Windows 控制台准备
│     └─ other.rs              # 非 Windows 控制台准备
├─ crates/
│  ├─ mining/
│  │  └─ src/
│  │     ├─ lib.rs             # 挖矿公开 API 门面
│  │     ├─ client.rs          # 矿池客户端
│  │     ├─ config.rs          # 配置与模式
│  │     ├─ error.rs           # 错误类型
│  │     ├─ gpu.rs             # GPU 可用性与 benchmark 入口
│  │     ├─ messages.rs        # 挖矿文案与本地化
│  │     ├─ protocol.rs        # 挖矿协议 DTO
│  │     ├─ backend/
│  │     │  ├─ mod.rs          # 计算后端导出与 nonce 分配
│  │     │  ├─ cpu.rs
│  │     │  ├─ cuda.rs
│  │     │  ├─ metal.rs
│  │     │  ├─ opencl.rs
│  │     │  └─ types.rs
│  │     └─ runner/
│  │        ├─ mod.rs          # 挖矿运行流程
│  │        ├─ gpu.rs          # GPU 调优与 failover
│  │        └─ support.rs      # 共享辅助类型与工具
│  ├─ mining-cuda-sys/         # Windows CUDA FFI
│  ├─ mining-opencl-sys/       # macOS OpenCL FFI
│  └─ mining-metal-sys/        # macOS Metal FFI
├─ native/
│  ├─ mining-balance-cuda/     # 独立原生余额码工具
│  ├─ mining-invite-cuda/      # 独立原生邀请码工具
│  ├─ mining-cuda/             # CUDA 计算核心
│  ├─ mining-opencl/           # OpenCL 计算核心
│  └─ mining-metal/            # Metal 计算核心
├─ scripts/                    # 构建、release、smoke 脚本
├─ var/
│  ├─ data/                    # 账号缓存、结果文件等运行时数据
│  └─ log/                     # 运行日志
├─ dist/                       # 打包产物输出目录
├─ Cargo.toml                  # 根包 + workspace 清单
└─ README.md
```

## 模块职责

- `cli`：命令行菜单、选项输入、流程接线
- `api`：HTTP client、cookie、接口边界、接口错误
- `model`：共享 DTO / serde 结构
- `workflows`：签到、刮刮乐、羊了个羊、白嫖组合流程
- `storage`：账号缓存读写与登录态归一化
- `runtime`：路径解析、打包产物查找、legacy 数据迁移
- `solver`：羊了个羊求解逻辑
- `ui`：终端准备、固定顶部、应用内滚动日志视图、ESC 中断、单行渲染

`crates/mining` 继续负责挖矿控制面、后端测速、CPU/GPU 并挖调度与奖励码输出；原生计算实现通过 `native/` 和对应 `*-sys` crate 提供。

## 运行时文件布局

运行时文件统一放在 `var/`：

- `var/data/auth.json`
- `var/data/mining/invite-codes.txt`
- `var/data/mining/balance-codes.txt`
- `var/log/checkin/checkin.log`
- `var/log/scratch/<sanitized-email>.log`
- `var/log/sheepmatch/<sanitized-email>.log`

兼容性约定：

- 启动时会尝试把仓库根目录下的旧 `auth.json`、`invite-codes.txt`、`balance-codes.txt` 迁移到 `var/data/`。
- 账号缓存继续兼容旧单账号格式和旧 `sessions` 字段。
- 读取时会自动归一化，保存时统一写回当前格式。
- `token_type` 仍保留在账号级，默认归一为 `Bearer`。

## 打包产物布局

打包产物统一平铺在 `dist/` 根目录：

- `dist/hdd-win-x64.exe`
- `dist/hdd-macos-amd64`
- `dist/hdd-macos-arm64`

运行时查找打包文件时，会优先检查：

1. 可执行文件所在目录
2. 当前工作目录
3. workspace 根目录下的 `dist/`
4. 旧的 `artifacts/`（兼容旧布局）

## 运行

源码运行：

```bash
cargo run --release --bin hdd
```

打包后运行 `dist/` 下对应平台的产物即可。程序启动时会自动准备运行时目录，并把旧版根目录数据迁移到 `var/data/`。

## 构建与打包

### Windows x64

```bat
scripts\build-win-x64.bat
```

### macOS amd64

```bash
bash scripts/build-macos-amd64.sh
```

### macOS arm64

```bash
bash scripts/build-macos-arm64.sh
```

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

语义约定：

- 缺少某个平台的基础构建环境：该平台标记为 `failed`，其他平台继续。
- 缺少某个平台某个 GPU 后端的可选原生构建环境：该平台仍可打包，状态标记为 `built_degraded`。
- 全部完整成功：输出 `Release 打包完成。`
- 存在降级包但没有基础失败：输出 `Release 打包完成，部分平台为降级包。`

## 测试

格式检查：

```bash
cargo fmt --check
```

完整测试：

```bash
cargo test --workspace
```

快速编译检查：

```bash
cargo check --workspace
```

当前测试主要覆盖：

- 账号缓存兼容与归一化
- 根目录 / 运行时 / 打包路径解析
- 挖矿后端选择与基础行为
- 主程序中签到、刮刮乐、羊了个羊的关键批量逻辑

## 批量菜单烟测

Windows 下可运行：

```powershell
powershell -NoLogo -ExecutionPolicy Bypass -File scripts/smoke-batch-menu.ps1
```

包含刮刮乐的完整 smoke：

```powershell
powershell -NoLogo -ExecutionPolicy Bypass -File scripts/smoke-batch-menu.ps1 -SkipScratch:$false
```

这个脚本会真实驱动当前 Windows 产物，覆盖：

- 自动签到
- 自动羊了个羊
- 全自动完成所有白嫖玩法
- 可选开启自动随机刮刮乐

并检查：

- 完成提示出现
- `ESC` 返回上一级菜单可用
- 对应日志文件有增长
- 每个 flow 的会话日志会写到 `.tmp-smoke-batch/` 下
