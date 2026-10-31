(function() {
    class inputPlugin {
        constructor({ playbackManager, inputManager }) {
            this.name = 'Input Plugin';
            this.type = 'input';
            this.id = 'inputPlugin';
            this.playbackManager = playbackManager;
            this.inputManager = inputManager;
            this.subscriptions = [];
            this.playerSubscriptions = [];
            this.artworkAbortController = null;
            this.artworkReader = null;
            this.pendingArtworkUrl = null;
            this.attachedPlayer = null;

            console.debug('[Media] inputPlugin constructed with playbackManager:', !!playbackManager);

            if (playbackManager && window.Events) {
                this.setupEvents(playbackManager);
            }
        }

        notifyMetadata(item) {
            if (!item || !window.jmpNative) return;
            const meta = {
                Name: item.Name || '',
                Type: item.Type || '',
                MediaType: item.MediaType || '',
                SeriesName: item.SeriesName || '',
                SeasonName: item.SeasonName || '',
                Album: item.Album || '',
                Artists: item.Artists || [],
                IndexNumber: item.IndexNumber || 0,
                RunTimeTicks: item.RunTimeTicks || 0,
                Id: item.Id || ''
            };
            console.debug('[Media] notifyMetadata:', meta.Name);
            window.jmpNative.notifyMetadata(JSON.stringify(meta));
            this.fetchAlbumArt(item);
        }

        getImageUrl(item, baseUrl) {
            const imageTags = item.ImageTags || {};
            const itemType = item.Type || '';
            const mediaType = item.MediaType || '';

            if (itemType === 'Episode') {
                if (item.SeriesId && item.SeriesPrimaryImageTag) {
                    return baseUrl + '/Items/' + item.SeriesId + '/Images/Primary?tag=' + item.SeriesPrimaryImageTag + '&maxWidth=512';
                }
                if (item.SeasonId && item.SeasonPrimaryImageTag) {
                    return baseUrl + '/Items/' + item.SeasonId + '/Images/Primary?tag=' + item.SeasonPrimaryImageTag + '&maxWidth=512';
                }
            }

            if (mediaType === 'Audio' || itemType === 'Audio') {
                if (item.AlbumId && item.AlbumPrimaryImageTag) {
                    return baseUrl + '/Items/' + item.AlbumId + '/Images/Primary?tag=' + item.AlbumPrimaryImageTag + '&maxWidth=512';
                }
            }

            if (imageTags.Primary && item.Id) {
                return baseUrl + '/Items/' + item.Id + '/Images/Primary?tag=' + imageTags.Primary + '&maxWidth=512';
            }
            if (item.BackdropImageTags && item.BackdropImageTags.length > 0 && item.Id) {
                return baseUrl + '/Items/' + item.Id + '/Images/Backdrop/0?tag=' + item.BackdropImageTags[0] + '&maxWidth=512';
            }

            return null;
        }

        cancelArtwork() {
            this.artworkAbortController?.abort();
            this.artworkReader?.abort();
            this.artworkAbortController = null;
            this.artworkReader = null;
            this.pendingArtworkUrl = null;
        }

        fetchAlbumArt(item) {
            if (!item || !window.jmpNative) return;
            const baseUrl = window.ApiClient?.serverAddress?.();
            const imageUrl = baseUrl ? this.getImageUrl(item, baseUrl) : null;
            if (imageUrl && imageUrl === this.pendingArtworkUrl) return;
            this.cancelArtwork();
            if (!imageUrl) return;

            const controller = new AbortController();
            const { signal } = controller;
            this.pendingArtworkUrl = imageUrl;
            this.artworkAbortController = controller;
            const finish = () => {
                if (this.artworkAbortController !== controller) return;
                this.artworkAbortController = null;
                this.artworkReader = null;
                this.pendingArtworkUrl = null;
            };
            fetch(imageUrl, { signal })
                .then(response => {
                    if (!response.ok) throw new Error('Failed to fetch image');
                    return response.blob();
                })
                .then(blob => {
                    if (signal.aborted) return;
                    const reader = new FileReader();
                    this.artworkReader = reader;
                    reader.onloadend = () => {
                        if (!signal.aborted && !reader.error) {
                            window.jmpNative.notifyArtwork(reader.result);
                        }
                        reader.onloadend = null;
                        finish();
                    };
                    reader.readAsDataURL(blob);
                })
                .catch(err => {
                    if (err.name !== 'AbortError') {
                        console.warn('[Media] Album art fetch failed:', err.message);
                    }
                    finish();
                });
        }

        updateQueueState() {
            try {
                if (!window.jmpNative) return;

                const pm = this.playbackManager;
                if (!pm) return;

                const qm = pm._playQueueManager;
                const playlist = qm?.getPlaylist();
                const currentIndex = qm?.getCurrentPlaylistIndex();

                if (!playlist || !Array.isArray(playlist) || playlist.length === 0 ||
                    currentIndex === undefined || currentIndex === null || currentIndex < 0 || currentIndex >= playlist.length) {
                    window.jmpNative.notifyQueueChange(false, false);
                    return;
                }

                const canNext = currentIndex < playlist.length - 1;

                const state = pm.getPlayerState ? pm.getPlayerState() : null;
                const isMusic = state?.NowPlayingItem?.MediaType === 'Audio';
                const canPrev = isMusic ? true : (currentIndex > 0);

                console.debug('[Media] updateQueueState: idx=' + currentIndex + ' len=' + playlist.length + ' canNext=' + canNext + ' canPrev=' + canPrev);
                window.jmpNative.notifyQueueChange(canNext, canPrev);
            } catch (e) {
                console.error('[Media] updateQueueState error:', e);
            }
        }

        listen(owner, name, callback, subscriptions = this.subscriptions) {
            window.Events.on(owner, name, callback);
            subscriptions.push(() => window.Events.off(owner, name, callback));
        }

        connect(signal, callback) {
            signal.connect(callback);
            this.subscriptions.push(() => signal.disconnect(callback));
        }

        setupEvents(pm) {
            this.listen(pm, 'playbackstart', (_event, player) => {
                const state = pm.getPlayerState?.();
                if (state?.NowPlayingItem) this.notifyMetadata(state.NowPlayingItem);
                if (player !== this.attachedPlayer) {
                    this.playerSubscriptions.splice(0).forEach(dispose => dispose());
                    this.attachedPlayer = player;
                    if (player) this.listen(player, 'playing', () => this.updateQueueState(), this.playerSubscriptions);
                }
                this.updateQueueState();
            });
            this.listen(pm, 'playbackstop', () => {
                this.playerSubscriptions.splice(0).forEach(dispose => dispose());
                this.attachedPlayer = null;
                this.cancelArtwork();
                window.jmpNative?.notifyQueueChange(false, false);
            });
            for (const name of ['playlistitemremove', 'playlistitemadd', 'playlistitemchange']) {
                this.listen(pm, name, () => this.updateQueueState());
            }
            const remap = { play_pause: 'playpause', seek_forward: 'fastforward', seek_backward: 'rewind' };
            this.connect(window.api.input.hostInput, actions => {
                for (const action of actions) this.inputManager?.handleCommand?.(remap[action] || action, {});
            });
            this.connect(window.api.input.positionSeek, positionMs => {
                const player = pm.getCurrentPlayer ? pm.getCurrentPlayer() : pm._currentPlayer;
                const duration = pm.duration?.() || 0;
                if (player && duration > 0) pm.seekPercent(positionMs * 10000 / duration * 100, player);
            });
            this.connect(window.api.input.rateChanged, rate => {
                const player = pm.getCurrentPlayer ? pm.getCurrentPlayer() : pm._currentPlayer;
                player?.applyPlaybackRate?.(rate);
            });
        }

        destroy() {
            this.playerSubscriptions.splice(0).forEach(dispose => dispose());
            this.subscriptions.splice(0).forEach(dispose => dispose());
            this.cancelArtwork();
            this.attachedPlayer = null;
            this.playbackManager = null;
            this.inputManager = null;
        }

    }

    window._inputPlugin = inputPlugin;
    console.debug('[Media] inputPlugin class installed');
})();
