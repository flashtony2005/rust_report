# -*- coding: utf-8 -*-
"""复核 `source` 契约：source != null  <-等价于->  (r <= template_rows 且 c <= template_cols)？
输出混淆矩阵，以及 `cells` 数组的稀疏情况。
"""
import json
import os
import glob

D = os.path.dirname(os.path.abspath(__file__))


def main():
    for f in sorted(glob.glob(os.path.join(D, "engine_*.expected.json"))):
        name = os.path.basename(f).replace(".expected.json", "")
        with open(f, "r", encoding="utf-8") as fh:
            j = json.load(fh)
        calc = j.get("calculated") or {}
        cells = calc.get("cells") or []
        if not cells:
            print("%-18s (无网格)" % name)
            continue
        tr, tc = j.get("template_rows"), j.get("template_cols")
        rows, cols = calc.get("rows"), calc.get("cols")

        tp = fp = fn = tn = 0
        bad = []
        for c in cells:
            r, cc = c.get("r"), c.get("c")
            inside = (r <= tr and cc <= tc)
            has = c.get("source") is not None
            if inside and has:
                tp += 1
            elif not inside and not has:
                tn += 1
            elif inside and not has:
                fn += 1
                if len(bad) < 6:
                    bad.append(("在模板内但无 source", r, cc, c.get("type")))
            else:
                fp += 1
                if len(bad) < 6:
                    bad.append(("在模板外却有 source", r, cc, c.get("type")))

        emitted = len(cells)
        full = rows * cols
        print("%-18s tmpl=%sx%s calc=%sx%s emitted=%d/%d (缺 %d) | tp=%d tn=%d fn=%d fp=%d"
              % (name, tr, tc, rows, cols, emitted, full, full - emitted, tp, tn, fn, fp))
        for b in bad:
            print("      ✘", b)
        # 打印哪些坐标没被落盘
        have = {(c.get("r"), c.get("c")) for c in cells}
        missing = [(r, cc) for r in range(1, rows + 1) for cc in range(1, cols + 1) if (r, cc) not in have]
        if missing:
            print("      未落盘坐标: %s%s" % (missing[:14], " ..." if len(missing) > 14 else ""))


main()
