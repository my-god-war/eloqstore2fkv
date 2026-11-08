# test-23 compaction 页缓存清空调用零判别力：删掉 `clear_page_cache` 后全套件仍绿（语义域探针实证 load-bearing 的陈旧读窗口无测试锁定）

- 领域：测试 / 失败路径（`6f79f44` 页缓存 × compaction 失效矩阵）
- 优先级：P3（代码正确、防护在位；缺的是回归锁——第 32 轮语义域 M2 变异
  「删 compact.rs 开批前 `clear_page_cache`」全套件全绿，但探针实证了
  「快照回填退役页 → maintain 回池 → compact 裸写复用同 id」的真实陈旧
  读窗口，防护是 load-bearing 的）
- 置信度：高（变异 + 探针双证；探针场景由语义域子代理构造并验证后丢弃）

## 现象

`compact.rs` 开批前 `clear_page_cache` 覆盖成功 / IO 失败 / 零搬迁全部
出口（模块文档自述），但删除该调用后 `fkv_store` 全套件（387 例）不红。
失效矩阵其余两处（freed 逐条失效、失败批清空）分别有纵深防御与判别测试，
唯独 compaction 清空裸奔。

## 建议（已实施）

`fkv_store/tests/page_cache.rs` 新增
`compact_clears_page_cache_before_moving_pages`：8 页/文件夹具 + 删 file 0
前段 key 制造压缩候选 + 读热缓存（断言 bytes > 0 锁前提）→ `compact_file`
（断言 moved > 0）→ **断言 `read_cache_stat()` 占用归零**（删掉清空调用
即红）→ 幸存 key 全部可读（占用断言先行，避免回读回填干扰）。

## 处置记录

- 2026-11-07 第 32 轮语义域深审 `6f79f44` 时发现并移交协调者；实现 fkv
  commit `7179442`。
- 附带：`fkv_failpoint/src/lib.rs` 增补「进程级注册表 × libtest 并行串扰」
  文档声明（第 19/32 轮两次观察的收口，fkv commit `4c40a75`）——nextest
  门禁（每用例独立进程）无此问题，`cargo test` 调试需 `RUST_TEST_THREADS=1`。

## 复审记录（2026-11-07，第 33 轮结构域）

- 判定：**达标**，移入 reviewed/。断言顺序设计核实（前提 bytes>0 →
  moved>0 → 占用归零 → 回读后置）；**变异复核红**（fkv-mut 注释
  `compact.rs:34` 的 `clear_page_cache` → fail 于 `(4608, 9) != (0, 0)`），
  还原后复绿；随附 failpoint 文档声明到位。
