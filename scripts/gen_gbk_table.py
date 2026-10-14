#!/usr/bin/env python3
"""M3-02：把 ProbeEncoding.java 的 JDK GBK dump 冻结成 gbk.bin（零运行时依赖）。

## 数据链（为什么这个脚本不敢发明映射）

    产品行为（FileObject(fileName, "GBK", "s", ctx)）
      → JDK GBK charset
      → ProbeEncoding.java 全定义域 dump（tests/fixtures/gbk_jdk.bin）
      → 本脚本：校验 + 加头 + 排序（**不改任何一对映射**）
      → crates/rpx-engine/data/gbk.bin（include_bytes!）

## 校验（任何一条不过即拒绝写出，宁可没有表也不能有错表）

1. dump 是 4 字节对的整数倍；
2. gbk 码位严格升序、互不重复（探针的双重循环天然有序，防呆）；
3. gbk 码位 ∈ [0x8140, 0xFEFE]，lead ∈ [0x81,0xFE]，trail ∈ [0x40,0xFE] 且 != 0x7F；
4. unicode != U+FFFD（探针已排除，双保险）；
5. unicode != U+D800..U+DFFF（代理区不可能是合法映射目标）。

## 输出格式

    8  bytes  magic b"RPXGBK\\x01\\x00"
    4  bytes  u32 BE pair count
    N x 4     (u16 BE gbk, u16 BE unicode)，按 gbk 升序

用法：python3 scripts/gen_gbk_table.py [dump 路径] [输出路径]
"""
from __future__ import annotations

import struct
import sys
from pathlib import Path

MAGIC = b"RPXGBK\x01\x00"


def fail(msg: str) -> None:
    raise SystemExit(f"gen_gbk_table: {msg}")


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    dump_path = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "tests/fixtures/gbk_jdk.bin"
    out_path = (
        Path(sys.argv[2])
        if len(sys.argv) > 2
        else root / "crates/rpx-engine/data/gbk.bin"
    )
    data = dump_path.read_bytes()
    if len(data) % 4 != 0:
        fail(f"dump 长度 {len(data)} 不是 4 的倍数（不是 (u16,u16) 对序列）")
    pairs = list(struct.iter_unpack(">HH", data))
    if not pairs:
        fail("dump 为空：探针没跑或没写对")

    prev = -1
    for gbk, uni in pairs:
        if gbk <= prev:
            fail(f"gbk 码位非严格升序/重复：{gbk:#06x}（前一个 {prev:#06x}）")
        prev = gbk
        if not (0x8140 <= gbk <= 0xFEFE):
            fail(f"gbk 码位越界：{gbk:#06x}")
        lead, trail = gbk >> 8, gbk & 0xFF
        if not (0x81 <= lead <= 0xFE):
            fail(f"lead 字节越界：{lead:#04x}")
        if not (0x40 <= trail <= 0xFE) or trail == 0x7F:
            fail(f"trail 字节非法：{trail:#04x}")
        if uni == 0xFFFD:
            fail(f"{gbk:#06x} 映射到 U+FFFD（探针应已排除）")
        if 0xD800 <= uni <= 0xDFFF:
            fail(f"{gbk:#06x} 映射到代理区 U+{uni:04X}")

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_bytes(MAGIC + struct.pack(">I", len(pairs)) + data)
    print(f"gen_gbk_table: {len(pairs)} 对 -> {out_path} ({out_path.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
