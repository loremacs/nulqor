# llama.cpp server provider

OpenAI-compatible API from [llama.cpp server](https://github.com/ggerganov/llama.cpp) (default `http://localhost:8080`).

## 8 GB VRAM

Start the server with one GGUF model loaded:

```powershell
llama-server -m C:\models\Qwen2.5-7B-Instruct-Q4_K_M.gguf --port 8080
```

Use Q4_K_M or Q5 quantizations for 7–8B models on 8 GB VRAM.

Set `active_provider = "llamacpp"` in `nulqor.toml` and restart Nulqor.

**Note:** llama.cpp loads the model at server start. **Stop model** clears Nulqor's selection only; it does not unload VRAM (restart the server to swap models).
