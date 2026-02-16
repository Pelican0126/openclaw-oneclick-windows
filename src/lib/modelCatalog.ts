import type { ModelCatalogItem } from "./types";

function model(key: string, name: string): ModelCatalogItem {
  const provider = key.includes("/") ? key.split("/")[0] : "unknown";
  return {
    key,
    provider,
    name,
    available: null,
    missing: false
  };
}

// Frontend fallback for instant render when backend catalog query is slow.
export const FALLBACK_MODEL_CATALOG: ModelCatalogItem[] = [
  model("openai/gpt-5.2", "GPT-5.2"),
  model("openai/gpt-4.1", "GPT-4.1"),
  model("openai/o3", "o3"),
  model("openai/o4-mini", "o4-mini"),
  model("anthropic/claude-opus-4-6", "Claude Opus 4.6"),
  model("anthropic/claude-sonnet-4-5", "Claude Sonnet 4.5"),
  model("anthropic/claude-3-7-sonnet-latest", "Claude 3.7 Sonnet Latest"),
  model("google/gemini-2.5-pro", "Gemini 2.5 Pro"),
  model("google/gemini-2.5-flash", "Gemini 2.5 Flash"),
  model("moonshot/kimi-k2-0905-preview", "Kimi K2 0905 Preview"),
  model("moonshot/kimi-k2-250711", "Kimi K2 250711"),
  model("moonshot/kimi-2.5", "Kimi 2.5"),
  model("xai/grok-4", "Grok 4"),
  model("xai/grok-3", "Grok 3"),
  model("openrouter/moonshotai/kimi-k2", "OpenRouter Kimi K2"),
  model("zai/glm-4.5", "GLM 4.5"),
  model("minimax/minimax-m1", "MiniMax M1"),
  model("qwen/qwen3-max", "Qwen 3 Max")
].sort((a, b) => a.key.localeCompare(b.key));

export function mergeModelCatalog(...sources: ModelCatalogItem[][]): ModelCatalogItem[] {
  const dedup = new Map<string, ModelCatalogItem>();
  for (const source of sources) {
    for (const item of source) {
      if (!item?.key?.trim()) continue;
      if (!dedup.has(item.key)) {
        dedup.set(item.key, item);
      }
    }
  }
  return Array.from(dedup.values()).sort((a, b) => a.key.localeCompare(b.key));
}
