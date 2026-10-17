#!/usr/bin/env bash
# 真 CLI + 真 HTTP socket：共享源码、修订冲突、固定结果；临时库自动清理。
set -eu
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo build --locked -p rpx-cli -p rpx-server
python3 - <<'PY'
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--no-deps', '--format-version', '1']))
bin_dir = Path(metadata['target_directory']) / 'debug'
suffix = '.exe' if __import__('os').name == 'nt' else ''

def cli(*args):
    value = json.loads(subprocess.check_output([str(bin_dir / ('rpx' + suffix)), *map(str, args)]))
    assert value['ok'], value
    return value['data']

with tempfile.TemporaryDirectory(prefix='rpx-e2e-') as temp:
    library = Path(temp) / 'reports'
    cli('library', 'init', library, '--presets')
    detail = cli('report', 'show', 'complex-master', '--library', library)
    run = cli('report', 'run', 'complex-master', '--library', library,
              '--if-match', detail['revision'], '--export', 'csv')
    assert run['run']['shape']['rows'] == 23
    assert run['run']['shape']['cols'] == 25
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    addr = f'127.0.0.1:{port}'
    process = subprocess.Popen([str(bin_dir / ('rpx-server' + suffix)), 'serve', '--library', str(library), '--addr', addr], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    def http(path, method='GET', body=None, revision=None):
        headers = {'Content-Type': 'application/json'}
        if revision is not None:
            headers['If-Match'] = revision
        request = urllib.request.Request(f'http://{addr}{path}', method=method, headers=headers,
                                        data=None if body is None else json.dumps(body).encode())
        try:
            with urllib.request.urlopen(request, timeout=5) as response:
                return response.status, response.read()
        except urllib.error.HTTPError as error:
            return error.code, error.read()
    try:
        for attempt in range(50):
            try:
                if http('/health') == (200, b'ok'):
                    break
            except OSError:
                time.sleep(.1)
        else:
            raise AssertionError('server did not start')
        detail['template']['cells'][0]['text'] = '人工与 agent 共同编辑'
        path = '/reports/complex-master/template'
        assert http(path, 'PUT', detail['template'])[0] == 428
        assert http(path, 'PUT', detail['template'], detail['revision'])[0] == 200
        assert http(path, 'PUT', detail['template'], detail['revision'])[0] == 412
        state = json.loads(http('/reports/complex-master')[1])
        assert state['result_state'] == 'older_revision'
        # Results remain available when actual business data is absent.
        (library / 'complex-master/data/complex-master-sales.csv').unlink()
        run_id = run['run']['run_id']
        status, workbook = http(f'/reports/complex-master/runs/{run_id}/artifacts/workbook.json')
        assert status == 200 and workbook == Path(run['workbook']).read_bytes()
        source = json.loads(http(f'/reports/complex-master/runs/{run_id}/source/template.json')[1])
        assert source['cells'][0]['text'] != detail['template']['cells'][0]['text']
        assert http('/render', 'POST', {})[0] == 404
        assert cli('report', 'runs', 'complex-master', '--library', library)['total'] == 1
        print('E2E PASS: CLI run 23x25, source save, stale revision, fixed workbook and source snapshot')
    finally:
        process.terminate()
        process.wait(timeout=5)
PY
