// 金仓真数端到端演示：report schema 的 emp_info 表 → SQL 数据集模板 →
// /render（数据源注册表取数）→ Univer canvas 截图。
// 内容断言走服务端 POST /render（Univer 内容画在 canvas，DOM 无文本）。
const fs = require('fs');
const path = require('path');
const { chromium } = require('C:/Users/Administrator/.workbuddy/binaries/node/versions/22.22.2-6/node_modules/@playwright/cli/node_modules/playwright');

const FIX = 'F:/project/report/rpx/tests/fixtures';
const RPX = path.join(FIX, 'report_demo.rpx');

async function assertSnapshot() {
  const fd = new FormData();
  fd.append('report', new Blob([fs.readFileSync(RPX)]), 'report_demo.rpx');
  const res = await fetch('http://127.0.0.1:8090/render', { method: 'POST', body: fd });
  if (res.status !== 200) throw new Error('render HTTP ' + res.status + ': ' + (await res.text()).slice(0, 300));
  const txt = await res.text();
  for (const needle of ['员工信息查询', '编号', '部门', '姓名', '工资', '张伟', '孙丽', '人事部', '市场部', '12000', '7800']) {
    if (!txt.includes(needle)) throw new Error('快照缺内容: ' + needle);
  }
  const snap = JSON.parse(txt);
  const sid = snap.sheetOrder[0];
  const rows = Object.keys(snap.sheets[sid].cellData).length;
  const merges = snap.sheets[sid].mergeData.length;
  if (merges < 1) throw new Error('标题跨列合并应进快照 mergeData');
  console.log('SNAPSHOT_OK rows=' + rows + ' merges=' + merges + ' (金仓真数)');
}

(async () => {
  await assertSnapshot();
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  await page.goto('http://127.0.0.1:5173', { waitUntil: 'networkidle' });
  await page.setInputFiles('input[type=file]', [RPX]);
  await page.getByText('渲染', { exact: true }).click();
  await page.waitForSelector('canvas', { timeout: 15000 });
  await page.waitForTimeout(2500); // Univer 首帧 + 样式稳定
  const canvases = await page.locator('canvas').count();
  await page.screenshot({ path: path.join(FIX, 'out', 'm4-kingbase-demo.png') });
  console.log('KINGBASE_DEMO_OK canvas=' + canvases);
  await browser.close();
})().catch((e) => {
  console.error('KINGBASE_DEMO_FAIL', e.message);
  process.exit(1);
});
