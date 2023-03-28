local handlers = {}

local keycodes_to_table_key = function(keycodes)
    return table.concat(keycodes, "+")
end

internal.dispatch = function(keycodes)
    local keycodes = keycodes_to_table_key(keycodes)
    for _, handler in ipairs(handlers[keycodes] or {}) do
        handler()
    end
end

on_keys = function(keycodes, f)
    internal.listen_for_hotkeys(keycodes)

    local keycodes = keycodes_to_table_key(keycodes)
    if handlers[keycodes] == nil then
        handlers[keycodes] = {}
    end
    table.insert(handlers[keycodes], f)
end