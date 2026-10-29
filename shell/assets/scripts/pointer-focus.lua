-- Called after a scene change, never on a polling loop. Moving to the same
-- position asks Hyprland to repeat its own hit-test, including layer grabs.
--
-- With a window (one that just opened): the pointer moves onto it, so focus
-- that follows the pointer doesn't snap straight back to the window under it.
-- Only for a window Hyprland has already focused and shows: focusing is
-- Hyprland's call (window rules such as no_initial_focus, a window opening on
-- a space you can't see, a dialog meant to stay behind), not this script's.
return function(address)
    local p = hl.get_cursor_pos()
    if not p then return end
    if address and address ~= "" then
        local w = hl.get_window("address:" .. address)
        if not w or not w.mapped or w.hidden or not w.accepts_input then return end
        if not w.active or w.visible == false then return end
        local a, s = w.at, w.size
        if not a or not s or s.x <= 0 or s.y <= 0 then return end
        local x, y = p.x, p.y
        if x < a.x or y < a.y or x >= a.x + s.x or y >= a.y + s.y then
            x, y = a.x + s.x / 2, a.y + s.y / 2
        end
        hl.dispatch(hl.dsp.cursor.move({x = x, y = y}))
    else
        hl.dispatch(hl.dsp.cursor.move({x = p.x, y = p.y}))
    end
end
