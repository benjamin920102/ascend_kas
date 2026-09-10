# ascend_kas

**English** | [中文](README_zh.md)

Huawei Ascend 910B Kaspa Stratum miner. Rust handles Stratum protocol, job management, CPU-side share verification, and dynamic loading of the NPU kernel; Ascend C handles prehash, HeavyHash GEMM, and posthash on the NPU itself.

## Table of Contents

- [Requirements](#requirements)
- [Getting the Binary](#getting-the-binary)
  - [Option A: Download from GitHub Actions](#option-a-download-a-prebuilt-artifact-from-github-actions)
  - [Option B: Build from source](#option-b-build-from-source)
- [Quick Start](#quick-start)
- [Command-Line Options](#command-line-options)
- [Environment Variables](#environment-variables)
- [Choosing a Pool](#choosing-a-pool)
- [Build Tuning Parameters](#build-tuning-parameters-advanced)
- [Running as a systemd Service](#running-as-a-systemd-service-optional)
- [Testing](#testing)
- [Project Structure](#project-structure)
- [Troubleshooting](#troubleshooting)

## Requirements

To **run** a prebuilt binary you only need:

- Linux AArch64
- A machine with a Huawei Ascend 910B NPU and the CANN runtime installed (drivers + `libascendcl.so` on the library path)

To **build from source** you additionally need:

- Rust stable toolchain
- CMake 3.16+
- CANN/Ascend C toolkit with the `ASC_MODULES` environment variable set
- The `dav-2201` Ascend C compilation target (Ascend 910B)

## Getting the Binary

### Option A: Download a prebuilt artifact from GitHub Actions

If your repository's CI (`.github/workflows/main.yml`) has already built the project, you don't need to build anything locally:

1. Open the repository on GitHub and click the **Actions** tab.
2. Select the latest successful run of the build workflow.
3. Scroll to the **Artifacts** section at the bottom of the run summary and download the archive (it contains `ascend_kas` and `libascend_kas.so`).
4. Unzip it and place both files in the same directory, e.g. `build/`:

   ```bash
   mkdir -p build
   unzip ascend_kas-artifact.zip -d build
   chmod +x build/ascend_kas
   ```

5. Skip straight to [Quick Start](#quick-start).

> If you're pulling the artifact via the `gh` CLI instead of the web UI:
> ```bash
> gh run download <run-id> -n <artifact-name> -D build
> chmod +x build/ascend_kas
> ```

### Option B: Build from source

Only needed if you don't have a CI artifact available.

```bash
make
```

This produces:

```text
build/ascend_kas          # Rust host binary (Stratum client + CPU verification)
build/libascend_kas.so    # Ascend C NPU kernel, dynamically loaded at runtime
```

## Quick Start

Once you have `build/ascend_kas` and `build/libascend_kas.so` in place:

Here's a working example using the [Kryptex Pool](https://pool.kryptex.com/kas) Kaspa Stratum endpoint:

```bash
./build/ascend_kas \
  --pool stratum+tcp://kas.kryptex.network:7011 \
  --address kaspa:qxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx/rig1 \
  --device 0 \
  --kernel-lib build/libascend_kas.so
```

- `--pool` — your Kaspa Stratum pool's address. Kryptex's global endpoint is `stratum+tcp://kas.kryptex.network:7011` (regional endpoints like `kas-sg.kryptex.network`, `kas-us.kryptex.network`, `kas-br.kryptex.network` are also available for lower latency — see [Choosing a Pool](#choosing-a-pool))
- `--address` — your Kaspa wallet address, optionally with a worker name suffix. Kryptex uses the `WALLET_ADDRESS/WORKER_NAME` format (a slash, not a dot)
- `--device` — which Ascend NPU device index to use (`0` for the first card; check with `npu-smi info` if you have multiple)
- `--kernel-lib` — path to the compiled `libascend_kas.so`

On startup you should see log lines confirming the pool connection, a subscribed/authorized handshake, and periodic share submission logs once jobs start arriving. Press `Ctrl+C` to stop the miner cleanly.

## Command-Line Options

| Flag | Env var | Default | Description |
|---|---|---|---|
| `--pool <URL>` | `KAS_POOL` | *(required)* | Stratum pool URL, e.g. `stratum+tcp://host:port` |
| `--address <ADDR>` | `KAS_WALLET` | *(required)* | Kaspa payout wallet address (with optional `.worker` suffix) |
| `--device <N>` | `ASCEND_DEVICE_ID` | `0` | Ascend NPU device index |
| `--kernel-lib <PATH>` | `ASCEND_KAS_LIB` | `build/libascend_kas.so` | Path to the compiled NPU kernel shared library |
| `--no-cpu-verify` | — | `false` | Skip CPU-side re-verification of shares before submission (faster, less safe) |
| `--reconnect-ms <MS>` | — | `1000` | Delay before reconnecting to the pool after a dropped connection |

Run `./build/ascend_kas --help` at any time to see this list from the binary itself.

## Environment Variables

Every flag above (except `--no-cpu-verify` and `--reconnect-ms`) can also be set via environment variable, which is convenient for systemd units or Docker:

```bash
export KAS_POOL="stratum+tcp://kas.kryptex.network:7011"
export KAS_WALLET="kaspa:qxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx/rig1"
export ASCEND_DEVICE_ID=0
export ASCEND_KAS_LIB="/opt/ascend_kas/libascend_kas.so"

./build/ascend_kas
```

CLI flags take precedence over environment variables if both are set.

## Choosing a Pool

Any Kaspa Stratum-compatible pool works. This project has been tested against [Kryptex Pool](https://pool.kryptex.com/kas), which is used as the example throughout this README:

| Region | Stratum URL |
|---|---|
| Global | `stratum+tcp://kas.kryptex.network:7777` |
| Global (alt port) | `stratum+tcp://kas.kryptex.network:7011` |
| Asia | `stratum+tcp://kas-sg.kryptex.network:7011` |
| USA | `stratum+tcp://kas-us.kryptex.network:7011` |
| Brazil | `stratum+tcp://kas-br.kryptex.network:7011` |

Kryptex uses `WALLET_ADDRESS/WORKER_NAME` (slash-separated) as the `--address` value. No account registration is required — you mine straight to your wallet, and can check live stats anytime by pasting your wallet address on [pool.kryptex.com/kas](https://pool.kryptex.com/kas).

General setup for any Kaspa Stratum pool:

1. Sign up on the pool's website (if required) and copy the Stratum URL/port they provide.
2. Use your Kaspa wallet address as `--address`; check the pool's docs for whether they expect a dot (`kaspa:qxxxxx.rig1`) or a slash (`kaspa:qxxxxx/rig1`) before the worker name — Kryptex uses a slash.
3. If you run multiple Ascend cards on one machine, launch one `ascend_kas` process per device (`--device 0`, `--device 1`, …), each pointed at the same pool with a different worker suffix so you can tell them apart in the pool dashboard.

## Build Tuning Parameters (Advanced)

If you're building from source, the Ascend C kernel exposes a few compile-time tuning knobs via `make`:

```bash
make BATCH=4096 VEC_GRID=40 CUBE_GRID=20 CUBE_SINGLE_N=224
```

| Parameter | Default | Meaning |
|---|---|---|
| `BATCH` | `4096` | Number of nonces processed per NPU batch |
| `VEC_GRID` | `40` | Vector-core grid size for prehash/posthash/Keccak |
| `CUBE_GRID` | `20` | Cube-core grid size for the HeavyHash GEMM |
| `CUBE_SINGLE_N` | `224` | Preferred per-Cube-core N split. Keep this a multiple of 32 for INT8; runtime falls back to 256 if the preferred tiling is rejected. |
| `CANN_ARCH` | `dav-2201` | Target Ascend C architecture (910B) |

Larger `BATCH`/grid values can improve throughput on a busy card but increase memory use and compile time; tune experimentally and watch `npu-smi info` for utilization/memory headroom. With the default `BATCH=4096`, `CUBE_SINGLE_N=224` produces 19 N tiles instead of the original 16 at 256, improving Cube-core occupancy while preserving exact INT8×INT8→INT32 results. There's no need to touch these if you're using a prebuilt CI artifact.

## Running as a systemd Service (Optional)

For unattended/24-7 operation, create `/etc/systemd/system/ascend-kas.service`:

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

Then:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now ascend-kas
journalctl -u ascend-kas -f   # follow logs
```

## Testing

Unit tests cover Stratum message handling, job/nonce logic, and share verification — no NPU hardware is required:

```bash
cargo test --all-targets
```

## Project Structure

```text
src/main.rs                    Rust application (Stratum client, CLI, tests)
kernel/ascend_kas_vec.asc      Vector-core prehash/posthash and Keccak
kernel/ascend_kas_runtime.asc  Cube-core GEMM and ACL runtime glue
kernel/CMakeLists.txt          Ascend C kernel build configuration
Makefile                       Top-level project build (kernel + Rust)
.github/workflows/main.yml     AArch64 + CANN CI, produces build artifacts
```

## Troubleshooting

- **`error while loading shared libraries: libascendcl.so`** — the CANN runtime isn't on your library path. Source the CANN environment script (usually `source /usr/local/Ascend/ascend-toolkit/set_env.sh`) before running the miner.
- **Binary downloaded from Actions won't execute (`Permission denied`)** — run `chmod +x build/ascend_kas` after unzipping; GitHub Actions artifacts don't preserve the executable bit.
- **Connects to the pool but no shares submitted** — verify `--device` matches an actual NPU shown by `npu-smi info`, and that `--kernel-lib` points at a `libascend_kas.so` built for the `dav-2201` target.
- **Frequent reconnects** — check the pool URL/port and firewall rules; increase `--reconnect-ms` if the pool rate-limits reconnect attempts.
- **Low hashrate** — see [Build Tuning Parameters](#build-tuning-parameters-advanced); note tuning requires rebuilding the kernel, so it doesn't apply if you're only using a prebuilt CI artifact.
