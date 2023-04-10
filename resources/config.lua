config.window = {
    width = 640,
    height = 32,
}
config.hotkeys = {
    ["LControl"] = {
        ["Escape"] = function()
            ui.singleline(function(prompt)
                print(prompt)
            end)
        end
    }
}