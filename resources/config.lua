config.window = {
    width = 640,
    height = 32,
}
config.model = {
    path = "../llama-rs/models/llama/13B/ggml-vicuna-13b-1.1-q4_1.bin",
    context_token_length = 2048,
    architecture = "llama",
    prefer_mmap = true,
}
config.hotkeys = {
    ["LControl"] = {
        ["Escape"] = function()
            ui.singleline(function(prompt)
                local chunk, err = load("return " .. prompt)
                if err == nil then
                    input.key_sequence(tostring(chunk()))
                else
                    llm.infer(prompt, function(token)
                        input.key_sequence(token)
                    end)
                end
            end)
        end
    }
}