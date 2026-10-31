'use strict';
const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {render, seriesFor, readRecords} = require('./report.cjs');
const metadata = {type: 'metadata', schema: 1, intervalSeconds: 5};
const processMetric = {pid: 7, created: 1000, role: 'renderer', privateBytes: 1024 ** 2, workingSetBytes: 2 * 1024 ** 2, handles: 4, threads: 2, cpuSeconds: 1};
const sample = (elapsedSeconds, processes = [processMetric]) => ({type: 'sample', time: 1000 + elapsedSeconds * 1000, elapsedSeconds, collectionSeconds: 0.2, processErrors: 0, processes, gpuMemory: null, gpuEngines: null});

test('PID reuse and process disappearance never connect unrelated lifetimes', () => {
  const samples = [sample(0), sample(5, []), sample(10, [{...processMetric, created: 9000}])];
  const values = [...seriesFor(samples, 'privateBytes').values()];
  assert.deepEqual(values, [[1024 ** 2, null, null], [null, null, 1024 ** 2]]);
  const html = render([metadata, ...samples]);
  assert.match(html, /GPU counters were unavailable/);
  assert.match(html, /No counter data available/);
});

test('GPU engines and adapters remain distinct, values are not summed or clamped', () => {
  const s = sample(0);
  s.gpuEngines = [0, 1].map(engine => ({pid: 7, adapter: 'luid_0x0_0xabc_phys_0', engine: String(engine), kind: '3D', percent: 80}));
  const series = seriesFor([s], 'percent');
  assert.equal(series.size, 2);
  assert.deepEqual([...series.values()], [[80], [80]]);
});

test('report exports only fixed labels and numbers, escapes warnings, uses no network resources', () => {
  const s = sample(0);
  s.processes[0] = {...processMetric, commandLine: 'SECRET_URL'};
  const html = render([metadata, s], [
    {type: 'action', time: 1000, action: '</script><script>alert(1)</script>'},
    {type: 'action', time: 1000, action: 'pause', title: 'SECRET_TITLE', dropped: 3},
  ], ['<img src=x onerror=alert(1)>']);
  assert.doesNotMatch(html, /SECRET_|<img|alert\(1\)<\/script>|https?:\/\//);
  assert.match(html, /&lt;img/);
  assert.match(html, /dropped action records/);
  assert.match(html, /<td>pause<\/td>/);
});

test('invalid numeric data and unsupported schemas fail rather than creating misleading charts', () => {
  assert.throws(() => render([{...metadata, schema: 2}, sample(0)]), /schema/);
  assert.throws(() => render([metadata, {...sample(0), processes: [{...processMetric, privateBytes: -1}]}]), /metrics/);
  assert.throws(() => render([metadata, sample(5), sample(0)]), /order/);
  assert.throws(() => render([metadata]), /no samples/);
  assert.match(render([metadata, sample(0), {type: 'capture-error'}, {type: 'capture-stop'}]), /Collector failed/);
});

test('interrupted tail can be recovered; malformed complete records fail visibly', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'jellium-report-'));
  try {
    const file = path.join(dir, 'samples.jsonl');
    fs.writeFileSync(file, '{"type":"sample"}\n{"type":');
    const warnings = [];
    assert.equal(readRecords(file, warnings).length, 1);
    assert.equal(warnings.length, 1);
    fs.writeFileSync(file, '{bad}\n');
    assert.throws(() => readRecords(file, []), /line 1/);
  } finally { fs.rmSync(dir, {recursive: true}); }
});
