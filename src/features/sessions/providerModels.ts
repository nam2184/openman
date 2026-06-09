import providerModels from "../../config/provider-models.json";

const modelsByProvider = providerModels as Record<string, string[]>;

export function getModelOptions(provider: string, currentModel?: string) {
  const configuredModels = modelsByProvider[provider] ?? [];
  const trimmedCurrentModel = currentModel?.trim();

  if (trimmedCurrentModel && !configuredModels.includes(trimmedCurrentModel)) {
    return [trimmedCurrentModel, ...configuredModels];
  }

  return configuredModels;
}

export function getDefaultModel(provider: string, fallback = "") {
  return modelsByProvider[provider]?.[0] ?? fallback;
}
