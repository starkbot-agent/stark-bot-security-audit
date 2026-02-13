import { useState, useEffect } from 'react';
import { Key, Trash2, Plus, Clock, ClipboardCopy, Check, AlertTriangle } from 'lucide-react';
import Card, { CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import Button from '@/components/ui/Button';
import Input from '@/components/ui/Input';
import { getApiKeys, upsertApiKey, deleteApiKey, getApiKeyValue, getServiceConfigs, ApiKey, ServiceConfig } from '@/lib/api';

function getServiceLabel(keyName: string, configs: ServiceConfig[]): string | null {
  for (const config of configs) {
    for (const key of config.keys) {
      if (key.name === keyName) return config.label;
    }
  }
  return null;
}

function formatTimestamp(ts: string): string {
  try {
    const d = new Date(ts);
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
  } catch {
    return ts;
  }
}

export default function ApiKeys() {
  const [keys, setKeys] = useState<ApiKey[]>([]);
  const [serviceConfigs, setServiceConfigs] = useState<ServiceConfig[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  // Add form state
  const [showAddForm, setShowAddForm] = useState(false);
  const [selectedGroup, setSelectedGroup] = useState(''); // group name or '__custom__'
  const [customKeyName, setCustomKeyName] = useState('');
  const [keyValues, setKeyValues] = useState<Record<string, string>>({});
  const [isSaving, setIsSaving] = useState(false);

  // Delete confirmation
  const [deletingKey, setDeletingKey] = useState<string | null>(null);

  // Copy state
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [copyingKey, setCopyingKey] = useState<string | null>(null);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      const [keysData, configsData] = await Promise.all([
        getApiKeys(),
        getServiceConfigs(),
      ]);
      setKeys(keysData);
      setServiceConfigs(configsData);
    } catch {
      setMessage({ type: 'error', text: 'Failed to load API keys' });
    } finally {
      setIsLoading(false);
    }
  };

  const loadKeys = async () => {
    try {
      const data = await getApiKeys();
      setKeys(data);
    } catch {
      setMessage({ type: 'error', text: 'Failed to load API keys' });
    }
  };

  const isCustom = selectedGroup === '__custom__';
  const selectedService = serviceConfigs.find(c => c.group === selectedGroup);

  const resetForm = () => {
    setShowAddForm(false);
    setSelectedGroup('');
    setCustomKeyName('');
    setKeyValues({});
  };

  const handleSave = async () => {
    setIsSaving(true);
    setMessage(null);

    try {
      if (isCustom) {
        const name = customKeyName.trim().toUpperCase();
        const value = keyValues['__custom__']?.trim();
        if (!name) {
          setMessage({ type: 'error', text: 'Please enter a key name' });
          setIsSaving(false);
          return;
        }
        if (!value) {
          setMessage({ type: 'error', text: 'Please enter a key value' });
          setIsSaving(false);
          return;
        }
        await upsertApiKey(name, value);
        setMessage({ type: 'success', text: `${name} saved successfully` });
      } else if (selectedService) {
        // Validate all fields have values
        const emptyKeys = selectedService.keys.filter(k => !keyValues[k.name]?.trim());
        if (emptyKeys.length === selectedService.keys.length) {
          setMessage({ type: 'error', text: 'Please enter at least one key value' });
          setIsSaving(false);
          return;
        }

        // Save all non-empty keys
        let savedCount = 0;
        for (const keyConfig of selectedService.keys) {
          const value = keyValues[keyConfig.name]?.trim();
          if (value) {
            await upsertApiKey(keyConfig.name, value);
            savedCount++;
          }
        }
        setMessage({ type: 'success', text: `Saved ${savedCount} ${selectedService.label} key${savedCount !== 1 ? 's' : ''}` });
      }
      resetForm();
      await loadKeys();
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to save API key';
      setMessage({ type: 'error', text: msg });
    } finally {
      setIsSaving(false);
    }
  };

  const handleDelete = async (keyName: string) => {
    setDeletingKey(keyName);
    setMessage(null);

    try {
      await deleteApiKey(keyName);
      setMessage({ type: 'success', text: `${keyName} deleted` });
      await loadKeys();
    } catch {
      setMessage({ type: 'error', text: 'Failed to delete API key' });
    } finally {
      setDeletingKey(null);
    }
  };

  const handleCopy = async (keyName: string) => {
    setCopyingKey(keyName);
    try {
      const value = await getApiKeyValue(keyName);
      await navigator.clipboard.writeText(value);
      setCopiedKey(keyName);
      setTimeout(() => setCopiedKey(null), 2000);
    } catch {
      setMessage({ type: 'error', text: `Failed to copy ${keyName}` });
    } finally {
      setCopyingKey(null);
    }
  };

  // Check if save button should be enabled
  const canSave = (() => {
    if (isCustom) {
      return customKeyName.trim() !== '' && !!keyValues['__custom__']?.trim();
    }
    if (selectedService) {
      return selectedService.keys.some(k => !!keyValues[k.name]?.trim());
    }
    return false;
  })();

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center">
        <div className="flex items-center gap-3">
          <div className="w-6 h-6 border-2 border-stark-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-slate-400">Loading API keys...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="p-8">
      {/* Header */}
      <div className="mb-8 flex items-start justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white mb-2">API Keys</h1>
          <p className="text-slate-400">
            Manage API keys for external services.
          </p>
        </div>
        {!showAddForm && (
          <Button onClick={() => setShowAddForm(true)}>
            <Plus className="w-4 h-4 mr-2" />
            Add API Key
          </Button>
        )}
      </div>

      <div className="flex flex-col xl:flex-row gap-6">
        {/* Main content */}
        <div className="flex-1 min-w-0">
          {/* Message */}
          {message && (
            <div
              className={`mb-6 px-4 py-3 rounded-lg ${
                message.type === 'success'
                  ? 'bg-green-500/20 border border-green-500/50 text-green-400'
                  : 'bg-red-500/20 border border-red-500/50 text-red-400'
              }`}
            >
              {message.text}
            </div>
          )}

          {/* Add Form */}
          {showAddForm && (
            <div className="mb-6">
              <Card>
                <CardHeader>
                  <CardTitle>Add API Key</CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="space-y-4 max-w-lg">
                    {/* Service group selector */}
                    <div>
                      <label className="block text-sm font-medium text-slate-300 mb-1">
                        Service
                      </label>
                      <select
                        value={selectedGroup}
                        onChange={(e) => {
                          setSelectedGroup(e.target.value);
                          setCustomKeyName('');
                          setKeyValues({});
                        }}
                        className="w-full bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:ring-2 focus:ring-stark-500 focus:border-transparent"
                      >
                        <option value="">Select a service...</option>
                        {serviceConfigs.map((config) => (
                          <option key={config.group} value={config.group}>
                            {config.label}
                          </option>
                        ))}
                        <option value="__custom__">Custom...</option>
                      </select>
                    </div>

                    {/* Custom key name input */}
                    {isCustom && (
                      <div>
                        <label className="block text-sm font-medium text-slate-300 mb-1">
                          Key Name
                        </label>
                        <Input
                          value={customKeyName}
                          onChange={(e) => setCustomKeyName(e.target.value.toUpperCase().replace(/[^A-Z0-9_]/g, ''))}
                          placeholder="e.g. VERCEL_TOKEN"
                          className="font-mono"
                        />
                        <p className="text-xs text-slate-500 mt-1">
                          Uppercase letters, digits, and underscores only
                        </p>
                      </div>
                    )}

                    {/* Service description */}
                    {selectedService && (
                      <div className="text-sm text-slate-400 bg-slate-800/50 rounded-lg p-3">
                        <p>{selectedService.description}</p>
                        {selectedService.url && (
                          <a
                            href={selectedService.url}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-stark-400 hover:text-stark-300 mt-1 inline-block"
                          >
                            Get keys &rarr;
                          </a>
                        )}
                      </div>
                    )}

                    {/* Key value inputs for selected service group */}
                    {selectedService && selectedService.keys.map((keyConfig) => (
                      <div key={keyConfig.name}>
                        <label className="block text-sm font-medium text-slate-300 mb-1">
                          {keyConfig.label}
                        </label>
                        <Input
                          type="password"
                          value={keyValues[keyConfig.name] || ''}
                          onChange={(e) => setKeyValues(prev => ({ ...prev, [keyConfig.name]: e.target.value }))}
                          placeholder={keyConfig.name}
                        />
                      </div>
                    ))}

                    {/* Custom key value input */}
                    {isCustom && (
                      <div>
                        <label className="block text-sm font-medium text-slate-300 mb-1">
                          Key Value
                        </label>
                        <Input
                          type="password"
                          value={keyValues['__custom__'] || ''}
                          onChange={(e) => setKeyValues(prev => ({ ...prev, '__custom__': e.target.value }))}
                          placeholder="Enter key value"
                        />
                      </div>
                    )}

                    {/* Actions */}
                    {selectedGroup !== '' && (
                      <div className="flex gap-3 pt-2">
                        <Button
                          onClick={handleSave}
                          isLoading={isSaving}
                          disabled={!canSave}
                        >
                          Save
                        </Button>
                        <Button variant="secondary" onClick={resetForm}>
                          Cancel
                        </Button>
                      </div>
                    )}
                  </div>
                </CardContent>
              </Card>
            </div>
          )}

          {/* Installed Keys List */}
          <Card>
            <CardHeader>
              <CardTitle>Installed Keys</CardTitle>
            </CardHeader>
            <CardContent>
              {keys.length === 0 ? (
                <div className="text-center py-12 text-slate-500">
                  <Key className="w-12 h-12 mx-auto mb-3 opacity-50" />
                  <p className="text-lg">No API keys configured yet</p>
                  <p className="text-sm mt-1">Click "Add API Key" above to get started.</p>
                </div>
              ) : (
                <div className="space-y-2">
                  {keys.map((key) => {
                    const serviceLabel = getServiceLabel(key.key_name, serviceConfigs);
                    const isDeleting = deletingKey === key.key_name;
                    const isCopying = copyingKey === key.key_name;
                    const isCopied = copiedKey === key.key_name;

                    return (
                      <div
                        key={key.key_name}
                        className="flex items-center justify-between p-3 bg-slate-800/50 rounded-lg border border-slate-700/50 hover:border-slate-600/50 transition-colors"
                      >
                        <div className="flex items-center gap-4 min-w-0">
                          <div className="min-w-0">
                            <div className="flex items-center gap-2">
                              <span className="font-mono text-sm text-white font-medium">
                                {key.key_name}
                              </span>
                              {serviceLabel && (
                                <span className="text-xs text-slate-500 bg-slate-700/50 px-2 py-0.5 rounded">
                                  {serviceLabel}
                                </span>
                              )}
                            </div>
                            <div className="flex items-center gap-3 mt-0.5">
                              <span className="font-mono text-xs text-slate-500">
                                {key.key_preview}
                              </span>
                              <span className="flex items-center gap-1 text-xs text-slate-600">
                                <Clock className="w-3 h-3" />
                                {formatTimestamp(key.updated_at)}
                              </span>
                            </div>
                          </div>
                        </div>
                        <div className="flex items-center gap-1">
                          <button
                            onClick={() => handleCopy(key.key_name)}
                            disabled={isCopying}
                            className="text-slate-500 hover:text-slate-300 p-2 rounded transition-colors disabled:opacity-50"
                            title={isCopied ? 'Copied!' : 'Copy key value'}
                          >
                            {isCopying ? (
                              <div className="w-4 h-4 border-2 border-slate-400 border-t-transparent rounded-full animate-spin" />
                            ) : isCopied ? (
                              <Check className="w-4 h-4 text-green-400" />
                            ) : (
                              <ClipboardCopy className="w-4 h-4" />
                            )}
                          </button>
                          <button
                            onClick={() => {
                              if (confirm(`Delete ${key.key_name}?`)) {
                                handleDelete(key.key_name);
                              }
                            }}
                            disabled={isDeleting}
                            className="text-slate-500 hover:text-red-400 p-2 rounded transition-colors disabled:opacity-50"
                            title="Delete key"
                          >
                            {isDeleting ? (
                              <div className="w-4 h-4 border-2 border-red-400 border-t-transparent rounded-full animate-spin" />
                            ) : (
                              <Trash2 className="w-4 h-4" />
                            )}
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </CardContent>
          </Card>
        </div>

        {/* Sidebar hint box */}
        <div className="xl:w-72 shrink-0">
          <div className="sticky top-8 rounded-lg border border-amber-500/30 bg-amber-500/10 p-4">
            <div className="flex items-start gap-3">
              <AlertTriangle className="w-5 h-5 text-amber-400 shrink-0 mt-0.5" />
              <div>
                <p className="text-sm font-medium text-amber-400 mb-2">Warning</p>
                <p className="text-sm text-slate-300 leading-relaxed">
                  When this instance is upgraded or redeployed, API keys will be lost. Use the{' '}
                  <span className="text-amber-300 font-medium">Cloud Backup</span>{' '}
                  feature to preserve the data within your StarkBot instance.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
