# 惰性分配对照实验（附录）

对照组：commit `7608a70`（`mmap`/`sbrk` 为 eager，逐页 `map_one`）。
实现组：当前 lazy 版本（`mmap`/`append_to` 只登记或扩展 `MapArea`，缺页时 `lazy_map` 现场分配）。

测试程序：`user/src/bin/ch4_lazy_{correct,mem,perf,bad,tradeoff}.rs`，只使用既有系统调用。
两版都用 `make -C os kernel TEST=4 BASE=2 CHAPTER=4` 构建；做时间与连续性实验时 `link_app.S` 只保留被测应用，
避免抢占和其他任务分配物理帧的干扰。

## 空间

| 测试 | eager | lazy |
|---|---|---|
| `sbrk(256 MiB)`（RAM 仅 128 MiB） | 内核 panic：`frame_alloc().unwrap()` 为 `None`（`memory_set.rs:355`） | 成功，只访问首/中/尾三页 |
| `mmap(256 MiB)` | 未执行到 | 成功，访问首/尾页 |

## 时间

申请 1000 页、只访问其中 k 页（µs）：

| k | eager 分配 | eager 访问 | lazy 分配 | lazy 访问 |
|---:|---:|---:|---:|---:|
| 10 | 837 | 15 | 19 | 125 |
| 100 | 923 | 26 | 23 | 1275 |
| 500 | 922 | 83 | 21 | 5955 |
| 1000 | 734 | 152 | 21 | 11154 |

- eager 总成本 ≈ `850 + 0.15k`，lazy ≈ `20 + 11k`，交叉点 `k ≈ 76`（7.6%）。
- 已映射内存的 2 MiB 顺序读：eager 301 µs、lazy 300 µs，无差异。QEMU 用 TCG + 平坦内存映射执行访存
  （guest 物理地址只是 host RAMBlock 的偏移，两种布局最终都落在同一块 host 内存里），不建模 cache 与
  DRAM 行缓冲，访存耗时主要由指令翻译决定；加上 4 KiB 页的连续性影响本就有限，这类收益要靠大页。

## 复现

```bash
git worktree add /tmp/base 7608a70
rsync -a --exclude target --exclude build --exclude .git user/ /tmp/base/user/
cp os/Cargo.lock /tmp/base/os/
cd /tmp/base && make -C os kernel TEST=4 BASE=2 CHAPTER=4   # detached HEAD 必须显式指定章节
```
