# hdd

`hdd` 是号多多脚本整合工具的统一根工程。

它把原先分散的多个 Go CLI 和 Windows GPU sidecar 整合到一个统一入口下，保留原有业务逻辑和用户可见的中文输出风格，同时统一了账号缓存、路径解析、日志落点、打包路径和交互菜单。

## 当前功能

程序启动后会先显示：

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

运行时默认只走自动调优模式。

当环境满足 **Windows x64 + 可用 NVIDIA 环境 + 对应 GPU sidecar 存在** 时，程序会继续让用户选择：

1. GPU 挖矿
2. CPU 挖矿
3. 返回上一级菜单
4. 退出脚本

否则自动回退到 CPU 挖矿。

挖矿运行过程中支持按 `ESC` 返回上一级菜单，终端第一行会固定显示提示。

### 多账号批量功能

程序统一使用根目录 `auth.json` 管理账号。

批量功能菜单支持：

1. 查询所有账号余额
2. 自动签到
3. 自动随机刮刮乐
4. 自动羊了个羊
5. 返回上一级菜单
6. 退出脚本

特性说明：

- 登录逻辑复用统一 `internal/auth` 模块。
- 所有批量业务都按账号并发执行。
- 自动签到写共享日志。
- 自动随机刮刮乐和自动羊了个羊按账号写日志。
- 批量业务完成后不会自动退出，而是停留在“按 ESC 返回上一级菜单”的状态。

## 目录结构

当前整合后的主干结构如下：

```text
hdd/
├─ cmd/
│  └─ hdd/
├─ internal/
│  ├─ auth/
│  ├─ cli/
│  ├─ client/
│  ├─ config/
│  ├─ features/
│  │  ├─ checkin/
│  │  ├─ scratch/
│  │  └─ sheepmatch/
│  ├─ logging/
│  ├─ mining/
│  ├─ model/
│  ├─ solver/
│  └─ terminal/
├─ scripts/
├─ dist/
├─ log/
├─ native/
│  ├─ hdd-miner-gpu/
│  ├─ invite-miner-gpu/
│  └─ balance-miner-gpu/
├─ auth.json
├─ go.mod
└─ README.md
```

说明：

- `cmd/hdd` 是统一程序入口。
- `internal/` 是当前主工程实际使用的共享和业务代码。
- `native/` 下保留当前仍需存在的 Windows GPU sidecar 原生工程。
- `dist/` 直接平铺最终产物。
- `log/` 统一承接运行日志和结果文件。

## auth.json

统一账号缓存文件是根目录的 `auth.json`。

当前格式：

```json
{
  "base_url": "https://sub.hdd.sb",
  "selected_email": "demo@example.com",
  "accounts": [
    {
      "email": "demo@example.com",
      "password": "***",
      "token_type": "Bearer",
      "access_token": "***"
    }
  ]
}
```

兼容性约定：

- 继续兼容旧单账号格式和旧 `sessions` 字段。
- 读取时会自动归一化。
- 保存时统一写回当前格式。
- `token_type` 仍保留在账号级，默认归一为 `Bearer`。

## 日志路径

统一日志根目录：`log/`

当前主要落点：

- `log/checkin/checkin.log`
- `log/scratch/<sanitized-email>.log`
- `log/sheep-match/<sanitized-email>.log`
- `log/mining/system/invite-codes.txt`
- `log/mining/system/balance-codes.txt`

## 测试组织

当前 Go 测试按标准 colocated 方式放在源码同级：

- `internal/auth/*_test.go`
- `internal/cli/*_test.go`
- `internal/features/checkin/*_test.go`
- `internal/features/scratch/*_test.go`
- `internal/features/sheepmatch/*_test.go`
- `internal/mining/*_test.go`

这是 Go 工程的常见规范，便于和被测代码一起演进。

## 构建与打包

### Windows x64

构建主程序：

```bat
scripts\build-win-x64.bat
```

产物：

- `dist/hdd-win-x64.exe`

完整 release：

```bat
scripts\release.bat
```

如果要在 release 前先跑一次批量菜单真实烟测：

```bat
scripts\release.bat --with-batch-smoke
```

### macOS amd64

```bash
bash scripts/build-macos-amd64.sh
```

产物：

- `dist/hdd-macos-amd64`

### macOS arm64

```bash
bash scripts/build-macos-arm64.sh
```

产物：

- `dist/hdd-macos-arm64`

## 批量菜单烟测

Windows 下可运行：

```bat
scripts\smoke-batch-menu.bat
```

这个脚本会真实驱动统一菜单，验证：

- 查询所有账号余额
- 自动签到
- 自动随机刮刮乐
- 自动羊了个羊

并检查：

- 完成提示出现
- `ESC` 返回上一级菜单可用
- 对应日志文件有增长
- 截图产物会写到 `.tmp-smoke-script/`

## 说明

当前仓库已经完成统一入口和主流程整合，但 Windows GPU 挖矿仍通过 sidecar 方式保留为独立原生工程。这是当前跨语言整合下较稳妥的发布方式：

- 主程序负责统一菜单、账号、日志、路径和运行时分发
- GPU sidecar 负责原生 CUDA 挖矿
- 当 sidecar 缺失或环境不满足时，主程序会自动回退到 CPU 挖矿
