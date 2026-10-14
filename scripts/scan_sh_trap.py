#!/usr/bin/env python3
"""扫描 shell 脚本里的「$var 后紧跟全角标点」陷阱（gate.sh 的 G6）。

坑的成因
--------
UTF-8 locale 下 bash 会把紧跟未加花括号的 ``$var`` 的非 ASCII 字符**吃进变量名**：

    set -u; v=HELLO
    echo "$v：文字"     # 变量名变成「v：文字」→ unbound → 脚本被杀，报错还是乱码

正确写法是 ``${v}：文字``。这类错误不会让脚本"结果不对"，而是让脚本**直接死**且
死因难以定位，所以必须由机器扫，不能靠人眼。

退出码：0 = 无命中；1 = 有命中（同时打印 file:line 与变长后的变量名）。
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# $var 后面紧跟「非 ASCII」即为违规（bash 会把它并进变量名）。
PATTERN = re.compile(r"\$([A-Za-z_][A-Za-z0-9_]*)(?=[^\x00-\x7F])")


def main(argv: list[str]) -> int:
    roots = argv[1:] or ["scripts"]
    hits: list[tuple[Path, int, str, str]] = []

    for root in roots:
        base = Path(root)
        files = sorted(base.rglob("*.sh")) if base.is_dir() else [base]
        for path in files:
            text = path.read_text(encoding="utf-8", errors="replace")
            for lineno, line in enumerate(text.splitlines(), 1):
                for m in PATTERN.finditer(line):
                    # 变量名被延长成的样子：从 $ 起吃到第一个 ASCII 字符
                    tail = re.match(r"\$[A-Za-z_][A-Za-z0-9_]*[^\x00-\x7f]+", line[m.start():])
                    extended = tail.group(0) if tail else m.group(0)
                    hits.append((path, lineno, m.group(0), extended))

    if not hits:
        print(f"G6: {len(roots)} 个路径下的 .sh 无「$var + 全角标点」陷阱")
        return 0

    print(f"G6: 发现 {len(hits)} 处「$var 后紧跟全角标点」—— bash 会把标点吃进变量名", file=sys.stderr)
    for path, lineno, raw, extended in hits:
        print(f"  {path}:{lineno}: {raw!r} -> 变量名变成 {extended!r}，应改成 ${{{raw[1:]}}}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
