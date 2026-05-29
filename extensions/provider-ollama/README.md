# Ollama provider

Local inference via [Ollama](https://ollama.com/) on `http://localhost:11434`.

## 8 GB VRAM suggestions

Pull one model at a time; avoid running LM Studio and Ollama heavy loads together.

| Model | Notes |
|-------|--------|
| `llama3.2:3b` | Fast, fits easily |
| `phi3:mini` | Small, capable |
| `gemma2:2b` | Very light |
| `qwen2.5:7b-instruct-q4_K_M` | Strong 7B quant |

```powershell
ollama pull llama3.2:3b
ollama serve
```

In Nulqor: set `active_provider = "ollama"` in `nulqor.toml`, restart, **Fetch models → Connect**.

## Commands

Backend namespace `ollama:*@1`; panels use `provider:*@1` via `provider-router`.
