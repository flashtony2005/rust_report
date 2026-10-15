// engine_emp 端到端演示：网页选文件 → 渲染 → Univer canvas 截图；
// 内容断言走服务端 POST /render（Univer 内容画在 canvas，DOM 无文本可断言）。
const fs = require('fs');
const path = require('path');
const { chromium } = require('C:/Users/Administrator/.workbuddy/binaries/node/versions/22.22.2-6/node_modules/@playwright/cli/node_modules/playwright');

const FIX = 'F:/project/report/rpx/tests/fixtures';
const RPX = path.join(FIX, 'engine_emp.rpx');
const CSV = path.join(FIX, 'engine_emp.csv');

async function assertSnapshot() {
  const fd = new FormData();
  fd.append('report', new Blob([fs.readFileSync(RPX)]), 'engine_emp.rpx');
  fd.append('dataset', new Blob([fs.readFileSync(CSV)]), 'engine_emp.csv');
  const res = await fetch('http://127.0.0.1:8090/render', { method: 'POST', body: fd });
  if (res.status !== 200) throw new Error('render HTTP ' + res.status + ': ' + (await res.text()).slice(0, 200));
  const txt = await res.text();
  for (const needle of ['员工信息查询', '编号', '部门', '张伟', '孙丽', '技术部', '12000', '7800']) {
    if (!txt.includes(needle)) throw new Error('快照缺内容: ' + needle);
  }
  const snap = JSON.parse(txt);
  const sid = snap.sheetOrder[0];
  const rows = Object.keys(snap.sheets[sid].cellData).length;
  console.log('SNAPSHOT_OK rows=' + rows + ' merges=' + snap.sheets[sid].mergeData.length);
}

(async () => {
  await assertSnapshot();
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  await page.goto('http://127.0.0.1:5173', { waitUntil: 'networkidle' });
  await page.setInputFiles('input[type=file]', [RPX, CSV]);
  await page.getByText('渲染', { exact: true }).click();
  await page.waitForSelector('canvas', { timeout: 15000 });
  await page.waitForTimeout(2500); // Univer 首帧 + 样式稳定
  const canvases = await page.locator('canvas').count();
  await page.screenshot({ path: path.join(FIX, 'out', 'm4-emp-demo.png') });
  console.log('EMP_DEMO_OK canvas=' + canvases);
  await browser.close();
})().catch((e) => {
  console.error('EMP_DEMO_FAIL', e.message);
  process.exit(1);
});
