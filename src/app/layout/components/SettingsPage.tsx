import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, Moon, Sun, Plus, Save, Settings } from "lucide-react";
import { useEffect, useMemo, useState, type ChangeEvent } from "react";
import {
  getContextWindow,
  getDefaultModel,
  getMaxOutput,
  getModelOptions,
  getModelSpec,
} from "../../../features/sessions/providerModels";
import type { ProviderConfig } from "../../../features/sessions/sessionStore";
import { cn } from "../../../lib/utils";
import { useAppStore } from "../../../features/app/appStore";
import { Button } from "../../../components/ui/button";
import { Input } from "../../../components/ui/input";

interface ProviderDraft {
  name: string;
  model: string;
  api_key: string;
  base_url: string;
  protocol: "openai" | "anthropic";
  enabled: boolean;
}

const emptyProviderDraft: ProviderDraft = {
  name: "",
  model: "",
  api_key: "",
  base_url: "",
  protocol: "openai",
  enabled: true,
};

export function SettingsPage() {
  const { settings, saveTheme, setView } = useAppStore();
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [selectedProviderName, setSelectedProviderName] = useState("");
  const [providerDraft, setProviderDraft] = useState<ProviderDraft>(emptyProviderDraft);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    loadProviders().catch((loadError) => {
      setError(formatError(loadError));
    });
  }, []);

  async function loadProviders() {
    const configs = await invoke<ProviderConfig[]>("get_provider_configs");
    setProviders(configs);

    const selected = configs.find((config) => config.name === selectedProviderName) ?? configs[0];
    if (selected) {
      setSelectedProviderName(selected.name);
      setProviderDraft(providerToDraft(selected));
    }
  }

  function selectProvider(name: string) {
    setSelectedProviderName(name);
    const provider = providers.find((config) => config.name === name);
    if (provider) {
      setProviderDraft(providerToDraft(provider));
    }
  }

  async function saveProviderConfig() {
    if (!providerDraft.name.trim() || !providerDraft.model.trim()) {
      setError("Provider name and default model are required.");
      return;
    }

    setIsSaving(true);
    setError(null);
    setStatus(null);

    try {
      const config: ProviderConfig = {
        name: providerDraft.name.trim(),
        model: providerDraft.model.trim(),
        api_key: providerDraft.api_key.trim() || null,
        base_url: providerDraft.base_url.trim() || null,
        protocol: providerDraft.protocol,
        enabled: providerDraft.enabled,
      };

      await invoke("upsert_provider_config", { config });
      setStatus("Provider config saved.");
      setSelectedProviderName(config.name);
      await loadProviders();
    } catch (saveError) {
      setError(formatError(saveError));
    } finally {
      setIsSaving(false);
    }
  }

  async function handleThemeToggle() {
    const newTheme = settings.theme === "dark" ? "light" : "dark";
    await saveTheme(newTheme);
  }

  const modelOptions = getModelOptions(providerDraft.name, providerDraft.model);
  const selectedSpec = useMemo(
    () => getModelSpec(providerDraft.model),
    [providerDraft.model]
  );
  const modelContextWindow = selectedSpec?.context_window ?? getContextWindow(providerDraft.model);
  const modelMaxOutput = selectedSpec?.max_output ?? getMaxOutput(providerDraft.model);

  return (
    <div className="flex h-full flex-col bg-black text-white">
      <header className="flex items-center gap-4 border-b border-[#1f1f1f] px-6 py-4">
        <Button variant="ghost" size="icon" onClick={() => setView("canvas")} aria-label="Back to canvas">
          <ArrowLeft className="h-5 w-5" />
        </Button>
        <div className="flex items-center gap-2">
          <Settings className="h-5 w-5" />
          <h1 className="text-lg font-semibold">Settings</h1>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-2xl space-y-8 p-6">
          <section className="space-y-4">
            <h2 className="text-sm font-medium text-[#d4d4d4]">Appearance</h2>
            <div className="rounded-lg border border-[#2a2a2a] bg-[#0a0a0a] p-4">
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-sm font-medium">Theme</p>
                  <p className="text-xs text-[#8a8a8a]">Choose your preferred color scheme</p>
                </div>
                <Button variant="secondary" size="icon" onClick={handleThemeToggle}>
                  {settings.theme === "dark" ? (
                    <Moon className="h-4 w-4" />
                  ) : (
                    <Sun className="h-4 w-4" />
                  )}
                </Button>
              </div>
            </div>
          </section>

          <section className="space-y-4">
            <h2 className="text-sm font-medium text-[#d4d4d4]">AI Configuration</h2>
            <div className="space-y-4 rounded-lg border border-[#2a2a2a] bg-[#0a0a0a] p-4">
              <div className="flex gap-2">
                <select
                  value={selectedProviderName}
                  onChange={(event) => selectProvider(event.target.value)}
                  className="h-9 min-w-0 flex-1 rounded-md border border-[#2a2a2a] bg-black px-3 text-sm text-white outline-none transition-colors hover:border-[#4a4a4a] focus:border-white"
                >
                  {providers.map((provider) => (
                    <option key={provider.name} value={provider.name}>{provider.name}</option>
                  ))}
                </select>
                <Button
                  variant="secondary"
                  size="icon"
                  onClick={() => {
                    setSelectedProviderName("");
                    setProviderDraft(emptyProviderDraft);
                    setStatus(null);
                    setError(null);
                  }}
                  aria-label="New provider"
                >
                  <Plus className="h-4 w-4" />
                </Button>
              </div>

              <label className="block space-y-1.5">
                <span className="text-xs font-medium text-[#d4d4d4]">Name</span>
                <Input
                  value={providerDraft.name}
                  onChange={(event: ChangeEvent<HTMLInputElement>) => {
                    const name = event.target.value;
                    setProviderDraft((draft) => ({
                      ...draft,
                      name,
                      model: draft.model || getDefaultModel(name),
                      protocol: inferProtocol(name),
                    }));
                  }}
                  placeholder="anthropic"
                />
              </label>
              <label className="block space-y-1.5">
                <span className="text-xs font-medium text-[#d4d4d4]">Protocol</span>
                <select
                  value={providerDraft.protocol}
                  onChange={(event: ChangeEvent<HTMLSelectElement>) => setProviderDraft((draft) => ({ ...draft, protocol: event.target.value as ProviderDraft["protocol"] }))}
                  className="h-9 w-full rounded-md border border-[#2a2a2a] bg-black px-3 text-sm text-white outline-none transition-colors hover:border-[#4a4a4a] focus:border-white"
                >
                  <option value="openai">OpenAI-compatible chat</option>
                  <option value="anthropic">Anthropic messages</option>
                </select>
              </label>
              <label className="block space-y-1.5">
                <span className="text-xs font-medium text-[#d4d4d4]">Default Model</span>
                <select
                  value={providerDraft.model}
                  onChange={(event: ChangeEvent<HTMLSelectElement>) => setProviderDraft((draft) => ({ ...draft, model: event.target.value }))}
                  className="h-9 w-full rounded-md border border-[#2a2a2a] bg-black px-3 text-sm text-white outline-none transition-colors hover:border-[#4a4a4a] focus:border-white disabled:cursor-not-allowed disabled:opacity-50"
                  disabled={modelOptions.length === 0}
                >
                  {modelOptions.length === 0 ? (
                    <option value="">Add models in src/config/provider-models.json</option>
                  ) : (
                    modelOptions.map((model) => (
                      <option key={model} value={model}>{model}</option>
                    ))
                  )}
                </select>
              </label>
              <label className="block space-y-1.5">
                <span className="text-xs font-medium text-[#d4d4d4]">API Key</span>
                <Input
                  type="password"
                  value={providerDraft.api_key}
                  onChange={(event: ChangeEvent<HTMLInputElement>) => setProviderDraft((draft) => ({ ...draft, api_key: event.target.value }))}
                  placeholder="Stored locally"
                />
              </label>
              <label className="block space-y-1.5">
                <span className="text-xs font-medium text-[#d4d4d4]">Base URL</span>
                <Input
                  value={providerDraft.base_url}
                  onChange={(event: ChangeEvent<HTMLInputElement>) => setProviderDraft((draft) => ({ ...draft, base_url: event.target.value }))}
                  placeholder="Optional provider endpoint"
                />
              </label>
              <div className="rounded-md border border-[#1f1f1f] bg-[#050505] p-3 text-xs text-[#bdbdbd]">
                <p className="text-[10px] uppercase tracking-wider text-[#737373]">Context budget (from model spec)</p>
                <p className="mt-1">
                  Window: <span className="text-white">{modelContextWindow.toLocaleString()}</span> tokens
                </p>
                <p>
                  Max output: <span className="text-white">{modelMaxOutput.toLocaleString()}</span> tokens
                </p>
                {!selectedSpec && providerDraft.model && (
                  <p className="mt-1 text-[#ff8a3d]">
                    Model not in registry — using fallback limits.
                  </p>
                )}
              </div>
              <label className="flex items-center gap-2 text-sm text-[#d4d4d4]">
                <input
                  type="checkbox"
                  checked={providerDraft.enabled}
                  onChange={(event) => setProviderDraft((draft) => ({ ...draft, enabled: event.target.checked }))}
                  className="h-4 w-4 rounded border-[#2a2a2a] bg-black accent-white"
                />
                Enabled for new sessions
              </label>
              <Button className="w-full" onClick={saveProviderConfig} disabled={isSaving}>
                <Save className="h-4 w-4" />
                Save Provider
              </Button>

              {(status || error) && (
                <p className={cn("text-xs", error ? "text-[#ff5f5f]" : "text-[#bdbdbd]")}>{error ?? status}</p>
              )}
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}

function providerToDraft(provider: ProviderConfig): ProviderDraft {
  return {
    name: provider.name,
    model: provider.model,
    api_key: provider.api_key ?? "",
    base_url: provider.base_url ?? "",
    protocol: provider.protocol ?? inferProtocol(provider.name),
    enabled: provider.enabled,
  };
}

function inferProtocol(name: string): ProviderDraft["protocol"] {
  return name.toLowerCase() === "anthropic" ? "anthropic" : "openai";
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
