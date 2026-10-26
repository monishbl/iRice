-- Touchpad gestures (and touchscreen space swipes).
--   3 fingers left/right   switch spaces
--   3 fingers up           overview
--   3 fingers down         App Library
--   4 fingers left/right   AI sidebar / Notification Center
-- Scroll on the menu bar's island (or Super+scroll) also steps through spaces.
local function shell(name)
    return function() hl.dispatch(hl.dsp.global("irice:" .. name)) end
end

hl.gesture({ fingers = 3, direction = "horizontal", action = "workspace" })
hl.gesture({ fingers = 3, direction = "up", action = shell("overview") })
hl.gesture({ fingers = 3, direction = "down", action = shell("launchpad") })
hl.gesture({ fingers = 4, direction = "left", action = shell("sidebar") })
hl.gesture({ fingers = 4, direction = "right", action = shell("notificationCenter") })

hl.config({ gestures = { workspace_swipe_touch = true } })
