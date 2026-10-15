# 解析乾学院讲座 HTML → 纯文本正文（去脚本/样式/导航/评论区）。
import html as h
import re
import sys
from pathlib import Path

DIR = Path(sys.argv[1])
OUT = Path(sys.argv[2])

TITLE_SKIP = re.compile(r'(首页|问答|知道|报表＆BI|计算＆AI|量化交易|^图书$|数据蒋堂|下载|官网|^文档$)')

def extract(p: Path) -> str:
    raw = p.read_text(encoding='utf-8', errors='ignore')
    raw = re.sub(r'<script\b.*?</script>', ' ', raw, flags=re.S | re.I)
    raw = re.sub(r'<style\b.*?</style>', ' ', raw, flags=re.S | re.I)
    raw = re.sub(r'<!--.*?-->', ' ', raw, flags=re.S)
    # 正文容器：乾学院文章在 markdown-body / article 里，退化为全页
    m = re.search(r'<div[^>]*(?:markdown-body|article-content|content)[^>]*>(.*?)</div>\s*(?:<div class="comment|<footer)',
                  raw, flags=re.S | re.I)
    body = m.group(1) if m else raw
    body = re.sub(r'<(script|style)\b.*?</\1>', ' ', body, flags=re.S | re.I)
    text = re.sub(r'<[^>]+>', '\n', body)
    text = h.unescape(text)
    lines = [ln.strip() for ln in text.splitlines()]
    lines = [ln for ln in lines if ln and not TITLE_SKIP.search(ln)]
    return '\n'.join(lines)

parts = []
for p in sorted(DIR.glob('*.html')):
    parts.append(f'\n\n===== {p.stem} =====\n')
    parts.append(extract(p))
OUT.write_text(''.join(parts), encoding='utf-8')
print('written', OUT, len(''.join(parts)), 'chars')
