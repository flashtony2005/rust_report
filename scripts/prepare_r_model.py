#!/usr/bin/env python3
"""用样例 SQL 创建两个独立 SQLite 数据源；连接配置写入可重生成目录。"""
import json
import sqlite3
import time
from pathlib import Path


def main():
    root = Path(__file__).resolve().parent.parent
    source = root / "examples/r-model"
    target = root / "output/r-model"
    target.mkdir(parents=True, exist_ok=True)
    config = {}
    for name in ("operations", "organization"):
        path = target / f"{name}.sqlite"
        # 数据准备命令只重建专用示例库，不访问用户配置的数据库。
        temporary = target / f"{name}.pending.sqlite"
        temporary.unlink(missing_ok=True)
        with sqlite3.connect(temporary) as db:
            db.executescript((source / f"{name}.sql").read_text())
        # 沙箱里刚写出的文件偶发被监控进程占住（WinError 32），重试几次再失败。
        for attempt in range(5):
            try:
                temporary.replace(path)
                break
            except PermissionError:
                if attempt == 4:
                    raise
                time.sleep(0.4)
        # ⚠ Windows 上 `f"sqlite://{path}"` 产出 `sqlite://output\r-model\x.sqlite`
        # （相对路径 + 反斜杠），sqlx 直接报 invalid domain character / unable to open。
        # 正确形态 = 绝对路径 + POSIX 斜杠 + 三斜杠（`sqlite:///` 不带 authority）。
        config[f"{name}-db"] = f"sqlite:///{path.resolve().as_posix()}?mode=ro"
    path = target / "datasources.json"
    path.write_text(json.dumps(config, ensure_ascii=False, indent=2) + "\n")
    print(path)


if __name__ == "__main__":
    main()
