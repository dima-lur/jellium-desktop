const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const test = require('node:test');
const root = path.join(__dirname, '..');
function load(ctx, name) { vm.runInContext(fs.readFileSync(path.join(root, name), 'utf8'), ctx); }
function setup() {
    const ctx = { console: { debug() {}, log() {}, warn() {}, error() {} },
        navigator: { platform: 'Win32' }, document: { addEventListener() {} } };
    ctx.window = ctx;
    ctx.jmpNative = { playerLoad() {}, playerStop() {}, playerSetVolume() {} };
    vm.createContext(ctx);
    const source = fs.readFileSync(path.join(root, 'native-shim.js'), 'utf8')
        .replaceAll('__SETTINGS_JSON__', '{}').replaceAll('__WINDOW_DECORATIONS__', 'null')
        .replaceAll('__WINDOW_DECORATION_OPTIONS__', '[]').replaceAll('__DEVICE_PROFILE_JSON__', '{}');
    vm.runInContext(source, ctx);
    const events = new Map();
    ctx.Events = {
        on(owner, name, callback) {
            if (!events.has(owner)) events.set(owner, new Map());
            const map = events.get(owner);
            if (!map.has(name)) map.set(name, new Set());
            map.get(name).add(callback);
        },
        off(owner, name, callback) { events.get(owner)?.get(name)?.delete(callback); },
        trigger(owner, name, ...args) { for (const callback of events.get(owner)?.get(name) || []) callback({}, ...args); }
    };
    return ctx;
}

test('self-disconnecting signal listener does not skip its successor', () => {
    const ctx = setup();
    const signal = ctx.api.player.playing;
    const called = [];
    function first() { called.push('first'); signal.disconnect(first); }
    signal.connect(first);
    signal.connect(() => called.push('second'));
    signal();
    assert.deepEqual(called, ['first', 'second']);
});

test('cancel and replacement settle each load once and remove old callbacks', () => {
    const ctx = setup();
    const results = [];
    const start = id => ctx.api.player.load('test', { startMilliseconds: 0 }, {}, 0, 0, 0, '', '',
        result => results.push([id, !!result?.canceled]));
    start('first');
    start('second');
    ctx.api.player.stop();
    ctx._nativeEmit('canceled');
    ctx._nativeEmit('playing');
    assert.deepEqual(results, [['first', true], ['second', true]]);
    start('third');
    ctx._nativeEmit('playing');
    ctx._nativeEmit('error');
    assert.deepEqual(results.at(-1), ['third', false]);
    assert.equal(results.length, 3);
});

test('destroy removes only the plugins subscriptions, including native signals', () => {
    const ctx = setup();
    load(ctx, 'input-plugin.js');
    let commands = 0;
    let rates = 0;
    let unrelated = 0;
    const pm = { getCurrentPlayer: () => ({ applyPlaybackRate() { rates++; } }) };
    const plugin = new ctx._inputPlugin({ playbackManager: pm, inputManager: { handleCommand() { commands++; } } });
    plugin.updateQueueState = () => { throw Error('destroyed plugin still called'); };
    ctx.Events.on(pm, 'playlistitemadd', () => unrelated++);
    plugin.destroy();
    plugin.destroy();
    ctx._nativeHostInput(['play']);
    ctx._nativeSetRate(1.5);
    ctx.Events.trigger(pm, 'playlistitemadd');
    assert.equal(commands, 0);
    assert.equal(rates, 0);
    assert.equal(unrelated, 1);
});

test('native speed observation updates UI without sending a command back', () => {
    const ctx = setup();
    let commands = 0;
    let changed = 0;
    ctx.jmpNative.playerSetSpeed = () => commands++;
    load(ctx, 'mpv-player-base.js');
    load(ctx, 'input-plugin.js');
    const player = new ctx.MpvPlayerBase({ events: { trigger(_owner, name) { if (name === 'ratechange') changed++; } },
        appSettings: { get() { return 1; }, set() {} } });
    new ctx._inputPlugin({ playbackManager: { getCurrentPlayer() { return player; } }, inputManager: {} });
    ctx._nativeSetRate(1.5);
    assert.equal(player.getPlaybackRate(), 1.5);
    assert.equal(commands, 0);
    assert.equal(changed, 1);
    player.setPlaybackRate(2);
    assert.equal(commands, 1);
    assert.equal(player.getPlaybackRate(), 1.5);
});

test('empty queue and playback stop clear transport capabilities', () => {
    const ctx = setup();
    load(ctx, 'input-plugin.js');
    const states = [];
    ctx.jmpNative.notifyQueueChange = (...state) => states.push(state);
    const pm = { _playQueueManager: { getPlaylist: () => [], getCurrentPlaylistIndex: () => -1 } };
    const plugin = new ctx._inputPlugin({ playbackManager: pm, inputManager: {} });
    plugin.updateQueueState();
    ctx.Events.trigger(pm, 'playbackstop');
    assert.deepEqual(states, [[false, false], [false, false]]);
    plugin.destroy();
});

test('superseded artwork cannot clear a newer request and destroy aborts its reader', async () => {
    const ctx = setup();
    ctx.AbortController = AbortController;
    ctx.ApiClient = { serverAddress: () => 'https://example.test' };
    const requests = [];
    ctx.fetch = (_url, { signal }) => new Promise((resolve, reject) => requests.push({ signal, resolve, reject }));
    let reader;
    ctx.FileReader = class {
        constructor() { reader = this; }
        readAsDataURL() {}
        abort() { this.aborted = true; this.onloadend?.(); }
    };
    let sent = 0;
    ctx.jmpNative.notifyArtwork = () => sent++;
    load(ctx, 'input-plugin.js');
    const plugin = new ctx._inputPlugin({ playbackManager: {}, inputManager: {} });
    const item = id => ({ Id: id, ImageTags: { Primary: 'tag' } });
    plugin.fetchAlbumArt(item('a'));
    plugin.fetchAlbumArt(item('a'));
    assert.equal(requests.length, 1);
    assert.equal(requests[0].signal.aborted, false);
    plugin.fetchAlbumArt(item('b'));
    assert.equal(requests[0].signal.aborted, true);
    requests[0].reject(Object.assign(new Error('aborted'), { name: 'AbortError' }));
    await new Promise(setImmediate);
    assert.match(plugin.pendingArtworkUrl, /\/b\//);
    requests[1].resolve({ ok: true, blob: async () => ({}) });
    await new Promise(setImmediate);
    plugin.destroy();
    assert.equal(requests[1].signal.aborted, true);
    assert.equal(reader.aborted, true);
    assert.equal(reader.onloadend, null);
    assert.equal(plugin.artworkReader, null);
    assert.equal(plugin.pendingArtworkUrl, null);
    assert.equal(sent, 0);
});
