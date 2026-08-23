import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Plus, Save, Trash2, Zap } from 'lucide-react';
import { Button, Card, CardHeader, Input } from '@/components/common';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import { formControlClass } from '@/utils/formControl';
import type { CreateLlmProviderRequest, LlmProviderAdmin, LlmProviderKind } from '@/types';

const LLM_QUERY_KEY = ['admin', 'llm-providers'];

const KIND_DEFAULTS: Record<LlmProviderKind, { baseUrl: string; models: string[] }> = {
  openai: { baseUrl: 'https://api.openai.com/v1', models: ['gpt-4o-mini'] },
  ollama: { baseUrl: 'http://127.0.0.1:11434/v1', models: ['llama3.2'] },
  gemini: {
    baseUrl: 'https://generativelanguage.googleapis.com/v1beta/openai',
    models: ['gemini-2.5-flash'],
  },
  grok: { baseUrl: 'https://api.x.ai/v1', models: ['grok-2-latest'] },
  anthropic: { baseUrl: 'https://api.anthropic.com/v1', models: ['claude-sonnet-4-20250514'] },
  openaiCompat: { baseUrl: 'http://127.0.0.1:11434/v1', models: ['llama3.2'] },
};

function emptyDraft(): CreateLlmProviderRequest {
  const defaults = KIND_DEFAULTS.ollama;
  return {
    slug: '',
    label: '',
    kind: 'ollama',
    baseUrl: defaults.baseUrl,
    models: defaults.models,
    defaultModel: defaults.models[0],
    enabled: true,
    sortOrder: 0,
  };
}

export default function LlmProvidersSettings() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery({
    queryKey: LLM_QUERY_KEY,
    queryFn: () => api.getLlmProvidersAdmin(),
  });

  const [rows, setRows] = useState<LlmProviderAdmin[]>([]);
  const [draft, setDraft] = useState<CreateLlmProviderRequest | null>(null);
  const [apiKeys, setApiKeys] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (data) setRows(data);
  }, [data]);

  const refresh = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: LLM_QUERY_KEY }),
      queryClient.invalidateQueries({ queryKey: ['chat', 'providers'] }),
    ]);
  }, [queryClient]);

  const saveNew = async () => {
    if (!draft) return;
    setBusy('new');
    setMessage(null);
    try {
      await api.createLlmProvider({
        ...draft,
        models: draft.models.filter(Boolean),
        apiKey: draft.apiKey?.trim() || undefined,
      });
      setDraft(null);
      await refresh();
      setMessage(t('chat.admin.saved'));
    } catch (e) {
      setMessage(getApiErrorMessage(e, t));
    } finally {
      setBusy(null);
    }
  };

  const saveRow = async (row: LlmProviderAdmin) => {
    setBusy(row.id);
    setMessage(null);
    try {
      const key = apiKeys[row.id];
      await api.updateLlmProvider(row.id, {
        slug: row.slug,
        label: row.label,
        kind: row.kind,
        baseUrl: row.baseUrl,
        apiKeyEnv: row.apiKeyEnv ?? undefined,
        models: row.models,
        defaultModel: row.defaultModel ?? undefined,
        enabled: row.enabled,
        sortOrder: row.sortOrder,
        ...(key !== undefined ? { apiKey: key } : {}),
      });
      setApiKeys((prev) => {
        const next = { ...prev };
        delete next[row.id];
        return next;
      });
      await refresh();
      setMessage(t('chat.admin.saved'));
    } catch (e) {
      setMessage(getApiErrorMessage(e, t));
    } finally {
      setBusy(null);
    }
  };

  const removeRow = async (id: string) => {
    setBusy(id);
    try {
      await api.deleteLlmProvider(id);
      await refresh();
    } catch (e) {
      setMessage(getApiErrorMessage(e, t));
    } finally {
      setBusy(null);
    }
  };

  const testRow = async (id: string) => {
    setBusy(`test-${id}`);
    setMessage(null);
    try {
      await api.testLlmProvider(id);
      setMessage(t('chat.admin.testOk'));
    } catch (e) {
      setMessage(getApiErrorMessage(e, t));
    } finally {
      setBusy(null);
    }
  };

  const updateRow = (id: string, patch: Partial<LlmProviderAdmin>) => {
    setRows((prev) => prev.map((r) => (r.id === id ? { ...r, ...patch } : r)));
  };

  if (isLoading) {
    return <p className="text-sm text-gray-500">{t('common.loading')}</p>;
  }

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader
          title={t('chat.admin.title')}
          action={
            <Button type="button" size="sm" variant="secondary" onClick={() => setDraft(emptyDraft())}>
              <Plus className="h-4 w-4 mr-1" />
              {t('chat.admin.add')}
            </Button>
          }
        />
        {message && <p className="px-4 pb-2 text-sm text-amber-700 dark:text-amber-300">{message}</p>}

        {draft && (
          <div className="p-4 border-t border-gray-200 dark:border-gray-800 space-y-3">
            <h3 className="font-medium text-sm">{t('chat.admin.newProvider')}</h3>
            <ProviderForm
              value={draft}
              onChange={(patch) => setDraft((d) => (d ? { ...d, ...patch } : d))}
              showApiKey
            />
            <div className="flex gap-2">
              <Button type="button" variant="primary" disabled={busy === 'new'} onClick={() => void saveNew()}>
                <Save className="h-4 w-4 mr-1" />
                {t('common.save')}
              </Button>
              <Button type="button" variant="secondary" onClick={() => setDraft(null)}>
                {t('common.cancel')}
              </Button>
            </div>
          </div>
        )}

        <div className="divide-y divide-gray-200 dark:divide-gray-800">
          {rows.map((row) => (
            <div key={row.id} className="p-4 space-y-3">
              <ProviderForm
                value={row}
                onChange={(patch) => updateRow(row.id, patch)}
                apiKeySet={row.apiKeySet}
                apiKeyValue={apiKeys[row.id] ?? ''}
                onApiKeyChange={(v) => setApiKeys((p) => ({ ...p, [row.id]: v }))}
              />
              <div className="flex flex-wrap gap-2">
                <Button type="button" size="sm" variant="primary" disabled={busy === row.id} onClick={() => void saveRow(row)}>
                  <Save className="h-4 w-4 mr-1" />
                  {t('common.save')}
                </Button>
                <Button type="button" size="sm" variant="secondary" disabled={busy === `test-${row.id}`} onClick={() => void testRow(row.id)}>
                  <Zap className="h-4 w-4 mr-1" />
                  {t('chat.admin.test')}
                </Button>
                <Button type="button" size="sm" variant="secondary" onClick={() => void removeRow(row.id)}>
                  <Trash2 className="h-4 w-4 mr-1" />
                  {t('common.delete')}
                </Button>
              </div>
            </div>
          ))}
        </div>
      </Card>
    </div>
  );
}

function ProviderForm({
  value,
  onChange,
  showApiKey,
  apiKeySet,
  apiKeyValue,
  onApiKeyChange,
}: {
  value: {
    slug: string;
    label: string;
    kind: LlmProviderKind;
    baseUrl: string;
    models: string[];
    defaultModel?: string | null;
    enabled?: boolean;
    apiKeyEnv?: string | null;
    sortOrder?: number;
    apiKey?: string;
  };
  onChange: (patch: Record<string, unknown>) => void;
  showApiKey?: boolean;
  apiKeySet?: boolean;
  apiKeyValue?: string;
  onApiKeyChange?: (v: string) => void;
}) {
  const { t } = useTranslation();
  const modelsStr = value.models.join(', ');

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
      <Input label={t('chat.admin.slug')} value={value.slug} onChange={(e) => onChange({ slug: e.target.value })} />
      <Input label={t('common.name')} value={value.label} onChange={(e) => onChange({ label: e.target.value })} />
      <div>
        <label className="block text-sm font-medium mb-1">{t('chat.admin.kind')}</label>
        <select
          className={formControlClass()}
          value={value.kind}
          onChange={(e) => {
            const kind = e.target.value as LlmProviderKind;
            const defaults = KIND_DEFAULTS[kind];
            onChange({
              kind,
              baseUrl: defaults.baseUrl,
              models: defaults.models,
              defaultModel: defaults.models[0],
            });
          }}
        >
          <option value="openai">OpenAI</option>
          <option value="ollama">Ollama</option>
          <option value="gemini">Gemini</option>
          <option value="grok">Grok (xAI)</option>
          <option value="anthropic">Anthropic</option>
          <option value="openaiCompat">OpenAI-compatible (custom)</option>
        </select>
      </div>
      <Input label={t('chat.admin.baseUrl')} value={value.baseUrl} onChange={(e) => onChange({ baseUrl: e.target.value })} />
      <Input
        label={t('chat.admin.models')}
        value={modelsStr}
        onChange={(e) => onChange({ models: e.target.value.split(',').map((s) => s.trim()).filter(Boolean) })}
      />
      <Input
        label={t('chat.admin.defaultModel')}
        value={value.defaultModel ?? ''}
        onChange={(e) => onChange({ defaultModel: e.target.value })}
      />
      <Input
        label={t('chat.admin.apiKeyEnv')}
        value={value.apiKeyEnv ?? ''}
        onChange={(e) => onChange({ apiKeyEnv: e.target.value })}
      />
      {(showApiKey || onApiKeyChange) && (
        <Input
          label={apiKeySet ? t('chat.admin.apiKeyReplace') : t('chat.admin.apiKey')}
          type="password"
          value={showApiKey ? (value.apiKey ?? '') : (apiKeyValue ?? '')}
          onChange={(e) => {
            if (showApiKey) onChange({ apiKey: e.target.value });
            else onApiKeyChange?.(e.target.value);
          }}
        />
      )}
      <label className="flex items-center gap-2 text-sm md:col-span-2">
        <input
          type="checkbox"
          checked={value.enabled !== false}
          onChange={(e) => onChange({ enabled: e.target.checked })}
        />
        {t('chat.admin.enabled')}
      </label>
    </div>
  );
}
