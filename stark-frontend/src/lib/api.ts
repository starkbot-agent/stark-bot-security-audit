const API_BASE = '/api';

export async function apiFetch<T>(
  endpoint: string,
  options: RequestInit = {}
): Promise<T> {
  const token = localStorage.getItem('stark_token');

  const headers: HeadersInit = {
    'Content-Type': 'application/json',
    ...options.headers,
  };

  if (token) {
    (headers as Record<string, string>)['Authorization'] = `Bearer ${token}`;
  }

  const response = await fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers,
  });

  if (!response.ok) {
    if (response.status === 401) {
      localStorage.removeItem('stark_token');
      window.location.href = '/';
      throw new Error('Unauthorized');
    }
    const errorText = await response.text();
    throw new Error(errorText || `HTTP ${response.status}`);
  }

  // Handle empty responses
  const text = await response.text();
  if (!text) {
    return {} as T;
  }

  return JSON.parse(text);
}

// Auth API - SIWE (Sign In With Ethereum)
export async function generateChallenge(publicAddress: string): Promise<{ challenge: string }> {
  const response = await fetch(`${API_BASE}/auth/generate_challenge`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ public_address: publicAddress }),
  });

  if (!response.ok) {
    const data = await response.json();
    throw new Error(data.error || 'Failed to generate challenge');
  }

  const data = await response.json();
  if (!data.success || !data.challenge) {
    throw new Error(data.error || 'Failed to generate challenge');
  }

  return { challenge: data.challenge };
}

export async function validateAuth(
  publicAddress: string,
  challenge: string,
  signature: string
): Promise<{ token: string; expires_at: number }> {
  const response = await fetch(`${API_BASE}/auth/validate_auth`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      public_address: publicAddress,
      challenge,
      signature,
    }),
  });

  const data = await response.json();

  if (!response.ok || !data.success) {
    throw new Error(data.error || 'Authentication failed');
  }

  return { token: data.token, expires_at: data.expires_at };
}

export async function validateToken(): Promise<{ valid: boolean }> {
  return apiFetch('/auth/validate');
}

export async function logout(): Promise<void> {
  await apiFetch('/auth/logout', { method: 'POST' });
  localStorage.removeItem('stark_token');
}

// Chat API
export async function sendChatMessage(
  content: string,
  conversationHistory: Array<{ role: string; content: string }>
): Promise<{ response: string }> {
  // Backend expects { messages: [...] } with the full conversation including the new message
  const messages = [
    ...conversationHistory,
    { role: 'user', content }
  ];

  const response = await apiFetch<{ success: boolean; message?: { content: string }; error?: string }>('/chat', {
    method: 'POST',
    body: JSON.stringify({ messages }),
  });

  if (!response.success || !response.message) {
    throw new Error(response.error || 'Failed to get response');
  }

  return { response: response.message.content };
}

// Agent Settings API
export async function getAgentSettings(): Promise<Record<string, unknown>> {
  return apiFetch('/agent-settings');
}

export async function updateAgentSettings(settings: Record<string, unknown>): Promise<void> {
  await apiFetch('/agent-settings', {
    method: 'PUT',
    body: JSON.stringify(settings),
  });
}

// Tools API
interface ToolInfo {
  name: string;
  description: string;
  group: string;
  enabled: boolean;
}

interface ToolsListResponse {
  success: boolean;
  tools?: ToolInfo[];
  error?: string;
}

export async function getTools(): Promise<ToolInfo[]> {
  const response = await apiFetch<ToolsListResponse>('/tools');
  return response.tools || [];
}

export async function updateToolEnabled(name: string, enabled: boolean): Promise<void> {
  await apiFetch(`/tools/${encodeURIComponent(name)}/enabled`, {
    method: 'PUT',
    body: JSON.stringify({ enabled }),
  });
}

// Skills API
export interface SkillInfo {
  name: string;
  description: string;
  version: string;
  source: string;
  enabled: boolean;
  requires_tools: string[];
  requires_binaries: string[];
  tags: string[];
  homepage?: string;
  metadata?: string;
}

export async function getSkills(): Promise<SkillInfo[]> {
  return apiFetch('/skills');
}

export async function uploadSkill(file: File): Promise<void> {
  const token = localStorage.getItem('stark_token');
  const formData = new FormData();
  formData.append('file', file);

  const response = await fetch(`${API_BASE}/skills/upload`, {
    method: 'POST',
    headers: token ? { Authorization: `Bearer ${token}` } : {},
    body: formData,
  });

  if (!response.ok) {
    throw new Error('Failed to upload skill');
  }
}

export async function deleteSkill(id: string): Promise<void> {
  await apiFetch(`/skills/${id}`, { method: 'DELETE' });
}

// Sessions API
export async function getSessions(): Promise<Array<{
  id: number;
  channel_type: string;
  channel_id: number;
  created_at: string;
  updated_at: string;
  message_count?: number;
}>> {
  return apiFetch('/sessions');
}

export async function deleteSession(id: string): Promise<void> {
  await apiFetch(`/sessions/${id}`, { method: 'DELETE' });
}

// Memories API
export async function getMemories(): Promise<Array<{
  id: number;
  content: string;
  importance?: number;
  created_at: string;
}>> {
  return apiFetch('/memories');
}

export async function deleteMemory(id: string): Promise<void> {
  await apiFetch(`/memories/${id}`, { method: 'DELETE' });
}

// Identities API
export async function getIdentities(): Promise<Array<{
  id: string;
  name: string;
  channel_type: string;
  platform_user_id: string;
  created_at: string;
}>> {
  return apiFetch('/identities');
}

// Channels API
export interface ChannelInfo {
  id: number;
  channel_type: string;
  name: string;
  enabled: boolean;
  bot_token: string;
  app_token?: string;
  created_at: string;
  updated_at: string;
  running?: boolean;
}

interface ChannelsListResponse {
  success: boolean;
  channels?: ChannelInfo[];
  error?: string;
}

interface ChannelOperationResponse {
  success: boolean;
  channel?: ChannelInfo;
  error?: string;
}

export async function getChannels(): Promise<ChannelInfo[]> {
  const response = await apiFetch<ChannelsListResponse>('/channels');
  return response.channels || [];
}

export async function getChannel(id: number): Promise<ChannelInfo | null> {
  const response = await apiFetch<ChannelOperationResponse>(`/channels/${id}`);
  return response.channel || null;
}

export async function createChannel(data: {
  channel_type: string;
  name: string;
  bot_token: string;
  app_token?: string;
}): Promise<ChannelInfo> {
  const response = await apiFetch<ChannelOperationResponse>('/channels', {
    method: 'POST',
    body: JSON.stringify(data),
  });
  if (!response.success || !response.channel) {
    throw new Error(response.error || 'Failed to create channel');
  }
  return response.channel;
}

export async function updateChannel(id: number, config: {
  name?: string;
  enabled?: boolean;
  bot_token?: string;
  app_token?: string;
}): Promise<ChannelInfo> {
  const response = await apiFetch<ChannelOperationResponse>(`/channels/${id}`, {
    method: 'PUT',
    body: JSON.stringify(config),
  });
  if (!response.success || !response.channel) {
    throw new Error(response.error || 'Failed to update channel');
  }
  return response.channel;
}

export async function deleteChannel(id: number): Promise<void> {
  const response = await apiFetch<ChannelOperationResponse>(`/channels/${id}`, {
    method: 'DELETE',
  });
  if (!response.success) {
    throw new Error(response.error || 'Failed to delete channel');
  }
}

export async function startChannel(id: number): Promise<ChannelInfo> {
  const response = await apiFetch<ChannelOperationResponse>(`/channels/${id}/start`, {
    method: 'POST',
  });
  if (!response.success || !response.channel) {
    throw new Error(response.error || 'Failed to start channel');
  }
  return response.channel;
}

export async function stopChannel(id: number): Promise<ChannelInfo> {
  const response = await apiFetch<ChannelOperationResponse>(`/channels/${id}/stop`, {
    method: 'POST',
  });
  if (!response.success || !response.channel) {
    throw new Error(response.error || 'Failed to stop channel');
  }
  return response.channel;
}

// Logs API
export async function getLogs(limit?: number): Promise<Array<{
  id: string;
  level: string;
  message: string;
  timestamp: string;
}>> {
  const query = limit ? `?limit=${limit}` : '';
  return apiFetch(`/logs${query}`);
}

// API Keys API
export interface ApiKey {
  service_name: string;
  key_preview: string;
  created_at: string;
  updated_at: string;
}

export interface ApiKeysResponse {
  success: boolean;
  keys?: ApiKey[];
  error?: string;
}

export async function getApiKeys(): Promise<ApiKey[]> {
  const response = await apiFetch<ApiKeysResponse>('/keys');
  return response.keys || [];
}

export async function upsertApiKey(serviceName: string, apiKey: string): Promise<void> {
  await apiFetch('/keys', {
    method: 'POST',
    body: JSON.stringify({ service_name: serviceName, api_key: apiKey }),
  });
}

export async function deleteApiKey(serviceName: string): Promise<void> {
  await apiFetch('/keys', {
    method: 'DELETE',
    body: JSON.stringify({ service_name: serviceName }),
  });
}
