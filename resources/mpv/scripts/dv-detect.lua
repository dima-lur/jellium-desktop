-- The selected flag is not populated at on_preloaded. mpv already exposes
-- the stream's Dolby Vision profile in track-list at this point.
if mp.get_property("platform") ~= "windows" then return end
local marker = "user-data/dv-detect/claimed"
if mp.get_property_native(marker, false) then return end
mp.set_property_native(marker, true)

local last_profile, last_path
local pending_cycle, awaiting_windowed

local function classify()
    for _, track in ipairs(mp.get_property_native("track-list", {})) do
        if track.type == "video" and track["dolby-vision-profile"] then
            return "dolby_vision"
        end
    end
    return "default"
end

mp.add_hook("on_preloaded", 50, function()
    local profile, path = classify(), mp.get_property("path")
    if profile == last_profile and path == last_path then return end
    local option = profile == "dolby_vision" and "no" or "yes"
    local hint, hint_error = mp.set_property("target-colorspace-hint", option)
    local flip, flip_error = mp.set_property("d3d11-flip", option)
    if not hint or not flip then
        mp.msg.error("Setting " .. profile .. " presentation: " .. tostring(hint_error or flip_error))
        last_path = nil
        return
    end
    pending_cycle = last_profile == "dolby_vision" and profile == "default"
    last_profile, last_path = profile, path
    mp.msg.info("Windows presentation profile: " .. profile)
end)

-- After the replacement VO starts presenting, let DWM renegotiate fullscreen
-- output on displays that otherwise remain in DV when SDR or HDR10 resumes.
mp.register_event("playback-restart", function()
    if not pending_cycle then return end
    pending_cycle = false
    if mp.get_property_native("fullscreen", false) then
        awaiting_windowed = mp.set_property_native("fullscreen", false)
    end
end)

mp.observe_property("fullscreen", "bool", function(_, fullscreen)
    if awaiting_windowed and fullscreen == false then
        awaiting_windowed = false
        mp.set_property_native("fullscreen", true)
    end
end)

mp.register_event("end-file", function()
    pending_cycle, awaiting_windowed = false, false
end)
