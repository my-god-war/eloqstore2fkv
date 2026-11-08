# 引擎级交叉证据（V4 / V3）

- 生成日期：2026-11-06
- 执行环境：Ubuntu 26.04 arm64 容器（Docker Desktop / Apple Silicon），Linux 4 KiB 页
  （`getconf PAGESIZE = 4096`），GCC 15.2.0，CMake 4.2.3，Rust nightly 1.101.0
  （`2d20b42b8 2026-11-04`）
- 证据修订（锚定内容树，不是可长期 checkout 的 SHA pin）：
  - eloqstore `82b774c62728b023b03c6e5e84e69416ffec9d74`（`COMPAT_BASELINE`
    首行，构建前断言）
  - fkv commit `d9d0ecfa5aa5ea86efd9e5670d1d8ab86156b35d`，内容树
    `ea15d6409a775b6f695ed051c68b582978b72448`（2026-11-06 运行；commit 时间
    2026-11-05T22:45:18Z）
  - eloqstore2fkv `23a3f46e46a25bfe4752a3d8b5bcee664343aab7`（本次运行时的
    driver / test 代码修订）
- pin 语义：fkv 主线会被周期性重建（2026-11-06 当天已从 `e2bd232` 重建为
  `80d91ea init` 后继续推进），任何 commit SHA 都会在 rewrite 后不可达——实测
  `git cat-file -t d9d0ecf…` 失败，`git merge-base --is-ancestor d9d0ecf…
  origin/main` 无法解析。因此本仓不再把「SHA = `origin/main` HEAD」当作可复现性
  依据：
  - 当前证明锚定 2026-11-06 运行的 commit `d9d0ecf` 与内容树 `ea15d64…`。
    commit SHA 不可跨 history rewrite 复现，tree hash 可以——tree 由内容决定，
    只要任一 clone / GitHub API 能取到同一内容树即可核对，不要求该 commit 仍在
    ref 历史中（该 tree 经 GitHub API 实测仍可解析）；
  - `.github/workflows/engine.yml` 不写死 fkv SHA，默认 checkout fkv `main`
    HEAD；每次运行把实际 `git rev-parse HEAD` 与 `HEAD^{tree}` 写入 step
    summary 与日志；
  - `workflow_dispatch` 的 `fkv_sha` 可对候选 commit 复跑同一链路（checkout
    后断言实际 HEAD），`fkv_tree` 可断言候选修订的 tree 等于已归档证据 tree；
  - 复跑后以新记录的 commit / tree 更新本文与 `COMPAT_BASELINE` 证据段。未在
    当前 main 上重跑时，L1 证据只覆盖上述 tree，不外推到当前 HEAD。
- CI 记录：`engine.yml` 不写死 fkv SHA（history rewrite 会让 SHA 失效），默认
  checkout fkv `main` HEAD；checkout 后计算 `git -C fkv rev-parse HEAD` /
  `HEAD^{tree}`，记录到 step summary 与日志。`workflow_dispatch` 支持 `fkv_sha`
  （验证候选 commit，checkout 后断言实际 HEAD）与 `fkv_tree`（断言候选修订的
  内容树等于已归档证据 tree）。fkv 主线推进不会自动触发本仓 workflow，发布前用
  `fkv_sha` 对目标 HEAD 重跑一轮，成功后把 step summary 记录的 commit / tree
  同步到本文与 `COMPAT_BASELINE` 证据段。eloqstore 由
  `scripts/fetch_eloqstore.sh` 按 `COMPAT_BASELINE` 首行 pin，构建前再断言一次。
- 与当前主线的关系：本次证据运行于 2026-11-06，fkv commit `d9d0ecf`、内容树
  `ea15d64…`。随后 fkv 主线被重建为 `80d91ea`（init）并继续推进（同日实测
  `origin/main = 50c27aa`，tree `3e9596d…`；本地 `main = ac9f805`，tree
  `7f6b444…`），均不同于证据 tree。旧 pin `6fd58ad`（parent `2a87472`）与更早
  的 `e2bd232` 同样不在当前历史。因此本证据**不再声称对当前主线树可复现**：
  它证明的是 tree `ea15d64…` 的内容；当前 main 需要重新 dispatch 后才形成新的
  证据记录。未重跑前，L1 的「当前性」以本节记录为准，不外推到 HEAD。
- 范围：Local 后端，覆盖两种分配模式——`data_append_mode=false`（默认 / Rust
  `AllocMode::Pooled`）与 `data_append_mode=true`（Rust `AllocMode::Append`，
  由 `ENGINE_APPEND_MODE=1` / `FKV_ENGINE_APPEND_MODE=1` 切换）；`engine.yml`
  以 `pooled` / `append` matrix 各执行一轮同一链路；`enable_compression=false`，
  单 shard、单表 `demo.1`、4 KiB 页；overflow 页大 value（102400 B）已覆盖，
  未覆盖同侧跨重启续写（reopen 后继续写）、segment 级 large-value / TTL /
  archive / branch / 压缩开关 / cloud / crash 注入 / 多页与邻近重启点边界。
  这正是 `COMPAT_BASELINE` 声明的 L1 范围。

## 结论

### pooled 模式

| 项 | 命令 | 结果 |
|---|---|---|
| C++ 引擎写 64 key | `engine_driver write <store> demo.1 c 64 cpp` | exit 0 |
| Rust 打开 C++ store、读 64 key、续写 16 key | `cargo test -p fkv_engine_cross --test engine_cross` | 3 passed |
| C++ 读回 Rust 续写 | `engine_driver verify <store> demo.1 r 16 rust` | exit 0 |
| C++ 读回自己的写 | `engine_driver verify <store> demo.1 c 64 cpp` | exit 0 |
| 同一 op log 双端执行 + dump | `engine_driver apply/dump` + `cargo test ... --test oplog_diff` | 1 passed |
| op log dump 逐字节 diff | `diff -u cpp.dump rust.dump` | 无输出（一致） |

### append 模式（`ENGINE_APPEND_MODE=1` / `FKV_ENGINE_APPEND_MODE=1`）

同一链路全部重跑：

| 项 | 结果 |
|---|---|
| C++ 引擎写 64 key（append） | exit 0 |
| Rust 读 64 + 续写 16（`AllocMode::Append`） | 3 passed |
| C++ 读回 Rust 续写 / 自己的写 | exit 0 / exit 0 |
| op log 双端执行 + dump | exit 0 / exit 0 |
| op log dump diff | 无输出（一致） |

### overflow / 大 value（4×102400 B，两种分配模式）

`value` 取 102400 B（4 KiB 页装不下，单条约 26 个 overflow 页，跨
`overflow_pointers` 分组）。C++ driver 新增两个命令（key 仍为 `<prefix>%04d`、
ts=1，value 为 `<size>` 个 `<byte>`）：

- `engine_driver write_repeat  <root> <table> <prefix> <count> <size> <byte>`
- `engine_driver verify_repeat <root> <table> <prefix> <count> <size> <byte>`

Rust 侧 `engine_cross.rs` 新增 `cpp_overflow_rust_read_then_rust_write`：读 C++ 写的
`L0000..L0003`（102400 B `'x'`），续写 `M0000..M0003`（102400 B `'y'`，ts=2）。
两个用例共用同一 store，测试内用进程级锁串行，避免并发 reopen / 续写。
CI 的 `engine` workflow 同时执行 `write` 与 `write_repeat`，并设置
`FKV_REQUIRE_OVERFLOW=1`：store 无 `L0000` 时该用例失败而不是跳过。所有引擎
用例都带 `#[ignore]`——常规 `cargo test --workspace` 报 ignored 而不是假绿；
证据运行显式 `--ignored`，此时缺 `FKV_CPP_STORE` / `FKV_OPLOG` 等 env 直接失败。

| 步骤 | pooled | append |
|---|---|---|
| C++ `write demo.1 c 64 cpp`（既有语料，保证整套用例可跑） | exit 0 | exit 0 |
| C++ `write_repeat demo.1 L 4 102400 x` | exit 0 | exit 0 |
| Rust 读 L + 续写 M（`cargo test ... --test engine_cross`） | 3 passed | 3 passed |
| C++ `verify_repeat demo.1 M 4 102400 y`（读回 Rust overflow） | exit 0 | exit 0 |
| C++ `verify_repeat demo.1 L 4 102400 x`（自检） | exit 0 | exit 0 |
| C++ `verify demo.1 r 16 rust` / `verify demo.1 c 64 cpp` | exit 0 / exit 0 | exit 0 / exit 0 |

盘面产物（Rust 续写后）：

```text
# pooled
demo.1/data_0_main_0        864256 bytes   # 211 个 4 KiB 页（普通页 + overflow 链）
demo.1/manifest_main_0       16384 bytes
# append（ENGINE_APPEND_MODE=1 / FKV_ENGINE_APPEND_MODE=1）
demo.1/data_0_main_0      1073741824 bytes # pages_per_file_shift=18 预分配
demo.1/manifest_main_0       16384 bytes
```

本次 Docker Desktop（virtiofs）上 append 的 `data_0_main_0` `du` 显示实际占用
1.0 GiB（`stat` blocks = 2097152），不是稀疏文件；文件是否稀疏取决于宿主 / CI
文件系统对 C++ `fallocate` 预分配的处理。

两种模式下 C++ / Rust 都是独立进程先后打开同一 store，因此覆盖的是 overflow
页链的落盘编码、指针分组与 page id 分配跨语言互操作，而非进程内缓存。

基本链路（64 + 16 key）盘面产物：pooled 模式 `data_0_main_0` 12288 B + `manifest_main_0` 8192 B；
append 模式 `data_0_main_0` 1073741824 B（C++ 按 `pages_per_file_shift=18`
预分配 1 GiB 文件；本次 virtiofs 上实际占用 1.0 GiB，见上）+ `manifest_main_0` 8192 B。

```text
demo.1/data_0_main_0    12288 bytes            # pooled
demo.1/data_0_main_0    1073741824 bytes       # append（预分配）
demo.1/manifest_main_0   8192 bytes
```

op log dump 各 2 行（`basic.oplog` 的 2 条记录），逐字节一致。

## 复现

本地（macOS + Docker）按以下步骤执行；CI 走 `.github/workflows/engine.yml` 的同一链路。

1. 构建 C++ 引擎驱动（容器内）：

```bash
docker run -d --name fkv-engine --security-opt seccomp=unconfined --ulimit memlock=-1 \
  -v "$PWD/../eloqstore:/src/eloqstore" \
  -v "$PWD:/src/compat:ro" \
  -v /tmp/fkv_engine_build:/build \
  -v /tmp/fkv_engine_data:/data \
  ubuntu:26.04 sleep infinity
docker exec fkv-engine bash -c "apt-get update && \
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  build-essential cmake ninja-build git curl ca-certificates pkg-config \
  libboost-all-dev libjsoncpp-dev libzstd-dev libssl-dev libaio-dev liburing-dev \
  libgoogle-glog-dev libgflags-dev libcurl4-openssl-dev libprotobuf-dev \
  libprotoc-dev protobuf-compiler libleveldb-dev libzmq3-dev cppzmq-dev \
  libjitterentropy3-dev"
docker exec fkv-engine bash -c "cmake -B /build -S /src/compat/cpp/engine_driver \
  -G Ninja -DCMAKE_BUILD_TYPE=Release -DELOQSTORE_SRC=/src/eloqstore \
  -DCMAKE_C_FLAGS=-D_GNU_SOURCE && cmake --build /build --target engine_driver -j\$(nproc)"
```

2. C++ 写 store：

```bash
docker exec fkv-engine /build/engine_driver write /data/store demo.1 c 64 cpp
```

3. 同一容器内跑 Rust（Linux 4 KiB 页，manifest framing 与 C++ 一致）：

```bash
docker commit fkv-engine fkv-engine:ready
docker run -d --name fkv-engine-run --security-opt seccomp=unconfined --ulimit memlock=-1 \
  -v "$PWD/../fkv:/src/fkv" -v "$PWD:/src/compat" \
  -v /tmp/fkv_engine_build:/build -v /tmp/fkv_engine_data:/data \
  -v /tmp/fkv_target:/target fkv-engine:ready sleep infinity
docker exec -e RUSTUP_TOOLCHAIN=nightly -e CARGO_TARGET_DIR=/target \
  -e FKV_CPP_STORE=/data/store fkv-engine-run \
  bash -lc 'cd /src/compat && cargo test -p fkv_engine_cross --test engine_cross -- --ignored --nocapture'
```

4. C++ 读回：

```bash
docker exec fkv-engine /build/engine_driver verify /data/store demo.1 r 16 rust
docker exec fkv-engine /build/engine_driver verify /data/store demo.1 c 64 cpp
```

5. overflow / 大 value（pooled；append 模式对两侧命令加 `ENGINE_APPEND_MODE=1` /
   `FKV_ENGINE_APPEND_MODE=1`，store 换成 `/data/store_overflow_append`）：

```bash
docker exec fkv-engine /build/engine_driver write /data/store_overflow demo.1 c 64 cpp
docker exec fkv-engine /build/engine_driver write_repeat /data/store_overflow demo.1 L 4 102400 x
docker exec -e RUSTUP_TOOLCHAIN=nightly -e CARGO_TARGET_DIR=/target \
  -e FKV_CPP_STORE=/data/store_overflow -e FKV_REQUIRE_OVERFLOW=1 fkv-engine-run \
  bash -lc 'cd /src/compat && cargo test -p fkv_engine_cross --test engine_cross -- --ignored --nocapture'
docker exec fkv-engine /build/engine_driver verify_repeat /data/store_overflow demo.1 M 4 102400 y
docker exec fkv-engine /build/engine_driver verify_repeat /data/store_overflow demo.1 L 4 102400 x
```

6. op log 差分：

```bash
docker exec fkv-engine /build/engine_driver apply /data/oplog-cpp demo.1 \
  /src/compat/cpp/engine_driver/tests/oplog/basic.oplog
docker exec fkv-engine /build/engine_driver dump /data/oplog-cpp demo.1 /data/cpp.dump
docker exec -e RUSTUP_TOOLCHAIN=nightly -e CARGO_TARGET_DIR=/target \
  -e FKV_OPLOG=/src/compat/cpp/engine_driver/tests/oplog/basic.oplog \
  -e FKV_RUST_STORE=/data/oplog-rust -e FKV_RUST_DUMP=/data/rust.dump \
  fkv-engine-run bash -lc 'cd /src/compat && cargo test -p fkv_engine_cross --test oplog_diff -- --ignored --nocapture'
diff -u /tmp/fkv_engine_data/cpp.dump /tmp/fkv_engine_data/rust.dump
```

`cpp/engine_driver/CMakeLists.txt` 已补两项 Ubuntu 26.04 链接修复：
`ZLIB::ZLIB` + `-Wl,--copy-dt-needed-entries`（libcurl 的 zlib DT_NEEDED）与
`jitterentropy`（静态 `libcrypto.a` 的 OpenSSL 3.5+ 熵源符号）。

## 覆盖与边界

- CI 覆盖（`.github/workflows/engine.yml`）：`pooled` / `append` matrix 各一轮，
  每轮 64+16 key 双向、overflow 4×102400 B 双向、op log 双端 dump 逐字节 diff；
  构建前断言 eloqstore `82b774c` 的 pin，fkv 不写死 SHA、运行时记录实际
  commit / tree（本次证据为 `d9d0ecf` / `ea15d64…`）。配置变更后以 Actions
  实际 run 为准；本文记录的是 2026-11-06 在 fkv `d9d0ecf`（tree `ea15d64…`）
  上的本地执行结果。
- 证据时效：本文的 fkv 证据锚定 tree `ea15d64…`（2026-11-06 运行时的 commit
  `d9d0ecf`）；该 commit 在后续 history rewrite 后可能不可达，以 tree hash 与
  本文记录为准。fkv 后续提交不会自动重跑 engine cross（本仓 workflow 无法监听
  fkv 仓 push）。发布前必须用 `workflow_dispatch` 的 `fkv_sha` 对最新 HEAD 跑
  一轮，把新的 commit / tree 记录回本文与 `COMPAT_BASELINE`；未重跑时，L1 结论
  只覆盖证据 tree，不得直接外推到 HEAD。
- 已覆盖场景：引擎级 C++ 写 → Rust 读/续写 → C++ 读回（V4），pooled 与 append
  两种分配模式；overflow 页大 value（4×102400 B）双向读写；同 op log 双端结果
  diff（V3 的基本形态）；页 / index / manifest 的格式层双向语料与 `fkv_compat`
  （V1/V2）此前已闭环。
- 未覆盖场景：同侧跨重启续写（写 → 关闭 → reopen → 继续写，当前只在测试开始时
  reopen 一次，`RESTART_INTERVAL` 仅是构造参数）；segment 级 large-value 布局；
  TTL 双树；archive / branch manifest；压缩开关（`enable_compression=true`）；
  cloud / standby；崩溃注入（V5 的 C++ 差分）；多页 / 邻近重启点边界；
  性能（V6，需 Linux 4 KiB 页；盘型不限）。
- 结论仅对 `COMPAT_BASELINE` 的 L1 范围成立：Local + pooled/append 分配 + 无压缩 +
  同页大小主机 + 上述未覆盖项不纳入保证。
