# -*- coding: utf-8 -*-
"""从 `_probe_listrange_raw.txt` 提取 `ProbeListRange.java` 的关键行摘要。

为什么不直接 grep：raw 文件里**产品输出的错误消息是 GBK 字节**（产品的 Java
`MessageManager` 按平台编码写 stdout），而 12 条成功行的数值部分是 ASCII。
本脚本按 GBK 解码后统一写成 UTF-8 的 `_probe_listrange_summary.txt`，
供人读、供审计对照 —— 摘要文件本身不是判据（判据是产品夹具 expected.json）。

可复现：`python _mk_listrange_summary.py`（在本目录下运行）。
"""

import io

SRC = "_probe_listrange_raw.txt"
DST = "_probe_listrange_summary.txt"


def main() -> None:
    raw = open(SRC, "rb").read()
    out = []
    out.append("# `ProbeListRange.java` 原始输出的关键行摘要（被测公式写在 (2,3) 格）")
    out.append("#")
    out.append("# 证据链：全量 dump 见 `_probe_listrange_raw.txt`；本文件由")
    out.append("# `_mk_listrange_summary.py` 提取，**未手改**（raw 里的产品错误消息是 GBK 字节，")
    out.append("# 本文件已按 GBK 解码，其余为 ASCII）。")
    out.append("#")
    out.append("# === 12 项成功：公式 -> (2,3) 的计算值 ===")
    seen = set()
    for line in raw.split(b"\n"):
        if b"(2,3) value=" in line and b"[" in line:
            s = line.decode("gbk", errors="replace").strip()
            if s not in seen:
                seen.add(s)
                out.append(s)
    out.append("#")
    out.append("# === 2 项反序对照：报循环引用（List 落到 value 位置，cellSet 退化为单格）===")
    for line in raw.split(b"\n"):
        if b"FAIL" in line:
            out.append(line.decode("gbk", errors="replace").strip())
    with io.open(DST, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out) + "\n")
    print("OK: wrote %s (%d lines)" % (DST, len(out)))


if __name__ == "__main__":
    main()
