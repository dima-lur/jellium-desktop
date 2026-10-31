'use strict';
const fs = require('node:fs');
const path = require('node:path');
const actions = new Set(['app-start', 'app-stop', 'load-start', 'file-loaded', 'file-end', 'pause', 'resume', 'seek', 'playback-restart', 'settings', 'about', 'baseline', 'play', 'switch-film', 'fullscreen', 'resize', 'idle', 'stop']);
const escape = value => String(value).replace(/[&<>"']/g, c => ({'&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;'}[c]));
const number = value => typeof value === 'number' && Number.isFinite(value) && value >= 0;

function readRecords(file, warnings) {
  if (!fs.existsSync(file)) return [];
  const text = fs.readFileSync(file, 'utf8').replace(/^\uFEFF/, '');
  const lines = text.split('\n');
  return lines.flatMap((line, index) => {
    if (!line.trim()) return [];
    try { return [JSON.parse(line)]; }
    catch (error) {
      if (index === lines.length - 1 && !text.endsWith('\n')) {
        warnings.push(`${path.basename(file)}: incomplete final record omitted (capture may still be running).`);
        return [];
      }
      throw new Error(`${path.basename(file)}: invalid JSON at line ${index + 1}`, {cause: error});
    }
  });
}

function validateSample(s) {
  if (!number(s.time) || !number(s.elapsedSeconds) || !number(s.collectionSeconds) || !number(s.processErrors) || !Array.isArray(s.processes)) throw new Error('Invalid sample');
  for (const p of s.processes) {
    if (!['browser', 'renderer', 'gpu', 'utility'].includes(p.role) || !['pid', 'created', 'privateBytes', 'workingSetBytes', 'handles', 'threads', 'cpuSeconds'].every(k => number(p[k]))) throw new Error('Invalid process metrics');
  }
  for (const [key, fields] of [['gpuMemory', ['dedicatedBytes', 'sharedBytes']], ['gpuEngines', ['percent']]]) {
    if (s[key] === null) continue;
    if (!Array.isArray(s[key])) throw new Error(`Invalid ${key}`);
    for (const g of s[key]) {
      if (!number(g.pid) || !/^luid_[\da-fx]+_[\da-fx]+_phys_\d+$/i.test(g.adapter) || !fields.every(k => number(g[k]))) throw new Error(`Invalid ${key} metrics`);
      if (key === 'gpuEngines' && (!/^\d+$/.test(g.engine) || !/^[a-zA-Z0-9_]+$/.test(g.kind))) throw new Error('Invalid engine identity');
    }
  }
}

function seriesFor(samples, field) {
  const series = new Map();
  samples.forEach((s, i) => {
    const add = (key, value) => {
      if (!series.has(key)) series.set(key, Array(samples.length).fill(null));
      series.get(key)[i] = value;
    };
    if (['privateBytes', 'workingSetBytes', 'handles', 'threads'].includes(field)) {
      for (const p of s.processes) add(`${p.role} PID ${p.pid} / ${p.created}`, p[field]);
    } else {
      for (const g of (s[field === 'percent' ? 'gpuEngines' : 'gpuMemory'] || [])) {
        const p = s.processes.find(p => p.pid === g.pid);
        if (p) add(`${p.role} PID ${p.pid} / ${p.created} · ${g.adapter}${field === 'percent' ? ` · ${g.kind} #${g.engine}` : ''}`, g[field]);
      }
    }
  });
  return series;
}

function chart(samples, events, field, title, unit, divisor) {
  const series = seriesFor(samples, field);
  const values = [...series.values()].flat().filter(number);
  if (!values.length) return `<section><h2>${title}</h2><p>No counter data available.</p></section>`;
  const max = values.reduce((max, v) => Math.max(max, v / divisor), Number.EPSILON);
  const start = samples[0].elapsedSeconds;
  const span = samples.at(-1).elapsedSeconds - start || 1;
  const x = i => 70 + (samples[i].elapsedSeconds - start) / span * 1000;
  const y = v => 250 - (v / divisor) / max * 210;
  const lines = [...series].map(([key, points], index) => {
    const color = `hsl(${index * 137.508 % 360} 65% 42%)`;
    let penDown = false;
    const d = points.map((value, i) => {
      if (value === null) { penDown = false; return ''; }
      const part = `${penDown ? 'L' : 'M'}${x(i).toFixed(2)},${y(value).toFixed(2)}`;
      penDown = true;
      return part;
    }).join(' ');
    const dots = points.map((value, i) => value === null ? '' : `<circle cx="${x(i)}" cy="${y(value)}" r="2"><title>${escape(key)}: ${(value / divisor).toFixed(2)} ${unit}, ${(samples[i].elapsedSeconds / 60).toFixed(2)} min</title></circle>`).join('');
    return `<g class="series" data-series="${index}" stroke="${color}" fill="${color}"><path d="${d}" fill="none" stroke-width="1.7"/>${dots}</g>`;
  }).join('');
  const markers = events.filter(e => e.time >= samples[0].time && e.time <= samples.at(-1).time).map(e => {
    // Wall-clock events are placed between adjacent sample timestamps; metrics use monotonic elapsed time.
    let i = samples.findIndex(s => s.time >= e.time);
    let pos = x(i);
    if (i > 0 && samples[i].time > samples[i - 1].time) pos = x(i - 1) + (x(i) - x(i - 1)) * (e.time - samples[i - 1].time) / (samples[i].time - samples[i - 1].time);
    return `<line x1="${pos}" x2="${pos}" y1="30" y2="250" stroke="#b65b00" stroke-dasharray="3 5"><title>${escape(e.action)} · ${new Date(e.time).toISOString()}</title></line>`;
  }).join('');
  const legend = [...series.keys()].map((key, index) => `<label style="border-left:4px solid hsl(${index * 137.508 % 360} 65% 42%)"><input type="checkbox" checked data-series="${index}">${escape(key)}</label>`).join('');
  return `<section><h2>${title}</h2><svg viewBox="0 0 1120 290" role="img" aria-label="${title}"><path d="M70 30V250H1070" fill="none" stroke="#778"/><text x="4" y="38">${max.toFixed(1)}</text><text x="10" y="250">0</text><text x="10" y="275">${unit}</text><text x="70" y="275">${(start / 60).toFixed(2)} min</text><text x="970" y="275">${(samples.at(-1).elapsedSeconds / 60).toFixed(2)} min</text>${markers}${lines}</svg><details><summary>Processes / engines (${series.size})</summary><div class="legend">${legend}</div></details></section>`;
}

function render(records, actionRecords = [], warnings = []) {
  const metadata = records.find(r => r.type === 'metadata');
  if (metadata?.schema !== 1 || !number(metadata.intervalSeconds) || metadata.intervalSeconds === 0) throw new Error('Unsupported capture schema');
  const samples = records.filter(r => r.type === 'sample');
  if (!samples.length) throw new Error('Capture contains no samples');
  samples.forEach(validateSample);
  for (let i = 1; i < samples.length; i++) {
    if (samples[i].elapsedSeconds < samples[i - 1].elapsedSeconds) throw new Error('Samples are out of order');
    if (samples[i].time < samples[i - 1].time) warnings.push('System clock moved backwards: action placement may be inaccurate.');
  }
  const events = actionRecords.filter(e => e.type === 'action' && number(e.time) && actions.has(e.action)).sort((a, b) => a.time - b.time);
  if (!records.some(r => r.type === 'capture-stop')) warnings.push('Capture has no clean stop record.');
  if (records.some(r => r.type === 'capture-error')) warnings.push('Collector failed: this capture is incomplete.');
  if (!samples.some(s => s.processes.length)) warnings.push('No matching process metrics were captured.');
  if (!events.some(e => e.action === 'app-start')) warnings.push('Automatic app timeline is absent; use -Launch with the instrumented build.');
  if (events.some(e => e.action === 'app-start') && !events.some(e => e.action === 'app-stop')) warnings.push('App timeline is incomplete (app still running or exited abruptly).');
  if (events.some(e => number(e.dropped) && e.dropped > 0)) warnings.push('The app reported dropped action records.');
  if (samples.some(s => s.processErrors)) warnings.push('Some processes exited or could not be read during collection.');
  if (samples.some(s => s.gpuMemory === null || s.gpuEngines === null)) warnings.push('GPU counters were unavailable in part or all of this capture; gaps are not zero usage.');
  if (samples.some(s => s.collectionSeconds > metadata.intervalSeconds)) warnings.push('Collection exceeded the requested interval; inspect sampling cost.');
  const specs = [
    ['privateBytes', 'Private process memory', 'MiB', 1024 ** 2],
    ['workingSetBytes', 'Working set (includes shared pages)', 'MiB', 1024 ** 2],
    ['dedicatedBytes', 'GPU dedicated memory per process / adapter', 'MiB', 1024 ** 2],
    ['sharedBytes', 'GPU shared memory per process / adapter', 'MiB', 1024 ** 2],
    ['percent', 'GPU engine utilization (independent engines)', '%', 1],
    ['handles', 'Process handles', 'count', 1],
    ['threads', 'Process threads', 'count', 1],
  ];
  const rows = events.map(e => `<tr><td>${new Date(e.time).toISOString()}</td><td>${escape(e.action)}</td><td>${e.source === 'manual' ? 'manual' : 'mpv / shell'}</td></tr>`).join('');
  return `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Jellium diagnostics</title><style>body{font:16px system-ui;background:#f4f6fa;color:#20283c;max-width:1280px;margin:auto;padding:24px}h1{margin-bottom:6px}section{background:white;padding:20px;margin:18px 0;border-radius:12px}svg{width:100%;height:auto}svg text{font-size:13px;fill:#435}h2{font-size:19px}label{display:block;font:12px monospace;padding:4px}.legend{max-height:240px;overflow:auto}table{border-collapse:collapse;width:100%}td,th{text-align:left;padding:7px;border-bottom:1px solid #ddd}.warning{color:#8c4900}summary{cursor:pointer}</style><h1>Jellium diagnostics</h1><p>${samples.length} samples · requested interval ${metadata.intervalSeconds} s · ${(samples.at(-1).elapsedSeconds / 60).toFixed(2)} min · ${events.length} action markers</p><p>Offline report. Hover points and orange action lines for details; expand legends to hide individual series. Metrics use a monotonic timeline; action timestamps are UTC.</p><p>Private bytes are committed process memory, not JS heap size. Working sets share pages and must not be summed as unique RAM. GPU engines are separate and must not be summed as overall GPU load. Missing processes/counters create gaps. A rise alone does not prove a leak: compare repeated workloads after warm-up and release.</p>${[...new Set(warnings)].map(w => `<p class="warning">${escape(w)}</p>`).join('')}${specs.map(spec => chart(samples, events, ...spec)).join('')}<section><h2>Action timeline</h2><table><thead><tr><th>UTC</th><th>Action</th><th>Source</th></tr></thead><tbody>${rows}</tbody></table></section><script>document.querySelectorAll('input[data-series]').forEach(input=>input.addEventListener('change',()=>{input.closest('section').querySelector('g[data-series="'+input.dataset.series+'"]').style.display=input.checked?'':'none'}));</script></html>`;
}

if (require.main === module) {
  try {
    if (process.argv.length !== 3) throw new Error('Usage: node dev/diagnostics/report.cjs CAPTURE_DIRECTORY');
    const directory = path.resolve(process.argv[2]);
    const warnings = [];
    const records = readRecords(path.join(directory, 'samples.jsonl'), warnings);
    const events = ['actions.jsonl', 'manual.jsonl'].flatMap(name => readRecords(path.join(directory, name), warnings));
    fs.writeFileSync(path.join(directory, 'report.html'), render(records, events, warnings));
    console.log(path.join(directory, 'report.html'));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
module.exports = {render, seriesFor, readRecords, validateSample};
