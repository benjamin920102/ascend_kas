# ascend_kas

[English](README.md) | **中文**

华为昇腾（Ascend）910B Kaspa Stratum 矿机程序。Rust 部分负责 Stratum 协议、任务管理、CPU 端算力验证以及 NPU 内核的动态加载；Ascend C 部分负责在 NPU 上执行 prehash（预哈希）、HeavyHash GEMM 矩阵运算以及 posthash（后哈希）。

## 目录

- [环境要求](#环境要求)
- [获取可执行文件](#获取可执行文件)
  - [方案 A：从 GitHub Actions 下载预编译产物](#方案-a从-github-actions-下载预编译产物)
  - [方案 B：从源码编译](#方案-b从源码编译)
- [快速开始](#快速开始)
- [命令行参数](#命令行参数)
- [环境变量](#环境变量)
- [选择矿池](#选择矿池)
- [编译调优参数（进阶）](#编译调优参数进阶)
- [作为 systemd 服务运行（可选）](#作为-systemd-服务运行可选)
- [测试](#测试)
- [项目结构](#项目结构)
- [故障排查](#故障排查)

## 环境要求

只是**运行**预编译好的可执行文件，你只需要：

- Linux AArch64（ARM64）系统
- 一台装有华为昇腾 910B NPU 且已安装 CANN 运行时的机器（驱动 + `libascendcl.so` 需在库路径中）

如果要**从源码编译**，还需要：

- Rust 稳定版工具链
- CMake 3.16 及以上
- CANN / Ascend C 工具包，并设置好 `ASC_MODULES` 环境变量
- 支持 `dav-2201` 编译目标（对应 Ascend 910B）

## 获取可执行文件

### 方案 A：从 GitHub Actions 下载预编译产物

如果你的仓库 CI（`.github/workflows/main.yml`）已经构建完成，**不需要在本地重新编译**，直接下载产物即可：

1. 打开仓库的 GitHub 页面，点击 **Actions** 标签页。
2. 选择最近一次成功运行的构建工作流（workflow run）。
3. 在该次运行结果页面底部找到 **Artifacts（构建产物）** 区块，下载压缩包（内含 `ascend_kas` 和 `libascend_kas.so`）。
4. 解压后，把两个文件放到同一目录下，例如 `build/`：

   ```bash
   mkdir -p build
   unzip ascend_kas-artifact.zip -d build
   chmod +x build/ascend_kas
   ```

5. 完成后直接跳到 [快速开始](#快速开始)。

> 如果你习惯用 `gh` 命令行工具而不是网页下载：
> ```bash
> gh run download <run-id> -n <artifact-name> -D build
> chmod +x build/ascend_kas
> ```

### 方案 B：从源码编译

只有在没有可用的 CI 构建产物时才需要这一步。

```bash
make
```

会生成：

```text
build/ascend_kas          # Rust 主程序（Stratum 客户端 + CPU 端验证）
build/libascend_kas.so    # Ascend C NPU 内核，运行时动态加载
```

## 快速开始

准备好 `build/ascend_kas` 和 `build/libascend_kas.so` 两个文件后，运行：

以下是使用 [Kryptex Pool](https://pool.kryptex.com/kas) Kaspa Stratum 节点的实际示例：

```bash
./build/ascend_kas \
  --pool stratum+tcp://kas.kryptex.network:7011 \
  --address kaspa:qxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx/rig1 \
  --device 0 \
  --kernel-lib build/libascend_kas.so
```

- `--pool`：Kaspa Stratum 矿池地址。Kryptex 的全球节点为 `stratum+tcp://kas.kryptex.network:7011`（也提供 `kas-sg.kryptex.network`、`kas-us.kryptex.network`、`kas-br.kryptex.network` 等区域节点以降低延迟，详见[选择矿池](#选择矿池)）
- `--address`：你的 Kaspa 钱包地址，可选附加矿工名后缀。Kryptex 使用 `钱包地址/矿工名`（斜杠分隔）的格式
- `--device`：使用第几张 Ascend NPU 卡（`0` 表示第一张；如有多卡，可用 `npu-smi info` 查看卡号）
- `--kernel-lib`：编译好的 `libascend_kas.so` 的路径

启动后，日志中应能看到矿池连接成功、订阅/授权握手完成的信息，随后在收到任务后会周期性打印提交算力（share）的日志。按 `Ctrl+C` 可正常停止矿机。

## 命令行参数

| 参数 | 对应环境变量 | 默认值 | 说明 |
|---|---|---|---|
| `--pool <URL>` | `KAS_POOL` | *（必填）* | Stratum 矿池地址，如 `stratum+tcp://host:port` |
| `--address <ADDR>` | `KAS_WALLET` | *（必填）* | 用于收款的 Kaspa 钱包地址（可选加 `.矿工名` 后缀） |
| `--device <N>` | `ASCEND_DEVICE_ID` | `0` | 使用的 Ascend NPU 设备编号 |
| `--kernel-lib <PATH>` | `ASCEND_KAS_LIB` | `build/libascend_kas.so` | 编译好的 NPU 内核动态库路径 |
| `--no-cpu-verify` | — | `false` | 提交前跳过 CPU 端的算力再验证（速度更快，但安全性降低） |
| `--reconnect-ms <MS>` | — | `1000` | 与矿池断线后重连的等待时间（毫秒） |

随时可运行 `./build/ascend_kas --help` 查看程序内置的完整参数说明。

## 环境变量

以上除 `--no-cpu-verify` 和 `--reconnect-ms` 外的所有参数，都可以通过环境变量设置，这在配置 systemd 服务或 Docker 时很方便：

```bash
export KAS_POOL="stratum+tcp://kas.kryptex.network:7011"
export KAS_WALLET="kaspa:qxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx/rig1"
export ASCEND_DEVICE_ID=0
export ASCEND_KAS_LIB="/opt/ascend_kas/libascend_kas.so"

./build/ascend_kas
```

如果命令行参数和环境变量同时设置，**命令行参数优先**。

## 选择矿池

任何兼容 Kaspa Stratum 协议的矿池都可以使用。本项目已在 [Kryptex Pool](https://pool.kryptex.com/kas) 上测试，本 README 中的示例均以其为例：

| 区域 | Stratum 地址 |
|---|---|
| 全球 | `stratum+tcp://kas.kryptex.network:7777` |
| 全球（备用端口） | `stratum+tcp://kas.kryptex.network:7011` |
| 亚洲 | `stratum+tcp://kas-sg.kryptex.network:7011` |
| 美国 | `stratum+tcp://kas-us.kryptex.network:7011` |
| 巴西 | `stratum+tcp://kas-br.kryptex.network:7011` |

Kryptex 的 `--address` 采用 `钱包地址/矿工名`（斜杠分隔）格式，**无需注册账号**，直接挖到你的钱包即可；随时可在 [pool.kryptex.com/kas](https://pool.kryptex.com/kas) 粘贴你的钱包地址查看实时算力和收益。

使用其他 Kaspa Stratum 矿池的通用流程：

1. 在矿池官网注册账号（如需要），获取其提供的 Stratum 地址和端口。
2. 使用你的 Kaspa 钱包地址作为 `--address`；请查阅该矿池文档确认矿工名前是用点（`kaspa:qxxxxx.rig1`）还是斜杠（`kaspa:qxxxxx/rig1`）分隔 —— Kryptex 使用斜杠。
3. 如果同一台机器上有多张 Ascend 卡，需要为每张卡单独启动一个 `ascend_kas` 进程（分别使用 `--device 0`、`--device 1` 等），并使用不同的矿工名后缀，方便在矿池后台区分各张卡的算力。

## 编译调优参数（进阶）

如果你是从源码编译的，Ascend C 内核提供了几个可在编译时调整的参数，通过 `make` 传入：

```bash
make BATCH=4096 VEC_GRID=40 CUBE_GRID=20 CUBE_SINGLE_N=224
```

| 参数 | 默认值 | 含义 |
|---|---|---|
| `BATCH` | `4096` | 每批次在 NPU 上处理的 nonce 数量 |
| `VEC_GRID` | `40` | 用于 prehash / posthash / Keccak 的向量核心（Vector Core）网格大小 |
| `CUBE_GRID` | `20` | 用于 HeavyHash GEMM 的矩阵核心（Cube Core）网格大小 |
| `CUBE_SINGLE_N` | `224` | 每个 Cube 核优先处理的 N 轴大小。INT8 建议保持 32 的倍数；若当前 CANN 不接受则回退到 256。 |
| `CANN_ARCH` | `dav-2201` | 目标 Ascend C 架构（对应 910B） |

调大 `BATCH` 或网格参数在满载的卡上可能提升吞吐量，但会增加显存占用和编译时间；建议通过实验调优，并用 `npu-smi info` 观察利用率和显存余量。如果你使用的是 CI 预编译产物，这些参数无需关心。

## 作为 systemd 服务运行（可选）

若需要无人值守 / 7×24 小时运行，可创建 `/etc/systemd/system/ascend-kas.service`：

```ini
[Unit]
Description=Ascend Kaspa Stratum Miner
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/opt/ascend_kas
Environment=KAS_POOL=stratum+tcp://kas.kryptex.network:7011
Environment=KAS_WALLET=kaspa:qxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx/rig1
Environment=ASCEND_DEVICE_ID=0
Environment=ASCEND_KAS_LIB=/opt/ascend_kas/libascend_kas.so
ExecStart=/opt/ascend_kas/ascend_kas
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

然后执行：

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now ascend-kas
journalctl -u ascend-kas -f   # 实时查看日志
```

## 测试

单元测试覆盖了 Stratum 消息处理、任务/nonce 逻辑以及算力验证，**无需 NPU 硬件**即可运行：

```bash
cargo test --all-targets
```

## 项目结构

```text
src/main.rs                    Rust 主程序（Stratum 客户端、命令行、单元测试）
kernel/ascend_kas_vec.asc      向量核心（Vector Core）的 prehash / posthash 与 Keccak 实现
kernel/ascend_kas_runtime.asc  矩阵核心（Cube Core）的 GEMM 与 ACL 运行时对接代码
kernel/CMakeLists.txt          Ascend C 内核的编译配置
Makefile                       项目顶层构建脚本（内核 + Rust）
.github/workflows/main.yml     AArch64 + CANN 的 CI 流水线，产出构建产物
```

## 故障排查

- **`error while loading shared libraries: libascendcl.so`**：CANN 运行时不在库路径中。运行矿机前先执行 CANN 环境初始化脚本（通常是 `source /usr/local/Ascend/ascend-toolkit/set_env.sh`）。
- **从 Actions 下载的可执行文件无法运行（`Permission denied`）**：解压后运行 `chmod +x build/ascend_kas`；GitHub Actions 的构建产物不会保留可执行权限位。
- **能连上矿池但一直没有提交算力**：确认 `--device` 对应的是 `npu-smi info` 中实际存在的 NPU 卡号，并确认 `--kernel-lib` 指向的是针对 `dav-2201` 目标编译的 `libascend_kas.so`。
- **频繁断线重连**：检查矿池地址、端口和防火墙设置；如果矿池对重连频率有限制，可适当调大 `--reconnect-ms`。
- **算力偏低**：参考[编译调优参数（进阶）](#编译调优参数进阶)；注意调优需要重新编译内核，如果你只是使用 CI 预编译产物则不适用。
