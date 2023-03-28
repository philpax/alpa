internal.dispatch = function(keycodes)
    local t = config.hotkeys
    for _, v in keycodes do
        t = t[v]
    end

    local handlers = {}
    if type(t) == "table" then
        handlers = t
    elseif type(t) == "function" then
        handlers = {t}
    else
        error("unexpected type when handling keycodes")
    end

    for _, handler in ipairs(handlers) do
        handler()
    end
end