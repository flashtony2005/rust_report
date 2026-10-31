# Pi + DeepSeek 接入 Report Studio

两件事拼起来就是「pi + deepseek」接入：

1. **Pi 的模型换成 DeepSeek**（`~/.pi/agent/models.json`）—— Pi 的大脑是 DeepSeek；
2. **装 `rpx-report` skill**（`~/.pi/skills/rpx-report/`）—— Pi 知道怎么操作报表管线。

Studio 自己的理解层也直连 DeepSeek（`POST /api/llm` 配 key）—— 两条路都通，
不依赖 Pi 时 Studio 也能起草查询。

## 1. Pi 配 DeepSeek（一次性）

把 [models.deepseek.json](./models.deepseek.json) 合并进 `~/.pi/agent/models.json`
（已有内容就只把 `deepseek` 条目加进 `providers`）：

```bash
# 没有 models.json 就直接拷
cp models.deepseek.json ~/.pi/agent/models.json

# 有就手工把 providers.deepseek 这一段并进去
```

⚠ `apiKey` 里填 [platform.deepseek.com](https://platform.deepseek.com) 创建的 key
（只显示一次，创建时立刻保存）。不想写进文件就用环境变量：

```bash
export DEEPSEEK_API_KEY=sk-...        # bash
$env:DEEPSEEK_API_KEY = "sk-..."      # PowerShell
```

验证：

```bash
pi --list-models deepseek        # 应列出 deepseek-v4-flash / deepseek-v4-pro
pi --provider deepseek -p "用一句话介绍你自己"
```

模型选择（2026-10 当前）：

| 模型 | 用途 | 价格（/1M tokens） |
|---|---|---|
| `deepseek-v4-flash` | 通用/起草查询（默认，快而便宜） | $0.15 in / $0.60 out |
| `deepseek-v4-pro` | 复杂推理（多步逻辑、难口径） | $0.66 in / $1.98 out |

⚠ 旧名 `deepseek-chat` / `deepseek-reasoner` 已于 **2026-07-24 退役**，用了会报错。

## 2. 装 rpx-report skill（一次性）

```bash
# 直接拷进 Pi 的技能目录
cp -r rpx-report ~/.pi/skills/

# 或启动时按路径加载（不装进全局）
pi --skill F:/project/report/rpx/pi-skill/rpx-report "分析一下各区域销售额"
```

验证：在 Pi 里说「看看数据源有哪些」，它应当去 `curl http://127.0.0.1:8778/api/datasources`。

## 3. Studio 理解层配 DeepSeek（可选但推荐）

```bash
curl -s --noproxy '*' -X POST http://127.0.0.1:8778/api/llm \
  -H 'Content-Type: application/json' \
  -d '{"api_key":"sk-...","model":"deepseek-v4-flash"}'
```

或设环境变量 `DEEPSEEK_API_KEY` 后重启 Studio。key 只落 `studio/data/llm.json`
（gitignore），**不回传前端**。配好后 `/understand` 就能自动起草查询；不配也能用，
只是起草这一步要靠 Pi（或人）来写查询 JSON。

## 4. 端到端试一把

```bash
pi --provider deepseek "Report Studio 里 demo-sqlite 数据源，按区域统计销售额，跑一下结果"
```

预期：Pi 调 `/understand` 起草 → 拿草案调 `/execute` → 把结果念给你；
口径有歧义时它会先问你，而不是替你猜。
