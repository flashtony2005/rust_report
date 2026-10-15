#!/usr/bin/env python3
"""G7：文档引用完整性 —— 指向 `*.md §N.M` 的引用，其章节必须真实存在。

## 判据

凡形如 `<path>.md` 紧跟 `§N[.M[.K]]`（路径可被反引号包裹、中间可夹 `**` 与空白）
的引用，其编号必须在目标文档里有一个同名标题（`#`..`####` 后跟该编号；编号可
直接开头如 `## 21. 标题`，也可带 `§` 前缀如 `## §40 标题`）。

## 为什么需要它（2026-10-13 立此闸的真实触发点）

一天之内查出三类**同款静默失败**：

1. **8 处**代码头注释指向 `docs/m2-engine-design.md` **§5.4**，而 §5.4 **从未写出**
   （章节只到 §5.3）。引用点：`calc/ctx.rs`、`calc/run.rs`、`calc/cell_eval.rs`、
   `expr/eval.rs`、`expr/dsfn.rs`、`calc/cell_eval/extend.rs`、`stage_a.rs`、
   `stage_b/merge_alias.rs`。
2. `merge.rs` 指向 `docs/java-reference-map.md` **§6.6**（不存在）。
3. `expr/names.rs` 指向 `docs/code-review.md` **§5.9**（当时不存在），另有 3 处指向 §5.12。

这类错误的性质：**编译过、测试绿、只有人真的去翻那一节时才发现**。它正是
`docs/completeness-audit.md` §3 问题 2 说的「文档与代码双向漂移」的第 4 种形态
（代码引用文档、文档没有）—— 而根因是「G1–G6 里没有任何一条检查文档」。

## 判据的有效性（故障注入）

本脚本的判据已用故障注入证明：把任意一处引用的编号改成不存在的值（如 `§9.9`）⇒
必须红；改回 ⇒ 必须绿。**一个固定报 0 条噪声的检查等于没有检查。**

## 用法

    python3 scripts/check_doc_refs.py [根目录]     # 默认从本脚本位置推仓库根

退出码：0 = 全部解析成功；1 = 存在悬空/指向不存在文档的引用。
"""

import re
import sys
from pathlib import Path

# 扫描范围：这些后缀的文件里可能出现对 docs 的引用
SCAN_SUFFIXES = {".rs", ".sh", ".py", ".md"}
# 跳过这些目录（快照/缓存/构建产物/agent 记忆，均不属于交付物）
SKIP_DIRS = {".git", "target", ".workbuddy-ai", ".snapshots", "node_modules"}

# 引用形态：<path>.md [反引号] [空白] [**] [空白] § [空白] N[.M[.K]]
# ⛔ 路径与 § 之间**只允许**空白/星号/反引号 —— 这样 "format-contract.md 后合入）：§5.2"
# 这类跨标点的巧合不会被误判（假阳性会让闸失去公信力，等于没有闸）。
REF = re.compile(
    r"([A-Za-z0-9_./\-]*[A-Za-z0-9_\-]+\.md)`?[ \t]*\*{0,2}[ \t]*§[ \t]*([0-9]+(?:\.[0-9]+)*)"
)
# 标题形态：编号可直接跟在 `#` 后（`## 21. 标题`），也可带 `§` 前缀
# （`## §40 标题` / `### §40.1 标题` —— RUNLOG 自 §33 起的风格）。
# 2026-10-15 修：原先只认前者 ⇒ 对 RUNLOG §40+ 的合法引用被判「悬空」
# （第一次触发：spec-m5 引用 RUNLOG §42 被误红）；`§` 前缀同样计入章节号。
# 故障注入性质不变：引用不存在的编号（如 §99.9）依旧必红。
HEADING = re.compile(r"^#{1,4}\s+(?:§\s*)?([0-9]+(?:\.[0-9]+)*)", re.M)


def collect_docs(root: Path) -> dict:
    """basename -> 该文档里存在的全部章节号。同名文档取并集。"""
    docs: dict[str, set] = {}
    for p in root.rglob("*.md"):
        if any(part in SKIP_DIRS for part in p.parts):
            continue
        secs = set(HEADING.findall(p.read_text(encoding="utf-8", errors="replace")))
        docs.setdefault(p.name, set()).update(secs)
    return docs


def iter_files(root: Path):
    for p in root.rglob("*"):
        if not p.is_file() or p.suffix not in SCAN_SUFFIXES:
            continue
        if any(part in SKIP_DIRS for part in p.parts):
            continue
        yield p


def main() -> int:
    root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent
    docs = collect_docs(root)

    bad = []
    checked = 0
    for f in iter_files(root):
        for lineno, line in enumerate(f.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            for m in REF.finditer(line):
                name, sec = m.group(1), m.group(2)
                base = name.rsplit("/", 1)[-1]
                checked += 1
                if base not in docs:
                    bad.append(f"  {f.relative_to(root)}:{lineno}  指向不存在的文档 `{name}`（§{sec}）")
                elif sec not in docs[base]:
                    bad.append(f"  {f.relative_to(root)}:{lineno}  悬空：`{name}` 无 §{sec}")

    if bad:
        print("G7: 文档引用完整性 FAIL")
        print("\n".join(bad))
        print(f"\n共 {len(bad)} 处问题（扫描了 {checked} 个 `*.md §N.M` 引用）。")
        return 1
    print(f"G7: {checked} 个 `*.md §N.M` 引用全部解析成功（索引了 {len(docs)} 份 .md）。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
