import type { ModelCatalogItem } from "./types";

function presetItem(key: string): ModelCatalogItem {
  const [provider] = key.split("/");
  return {
    key,
    provider: provider?.trim() || "unknown",
    name: key,
    available: null,
    missing: false
  };
}

const PRESET_MODEL_KEYS = [
  "openai/gpt-4.1",
  "openai/gpt-4.1-mini",
  "openai/gpt-4.1-nano",
  "openai/gpt-4o",
  "openai/gpt-4o-mini",
  "openai/gpt-5",
  "openai/gpt-5-mini",
  "openai/gpt-5-nano",
  "openai/gpt-5-pro",
  "openai/gpt-5.1",
  "openai/gpt-5.1-codex",
  "openai/gpt-5.1-codex-max",
  "openai/gpt-5.1-codex-mini",
  "openai/gpt-5.2",
  "openai/gpt-5.2-codex",
  "openai/gpt-5.2-pro",
  "openai/gpt-5.3-codex",
  "openai/o3",
  "openai/o3-pro",
  "openai/o4-mini",
  "openai-codex/gpt-5.2",
  "openai-codex/gpt-5.2-codex",
  "openai-codex/gpt-5.3-codex",
  "anthropic/claude-3-7-sonnet-latest",
  "anthropic/claude-haiku-4-5",
  "anthropic/claude-opus-4-1",
  "anthropic/claude-opus-4-5",
  "anthropic/claude-opus-4-6",
  "anthropic/claude-sonnet-4-5",
  "anthropic/claude-sonnet-4-5-20250929",
  "anthropic/claude-sonnet-4-6",
  "google/gemini-2.0-flash",
  "google/gemini-2.0-flash-lite",
  "google/gemini-2.5-flash",
  "google/gemini-2.5-flash-lite",
  "google/gemini-2.5-pro",
  "google/gemini-3-flash-preview",
  "google/gemini-3-pro-preview",
  "google/gemini-3.1-pro-preview",
  "xai/grok-3",
  "xai/grok-3-fast",
  "xai/grok-3-mini",
  "xai/grok-4",
  "xai/grok-4-1-fast",
  "xai/grok-4-fast",
  "xai/grok-code-fast-1",
  "zai/glm-4.5",
  "zai/glm-4.5-air",
  "zai/glm-4.7",
  "zai/glm-4.7-flash",
  "zai/glm-5",
  "minimax/MiniMax-M2",
  "minimax/MiniMax-M2.1",
  "minimax/MiniMax-M2.5",
  "kimi-coding/k2p5",
  "kimi-coding/kimi-k2-thinking",
  "moonshot/kimi-k2.5",
  "openrouter/anthropic/claude-3.7-sonnet",
  "openrouter/anthropic/claude-haiku-4.5",
  "openrouter/anthropic/claude-opus-4.1",
  "openrouter/anthropic/claude-opus-4.6",
  "openrouter/anthropic/claude-sonnet-4.5",
  "openrouter/anthropic/claude-sonnet-4.6",
  "openrouter/google/gemini-2.5-flash",
  "openrouter/google/gemini-2.5-pro",
  "openrouter/google/gemini-3-flash-preview",
  "openrouter/google/gemini-3.1-pro-preview",
  "openrouter/minimax/minimax-m2.5",
  "openrouter/moonshotai/kimi-k2",
  "openrouter/moonshotai/kimi-k2-0905",
  "openrouter/moonshotai/kimi-k2-thinking",
  "openrouter/moonshotai/kimi-k2.5",
  "openrouter/openai/gpt-4.1",
  "openrouter/openai/gpt-4o",
  "openrouter/openai/gpt-4o-mini",
  "openrouter/openai/gpt-5",
  "openrouter/openai/gpt-5-mini",
  "openrouter/openai/gpt-5-pro",
  "openrouter/openai/gpt-5.1",
  "openrouter/openai/gpt-5.2",
  "openrouter/openai/gpt-5.3-codex",
  "openrouter/openai/o3",
  "openrouter/openai/o3-pro",
  "openrouter/openai/o4-mini",
  "openrouter/qwen/qwen-max",
  "openrouter/qwen/qwen-plus",
  "openrouter/qwen/qwen-turbo",
  "openrouter/qwen/qwen3-235b-a22b",
  "openrouter/qwen/qwen3-30b-a3b",
  "openrouter/qwen/qwen3-coder",
  "openrouter/qwen/qwen3-coder-plus",
  "openrouter/qwen/qwen3-max",
  "openrouter/xai/grok-3",
  "openrouter/xai/grok-4",
  "openrouter/zai/glm-4.7",
  "amazon-bedrock/anthropic.claude-sonnet-4-5-20250929-v1:0",
  "amazon-bedrock/moonshotai.kimi-k2.5",
  "vercel-ai-gateway/moonshotai/kimi-k2",
  "vercel-ai-gateway/moonshotai/kimi-k2-thinking",
  "vercel-ai-gateway/moonshotai/kimi-k2.5",
  "vercel-ai-gateway/qwen/qwen3-coder-plus",
  "vercel-ai-gateway/qwen/qwen3-max",
  "opencode/claude-sonnet-4-5",
  "opencode/gpt-5.2",
  "opencode/kimi-k2.5",
  "opencode/qwen3-coder-plus"
] as const;

export const WIZARD_PRESET_MODEL_CATALOG: ModelCatalogItem[] = PRESET_MODEL_KEYS.map((key) =>
  presetItem(key)
);

export function mergeModelCatalogOptions(...sources: ModelCatalogItem[][]): ModelCatalogItem[] {
  const map = new Map<string, ModelCatalogItem>();
  for (const source of sources) {
    for (const item of source) {
      const key = item.key.trim();
      if (!key || map.has(key)) continue;
      map.set(key, {
        key,
        provider: item.provider,
        name: item.name || key,
        available: item.available ?? null,
        missing: !!item.missing
      });
    }
  }
  return Array.from(map.values()).sort((a, b) => a.key.localeCompare(b.key));
}
