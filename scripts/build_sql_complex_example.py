#!/usr/bin/env python3
"""Build the SQLite database + datasource config for examples/sql-complex."""
import json
import sqlite3
from pathlib import Path

root = Path(__file__).resolve().parent.parent
source = root / "examples/sql-complex"
out = root / "output/sql-complex"
out.mkdir(parents=True, exist_ok=True)
path = out / "demo.sqlite"
temporary = out / "demo.pending.sqlite"
temporary.unlink(missing_ok=True)
with sqlite3.connect(temporary) as db:
    db.executescript((source / "data.sql").read_text())
temporary.replace(path)
config = {"demo-db": f"sqlite://{path}?mode=ro"}
(out / "datasources.json").write_text(json.dumps(config, ensure_ascii=False, indent=2) + "\n")
print(out / "datasources.json")
