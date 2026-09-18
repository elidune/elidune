import type { EmailTemplateListItem, RemindersConfig, ReminderTierId } from '@/types';
import type { SupportedLanguage } from '@/locales';
import { emailTemplateDisplayName } from '@/utils/emailTemplatesConstants';

export const MIN_REMINDER_DELAY_DAYS = 0;
export const MAX_REMINDER_DELAY_DAYS = 3650;

export const DEFAULT_REMINDERS_CONFIG: RemindersConfig = {
  enabled: true,
  frequencyDays: 7,
  firstReminderDelayDays: 7,
  secondReminderDelayDays: 14,
  formalNoticeDelayDays: 21,
  firstReminderTemplate: 'overdue_reminder',
  secondReminderTemplate: 'overdue_second_reminder',
  formalNoticeTemplate: 'overdue_formal_notice',
  firstReminderEnabled: true,
  secondReminderEnabled: true,
  formalNoticeEnabled: true,
  sendTime: '09:00',
  accrueFines: true,
  smtpThrottleMs: 100,
};

export type ReminderTiersDraft = {
  firstEnabled: boolean;
  secondEnabled: boolean;
  formalNoticeEnabled: boolean;
  firstDelayDays: string;
  secondDelayDays: string;
  formalNoticeDelayDays: string;
  firstTemplate: string;
  secondTemplate: string;
  formalNoticeTemplate: string;
};

export const REMINDER_TIER_ORDER: ReminderTierId[] = ['first', 'second', 'formalNotice'];

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' ? (value as Record<string, unknown>) : {};
}

function pickBool(o: Record<string, unknown>, snake: string, camel: string, fallback: boolean): boolean {
  const v = o[snake] ?? o[camel];
  return typeof v === 'boolean' ? v : fallback;
}

function pickNum(o: Record<string, unknown>, snake: string, camel: string, fallback: number): number {
  const v = o[snake] ?? o[camel];
  return typeof v === 'number' && Number.isFinite(v) ? v : fallback;
}

function pickStr(o: Record<string, unknown>, snake: string, camel: string, fallback: string): string {
  const v = o[snake] ?? o[camel];
  return typeof v === 'string' && v.trim() ? v : fallback;
}

/** Parse GET /admin/config reminders.value (snake_case on the wire). */
export function parseRemindersConfig(value: unknown): RemindersConfig {
  const o = asRecord(value);
  const d = DEFAULT_REMINDERS_CONFIG;
  const parsed: RemindersConfig = {
    enabled: pickBool(o, 'enabled', 'enabled', d.enabled),
    frequencyDays: Math.max(1, Math.trunc(pickNum(o, 'frequency_days', 'frequencyDays', d.frequencyDays))),
    firstReminderDelayDays: pickNum(
      o,
      'first_reminder_delay_days',
      'firstReminderDelayDays',
      d.firstReminderDelayDays,
    ),
    secondReminderDelayDays: pickNum(
      o,
      'second_reminder_delay_days',
      'secondReminderDelayDays',
      d.secondReminderDelayDays,
    ),
    formalNoticeDelayDays: pickNum(
      o,
      'formal_notice_delay_days',
      'formalNoticeDelayDays',
      d.formalNoticeDelayDays,
    ),
    firstReminderTemplate: pickStr(
      o,
      'first_reminder_template',
      'firstReminderTemplate',
      d.firstReminderTemplate,
    ),
    secondReminderTemplate: pickStr(
      o,
      'second_reminder_template',
      'secondReminderTemplate',
      d.secondReminderTemplate,
    ),
    formalNoticeTemplate: pickStr(
      o,
      'formal_notice_template',
      'formalNoticeTemplate',
      d.formalNoticeTemplate,
    ),
    firstReminderEnabled: pickBool(o, 'first_reminder_enabled', 'firstReminderEnabled', d.firstReminderEnabled),
    secondReminderEnabled: pickBool(
      o,
      'second_reminder_enabled',
      'secondReminderEnabled',
      d.secondReminderEnabled,
    ),
    formalNoticeEnabled: pickBool(o, 'formal_notice_enabled', 'formalNoticeEnabled', d.formalNoticeEnabled),
    sendTime: pickStr(o, 'send_time', 'sendTime', d.sendTime),
    accrueFines: pickBool(o, 'accrue_fines', 'accrueFines', d.accrueFines),
    smtpThrottleMs: pickNum(o, 'smtp_throttle_ms', 'smtpThrottleMs', d.smtpThrottleMs),
  };
  return normalizeRemindersChain(parsed);
}

/** Server PUT body uses snake_case field names. */
export function remindersConfigToPayload(cfg: RemindersConfig): Record<string, unknown> {
  return {
    enabled: cfg.enabled,
    frequency_days: cfg.frequencyDays,
    first_reminder_delay_days: cfg.firstReminderDelayDays,
    second_reminder_delay_days: cfg.secondReminderDelayDays,
    formal_notice_delay_days: cfg.formalNoticeDelayDays,
    first_reminder_template: cfg.firstReminderTemplate,
    second_reminder_template: cfg.secondReminderTemplate,
    formal_notice_template: cfg.formalNoticeTemplate,
    first_reminder_enabled: cfg.firstReminderEnabled,
    second_reminder_enabled: cfg.secondReminderEnabled,
    formal_notice_enabled: cfg.formalNoticeEnabled,
    send_time: cfg.sendTime,
    accrue_fines: cfg.accrueFines,
    smtp_throttle_ms: cfg.smtpThrottleMs,
  };
}

export function remindersConfigToDraft(cfg: RemindersConfig): ReminderTiersDraft {
  return {
    firstEnabled: cfg.firstReminderEnabled,
    secondEnabled: cfg.secondReminderEnabled,
    formalNoticeEnabled: cfg.formalNoticeEnabled,
    firstDelayDays: String(cfg.firstReminderDelayDays),
    secondDelayDays: String(cfg.secondReminderDelayDays),
    formalNoticeDelayDays: String(cfg.formalNoticeDelayDays),
    firstTemplate: cfg.firstReminderTemplate,
    secondTemplate: cfg.secondReminderTemplate,
    formalNoticeTemplate: cfg.formalNoticeTemplate,
  };
}

export function normalizeReminderTiersDraft(draft: ReminderTiersDraft): ReminderTiersDraft {
  if (!draft.firstEnabled) {
    return { ...draft, secondEnabled: false, formalNoticeEnabled: false };
  }
  if (!draft.secondEnabled) {
    return { ...draft, formalNoticeEnabled: false };
  }
  return draft;
}

export function normalizeRemindersChain(cfg: RemindersConfig): RemindersConfig {
  if (!cfg.firstReminderEnabled) {
    return { ...cfg, secondReminderEnabled: false, formalNoticeEnabled: false };
  }
  if (!cfg.secondReminderEnabled) {
    return { ...cfg, formalNoticeEnabled: false };
  }
  return cfg;
}

export function applyReminderTierEnabled(
  draft: ReminderTiersDraft,
  tier: ReminderTierId,
  enabled: boolean,
): ReminderTiersDraft {
  if (tier === 'first') {
    if (!enabled) {
      return { ...draft, firstEnabled: false, secondEnabled: false, formalNoticeEnabled: false };
    }
    return { ...draft, firstEnabled: true };
  }
  if (tier === 'second') {
    if (!draft.firstEnabled) {
      return { ...draft, secondEnabled: false, formalNoticeEnabled: false };
    }
    if (!enabled) {
      return { ...draft, secondEnabled: false, formalNoticeEnabled: false };
    }
    return { ...draft, secondEnabled: true };
  }
  if (!draft.firstEnabled || !draft.secondEnabled) {
    return { ...draft, formalNoticeEnabled: false };
  }
  return { ...draft, formalNoticeEnabled: enabled };
}

export function isReminderTierSwitchDisabled(draft: ReminderTiersDraft, tier: ReminderTierId): boolean {
  if (tier === 'first') return false;
  if (tier === 'second') return !draft.firstEnabled;
  return !draft.firstEnabled || !draft.secondEnabled;
}

export type ParsedReminderDelay = { ok: true; value: number } | { ok: false };

export function parseReminderDelayDays(raw: string): ParsedReminderDelay {
  const trimmed = raw.trim();
  if (!/^\d+$/.test(trimmed)) return { ok: false };
  const n = Number(trimmed);
  if (!Number.isInteger(n) || n < MIN_REMINDER_DELAY_DAYS || n > MAX_REMINDER_DELAY_DAYS) {
    return { ok: false };
  }
  return { ok: true, value: n };
}

export type ReminderTiersParseResult =
  | { ok: true; cfg: Pick<
      RemindersConfig,
      | 'firstReminderEnabled'
      | 'secondReminderEnabled'
      | 'formalNoticeEnabled'
      | 'firstReminderDelayDays'
      | 'secondReminderDelayDays'
      | 'formalNoticeDelayDays'
      | 'firstReminderTemplate'
      | 'secondReminderTemplate'
      | 'formalNoticeTemplate'
    > }
  | { ok: false; field: 'delay' | 'template' };

export function parseReminderTiersDraft(draft: ReminderTiersDraft): ReminderTiersParseResult {
  const normalized = normalizeReminderTiersDraft(draft);
  const firstDelay = parseReminderDelayDays(normalized.firstDelayDays);
  const secondDelay = parseReminderDelayDays(normalized.secondDelayDays);
  const formalDelay = parseReminderDelayDays(normalized.formalNoticeDelayDays);
  if (!firstDelay.ok || !secondDelay.ok || !formalDelay.ok) {
    return { ok: false, field: 'delay' };
  }
  const firstTemplate = normalized.firstTemplate.trim();
  const secondTemplate = normalized.secondTemplate.trim();
  const formalTemplate = normalized.formalNoticeTemplate.trim();
  if (!firstTemplate || !secondTemplate || !formalTemplate) {
    return { ok: false, field: 'template' };
  }
  return {
    ok: true,
    cfg: {
      firstReminderEnabled: normalized.firstEnabled,
      secondReminderEnabled: normalized.secondEnabled,
      formalNoticeEnabled: normalized.formalNoticeEnabled,
      firstReminderDelayDays: firstDelay.value,
      secondReminderDelayDays: secondDelay.value,
      formalNoticeDelayDays: formalDelay.value,
      firstReminderTemplate: firstTemplate,
      secondReminderTemplate: secondTemplate,
      formalNoticeTemplate: formalTemplate,
    },
  };
}

/** Merge tier fields into an existing admin-config payload without dropping scheduler keys. */
export function mergeReminderTiersIntoPayload(
  existing: Record<string, unknown>,
  cfg: RemindersConfig,
): Record<string, unknown> {
  const payload = {
    ...existing,
    ...remindersConfigToPayload(cfg),
  };
  delete payload.overridable;
  return payload;
}

export function applyDraftToRemindersConfig(base: RemindersConfig, draft: ReminderTiersDraft): RemindersConfig | null {
  const parsed = parseReminderTiersDraft(draft);
  if (!parsed.ok) return null;
  return normalizeRemindersChain({ ...base, ...parsed.cfg });
}

/**
 * Next tier to send from `loans.reminderCount` (0/1/2 → first/second/formal notice).
 * `null` when every tier has already been sent.
 */
export function nextReminderTierFromCount(reminderCount: number): ReminderTierId | null {
  if (reminderCount <= 0) return 'first';
  if (reminderCount === 1) return 'second';
  if (reminderCount === 2) return 'formalNotice';
  return null;
}

export function emailTemplateSelectOptions(
  rows: EmailTemplateListItem[],
  uiLanguage: SupportedLanguage,
  extraIds: string[] = [],
): { id: string; label: string }[] {
  const grouped = new Map<string, EmailTemplateListItem[]>();
  for (const row of rows) {
    const arr = grouped.get(row.templateId) ?? [];
    arr.push(row);
    grouped.set(row.templateId, arr);
  }
  for (const id of extraIds) {
    if (id && !grouped.has(id)) grouped.set(id, []);
  }
  return [...grouped.keys()]
    .sort((a, b) => {
      const nameA = emailTemplateDisplayName(grouped.get(a) ?? [], uiLanguage) || a;
      const nameB = emailTemplateDisplayName(grouped.get(b) ?? [], uiLanguage) || b;
      return nameA.localeCompare(nameB, uiLanguage);
    })
    .map((id) => ({
      id,
      label: emailTemplateDisplayName(grouped.get(id) ?? [], uiLanguage) || id,
    }));
}
