import { useCallback, useEffect, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { MessageSquare, Plus, Send, StopCircle, Trash2, Wrench } from 'lucide-react';
import { Button, Card, Input } from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import { isAdmin, isLibrarian } from '@/types';
import type { ChatMessage, ChatStreamEvent, LlmProviderPublic } from '@/types';

function defaultModel(provider: LlmProviderPublic): string {
  return provider.defaultModel ?? provider.models[0] ?? '';
}

export default function ChatPage() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const staff = isLibrarian(user?.accountType);
  const admin = isAdmin(user?.accountType);

  const {
    data: providers = [],
    isLoading: providersLoading,
    isError: providersError,
    refetch: refetchProviders,
  } = useQuery({
    queryKey: ['chat', 'providers'],
    queryFn: () => api.getChatProviders(),
  });

  const { data: conversations = [], refetch: refetchConversations } = useQuery({
    queryKey: ['chat', 'conversations'],
    queryFn: () => api.getChatConversations(),
  });

  const [activeId, setActiveId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState('');
  const [streaming, setStreaming] = useState(false);
  const [streamText, setStreamText] = useState('');
  const [toolEvents, setToolEvents] = useState<ChatStreamEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [providerId, setProviderId] = useState('');
  const [model, setModel] = useState('');
  const abortRef = useRef<AbortController | null>(null);
  const bottomRef = useRef<HTMLDivElement | null>(null);

  const selectedProvider = providers.find((p) => p.id === providerId) ?? providers[0];

  useEffect(() => {
    if (providers.length && !providerId) {
      setProviderId(providers[0].id);
      setModel(defaultModel(providers[0]));
    }
  }, [providers, providerId]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, streamText, toolEvents]);

  const loadConversation = useCallback(async (id: string) => {
    setActiveId(id);
    setError(null);
    const detail = await api.getChatConversation(id);
    setMessages(detail.messages);
    setProviderId(detail.providerId);
    setModel(detail.model);
  }, []);

  const startNewConversation = useCallback(async () => {
    if (!selectedProvider) {
      setError(t('chat.noProviders'));
      return;
    }
    setError(null);
    const conv = await api.createChatConversation({
      providerId: selectedProvider.id,
      model: model || defaultModel(selectedProvider),
    });
    await refetchConversations();
    setActiveId(conv.id);
    setMessages([]);
    setStreamText('');
    setToolEvents([]);
  }, [selectedProvider, model, refetchConversations, t]);

  const deleteConversation = useCallback(async (id: string) => {
    await api.deleteChatConversation(id);
    if (activeId === id) {
      setActiveId(null);
      setMessages([]);
    }
    await refetchConversations();
  }, [activeId, refetchConversations]);

  const stopGeneration = useCallback(async () => {
    abortRef.current?.abort();
    if (activeId) {
      try {
        await api.cancelChatGeneration(activeId);
      } catch {
        // best effort
      }
    }
    setStreaming(false);
  }, [activeId]);

  const sendMessage = useCallback(async () => {
    const text = draft.trim();
    if (!text || streaming) return;

    let convId = activeId;
    if (!convId) {
      if (!selectedProvider) {
        setError(t('chat.noProviders'));
        return;
      }
      const conv = await api.createChatConversation({
        providerId: selectedProvider.id,
        model: model || defaultModel(selectedProvider),
      });
      convId = conv.id;
      setActiveId(convId);
      await refetchConversations();
    }

    setDraft('');
    setStreaming(true);
    setStreamText('');
    setToolEvents([]);
    setError(null);

    const userMsg: ChatMessage = {
      id: `local-${Date.now()}`,
      conversationId: convId,
      role: 'user',
      content: text,
      createdAt: new Date().toISOString(),
    };
    setMessages((prev) => [...prev, userMsg]);

    const controller = new AbortController();
    abortRef.current = controller;

    try {
      for await (const ev of api.streamChatMessage(
        convId,
        { content: text, providerId, model },
        controller.signal,
      )) {
        if (ev.type === 'delta') {
          setStreamText((s) => s + ev.text);
        } else if (ev.type === 'tool.start' || ev.type === 'tool.result') {
          setToolEvents((prev) => [...prev, ev]);
        } else if (ev.type === 'error') {
          setError(ev.message);
        } else if (ev.type === 'done') {
          setStreamText('');
          const detail = await api.getChatConversation(convId);
          setMessages(detail.messages);
          await refetchConversations();
        }
      }
    } catch (e) {
      if (!controller.signal.aborted) {
        setError(getApiErrorMessage(e, t));
      }
    } finally {
      setStreaming(false);
      abortRef.current = null;
    }
  }, [
    activeId,
    draft,
    streaming,
    selectedProvider,
    model,
    providerId,
    refetchConversations,
    t,
  ]);

  return (
    <div className="flex flex-col lg:flex-row gap-4 h-[calc(100vh-8rem)] min-h-[420px]">
      <Card className="lg:w-72 shrink-0 flex flex-col overflow-hidden p-0">
        <div className="p-3 border-b border-gray-200 dark:border-gray-800 flex items-center justify-between gap-2">
          <h1 className="text-sm font-semibold flex items-center gap-2">
            <MessageSquare className="h-4 w-4" />
            {t('nav.chat')}
          </h1>
          <Button type="button" size="sm" variant="secondary" onClick={() => void startNewConversation()}>
            <Plus className="h-4 w-4" />
          </Button>
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {conversations.length === 0 && (
            <p className="text-xs text-gray-500 dark:text-gray-400 px-2 py-4">{t('chat.emptyConversations')}</p>
          )}
          {conversations.map((c) => (
            <div
              key={c.id}
              className={`group flex items-center gap-1 rounded-lg ${
                activeId === c.id
                  ? 'bg-amber-50 dark:bg-amber-900/30'
                  : 'hover:bg-gray-100 dark:hover:bg-gray-800'
              }`}
            >
              <button
                type="button"
                className="flex-1 text-left px-3 py-2 text-sm truncate"
                onClick={() => void loadConversation(c.id)}
              >
                {c.title}
              </button>
              <button
                type="button"
                className="p-2 opacity-0 group-hover:opacity-100 text-gray-400 hover:text-red-500"
                aria-label={t('common.delete')}
                onClick={() => void deleteConversation(c.id)}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            </div>
          ))}
        </div>
      </Card>

      <Card className="flex-1 flex flex-col overflow-hidden p-0 min-w-0">
        <div className="p-3 border-b border-gray-200 dark:border-gray-800 flex flex-wrap items-end gap-3">
          <div className="min-w-[160px]">
            <label className="block text-xs font-medium mb-1">{t('chat.provider')}</label>
            <select
              className="w-full rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-900 px-2 py-1.5 text-sm"
              value={providerId}
              disabled={streaming || providersLoading || providers.length === 0}
              onChange={(e) => {
                const id = e.target.value;
                setProviderId(id);
                const p = providers.find((x) => x.id === id);
                if (p) setModel(defaultModel(p));
              }}
            >
              {providers.length === 0 && (
                <option value="">{providersLoading ? t('common.loading') : t('chat.noProvidersShort')}</option>
              )}
              {providers.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                </option>
              ))}
            </select>
          </div>
          <div className="min-w-[160px] flex-1">
            <label className="block text-xs font-medium mb-1">{t('chat.model')}</label>
            <select
              className="w-full rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-900 px-2 py-1.5 text-sm"
              value={model}
              disabled={streaming || providersLoading || !selectedProvider}
              onChange={(e) => setModel(e.target.value)}
            >
              {(selectedProvider?.models ?? []).map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </div>
        </div>

        {providersError && (
          <div className="px-4 py-2 text-sm text-red-600 dark:text-red-400 flex items-center gap-2">
            <span>{t('chat.providersLoadError')}</span>
            <Button type="button" size="sm" variant="secondary" onClick={() => void refetchProviders()}>
              {t('common.retry')}
            </Button>
          </div>
        )}

        {!providersLoading && !providersError && providers.length === 0 && (
          <div className="px-4 py-2 text-sm text-amber-700 dark:text-amber-300 bg-amber-50 dark:bg-amber-900/20 border-b border-amber-200 dark:border-amber-800">
            {admin ? (
              <>
                {t('chat.noProvidersAdmin')}{' '}
                <Link to="/settings?tab=llmProviders" className="underline font-medium">
                  {t('chat.admin.title')}
                </Link>
              </>
            ) : (
              t('chat.noProviders')
            )}
          </div>
        )}

        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {messages.length === 0 && !streamText && (
            <p className="text-sm text-gray-500 dark:text-gray-400">{t('chat.intro')}</p>
          )}
          {messages.map((m) => (
            <div
              key={m.id}
              className={`max-w-[85%] rounded-xl px-3 py-2 text-sm whitespace-pre-wrap ${
                m.role === 'user'
                  ? 'ml-auto bg-indigo-600 text-white'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100'
              }`}
            >
              {m.content}
            </div>
          ))}
          {toolEvents.length > 0 && (
            <div className="text-xs text-gray-600 dark:text-gray-400 space-y-1">
              {toolEvents.map((ev, i) => {
                if (ev.type === 'tool.start') {
                  return (
                    <div key={`${ev.id}-${i}`} className="flex items-center gap-1">
                      <Wrench className="h-3 w-3" />
                      {t('chat.toolRunning', { name: ev.name })}
                    </div>
                  );
                }
                if (ev.type === 'tool.result') {
                  return (
                    <details key={`${ev.id}-${i}`} className="rounded border border-gray-200 dark:border-gray-700 p-2">
                      <summary>
                        {ev.ok ? '✓' : '✗'} {ev.name}: {ev.summary}
                      </summary>
                      {staff && ev.detail && <pre className="mt-1 overflow-x-auto text-[11px]">{ev.detail}</pre>}
                      {!staff && !ev.ok && <p className="mt-1">{ev.summary}</p>}
                    </details>
                  );
                }
                return null;
              })}
            </div>
          )}
          {streamText && (
            <div className="max-w-[85%] rounded-xl px-3 py-2 text-sm bg-gray-100 dark:bg-gray-800 whitespace-pre-wrap">
              {streamText}
            </div>
          )}
          <div ref={bottomRef} />
        </div>

        {error && (
          <p className="px-4 pb-2 text-sm text-red-600 dark:text-red-400">{error}</p>
        )}

        <div className="p-3 border-t border-gray-200 dark:border-gray-800 flex gap-2">
          <Input
            className="flex-1"
            placeholder={t('chat.placeholder')}
            value={draft}
            disabled={streaming || providersLoading || providers.length === 0}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                void sendMessage();
              }
            }}
          />
          {streaming ? (
            <Button type="button" variant="secondary" onClick={() => void stopGeneration()}>
              <StopCircle className="h-4 w-4" />
            </Button>
          ) : (
            <Button type="button" variant="primary" disabled={!draft.trim() || providersLoading || providers.length === 0} onClick={() => void sendMessage()}>
              <Send className="h-4 w-4" />
            </Button>
          )}
        </div>
      </Card>
    </div>
  );
}
