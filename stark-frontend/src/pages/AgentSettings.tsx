import { useState, useEffect, FormEvent } from 'react';
import { Save } from 'lucide-react';
import Card, { CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import Button from '@/components/ui/Button';
import Input from '@/components/ui/Input';
import { getAgentSettings, updateAgentSettings } from '@/lib/api';

const ENDPOINTS = {
  llama: 'https://llama.defirelay.com/api/v1/chat/completions',
  kimi: 'https://kimi.defirelay.com/api/v1/chat/completions',
};

const DEFAULT_MODELS: Record<string, string> = {
  llama: 'default',
  kimi: 'default',
};

type EndpointOption = 'llama' | 'kimi' | 'custom';
type ModelArchetype = 'llama' | 'kimi' | 'anthropic' | 'openai';

interface Settings {
  endpoint?: string;
  model_archetype?: string;
  max_tokens?: number;
}

export default function AgentSettings() {
  const [endpointOption, setEndpointOption] = useState<EndpointOption>('llama');
  const [customEndpoint, setCustomEndpoint] = useState('');
  const [modelArchetype, setModelArchetype] = useState<ModelArchetype>('llama');
  const [maxTokens, setMaxTokens] = useState(40000);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const data = await getAgentSettings() as Settings;

      // Determine which endpoint option is being used
      if (data.endpoint === ENDPOINTS.llama) {
        setEndpointOption('llama');
      } else if (data.endpoint === ENDPOINTS.kimi) {
        setEndpointOption('kimi');
      } else if (data.endpoint) {
        setEndpointOption('custom');
        setCustomEndpoint(data.endpoint);
      } else {
        setEndpointOption('llama');
      }

      // Set model archetype
      if (data.model_archetype && ['llama', 'kimi', 'anthropic', 'openai'].includes(data.model_archetype)) {
        setModelArchetype(data.model_archetype as ModelArchetype);
      }

      // Set max tokens
      if (data.max_tokens && data.max_tokens > 0) {
        setMaxTokens(data.max_tokens);
      }
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to load settings' });
    } finally {
      setIsLoading(false);
    }
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setIsSaving(true);
    setMessage(null);

    let endpoint: string;
    if (endpointOption === 'llama') {
      endpoint = ENDPOINTS.llama;
    } else if (endpointOption === 'kimi') {
      endpoint = ENDPOINTS.kimi;
    } else {
      endpoint = customEndpoint;
    }

    if (endpointOption === 'custom' && !customEndpoint.trim()) {
      setMessage({ type: 'error', text: 'Please enter a custom endpoint URL' });
      setIsSaving(false);
      return;
    }

    try {
      // Use default model for known endpoints, empty for custom
      const model = endpointOption === 'custom' ? '' : (DEFAULT_MODELS[endpointOption] || '');

      await updateAgentSettings({
        endpoint,
        model_archetype: modelArchetype,
        max_tokens: maxTokens,
        // Keep these for backend compatibility
        provider: 'openai_compatible',
        api_key: '',
        model,
      });
      setMessage({ type: 'success', text: 'Settings saved successfully' });
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to save settings' });
    } finally {
      setIsSaving(false);
    }
  };

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center">
        <div className="flex items-center gap-3">
          <div className="w-6 h-6 border-2 border-stark-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-slate-400">Loading settings...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="p-8">
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-white mb-2">Agent Settings</h1>
        <p className="text-slate-400">Configure your AI agent endpoint and model type</p>
      </div>

      <form onSubmit={handleSubmit}>
        <div className="grid gap-6 max-w-2xl">
          <Card>
            <CardHeader>
              <CardTitle>Endpoint Configuration</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-slate-300 mb-2">
                  Agent Endpoint
                </label>
                <select
                  value={endpointOption}
                  onChange={(e) => setEndpointOption(e.target.value as EndpointOption)}
                  className="w-full px-4 py-3 bg-slate-900/50 border border-slate-600 rounded-lg text-white focus:outline-none focus:ring-2 focus:ring-stark-500 focus:border-transparent"
                >
                  <option value="llama">llama.defirelay.com</option>
                  <option value="kimi">kimi.defirelay.com</option>
                  <option value="custom">Custom Endpoint</option>
                </select>
              </div>

              {endpointOption === 'custom' && (
                <Input
                  label="Custom Endpoint URL"
                  value={customEndpoint}
                  onChange={(e) => setCustomEndpoint(e.target.value)}
                  placeholder="https://your-endpoint.com/v1/chat/completions"
                />
              )}

              <div>
                <label className="block text-sm font-medium text-slate-300 mb-2">
                  Model Archetype
                </label>
                <select
                  value={modelArchetype}
                  onChange={(e) => setModelArchetype(e.target.value as ModelArchetype)}
                  className="w-full px-4 py-3 bg-slate-900/50 border border-slate-600 rounded-lg text-white focus:outline-none focus:ring-2 focus:ring-stark-500 focus:border-transparent"
                >
                  <option value="llama">Llama</option>
                  <option value="kimi">Kimi</option>
                  <option value="anthropic">Anthropic</option>
                  <option value="openai">OpenAI</option>
                </select>
                <p className="text-xs text-slate-500 mt-1">
                  Select the model family to optimize prompt formatting
                </p>
              </div>

              <div>
                <label className="block text-sm font-medium text-slate-300 mb-2">
                  Max Tokens
                </label>
                <input
                  type="number"
                  value={maxTokens}
                  onChange={(e) => setMaxTokens(parseInt(e.target.value) || 40000)}
                  min={1000}
                  max={200000}
                  className="w-full px-4 py-3 bg-slate-900/50 border border-slate-600 rounded-lg text-white focus:outline-none focus:ring-2 focus:ring-stark-500 focus:border-transparent"
                />
                <p className="text-xs text-slate-500 mt-1">
                  Maximum tokens for AI response (default: 40,000)
                </p>
              </div>
            </CardContent>
          </Card>

          {message && (
            <div
              className={`px-4 py-3 rounded-lg ${
                message.type === 'success'
                  ? 'bg-green-500/20 border border-green-500/50 text-green-400'
                  : 'bg-red-500/20 border border-red-500/50 text-red-400'
              }`}
            >
              {message.text}
            </div>
          )}

          <Button type="submit" isLoading={isSaving} className="w-fit">
            <Save className="w-4 h-4 mr-2" />
            Save Settings
          </Button>
        </div>
      </form>
    </div>
  );
}
