import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import { FinePolicyFields } from './FinePolicyFields.tsx';

async function i18nFor(lng: 'fr' | 'en'): Promise<I18nInstance> {
  const i18n = createInstance();
  await i18n.init({
    lng,
    resources: {
      fr: { translation: fr },
      en: { translation: en },
    },
    interpolation: { escapeValue: false },
  });
  return i18n;
}

function render(i18n: I18nInstance, props: Parameters<typeof FinePolicyFields>[0]): string {
  return renderToStaticMarkup(
    createElement(I18nextProvider, { i18n }, createElement(FinePolicyFields, props)) as ReactElement,
  );
}

const base = {
  dailyRate: '0.20',
  blockMode: 'amount' as const,
  threshold: '5.00',
  graceDays: 3,
  maxAmount: '15.00',
  hasMediaRules: false,
  onEnabledChange: () => undefined,
  onDailyRateChange: () => undefined,
  onBlockModeChange: () => undefined,
  onThresholdChange: () => undefined,
};

describe('FinePolicyFields', () => {
  it('hides the rate and the threshold when fines are off', async () => {
    const html = render(await i18nFor('fr'), {
      ...base,
      showToggle: true,
      enabled: false,
    });
    assert.match(html, /role="switch"/);
    assert.match(html, /aria-checked="false"/);
    assert.match(html, /lecteur/);
    assert.match(html, /exemplaire/);
    assert.match(html, /perdu/);
    assert.match(html, new RegExp(fr.settings.finePolicy.blockingStillActive));
    assert.equal(html.includes('type="number"'), false);
    assert.equal(html.includes('type="radio"'), false);
    assert.equal(html.includes('id="fine-daily-rate"'), false);
  });

  it('shows the daily rate then the blocking threshold when fines are on', async () => {
    const html = render(await i18nFor('en'), {
      ...base,
      showToggle: true,
      enabled: true,
      blockMode: 'none',
      threshold: '',
    });
    assert.match(html, /aria-checked="true"/);
    assert.match(html, /id="fine-daily-rate"/);
    assert.match(html, /value="0.20"/);
    assert.equal((html.match(/type="radio"/g) ?? []).length, 2);
    assert.equal(html.includes('type="number"'), true);
    const rateAt = html.indexOf('id="fine-daily-rate"');
    const radioAt = html.indexOf('type="radio"');
    assert.equal(rateAt > 0 && radioAt > rateAt, true);
    assert.match(html, /15\.00|15,00/);
    assert.match(html, /€/);
  });

  it('keeps amount fields visible when this account cannot read late-fine activation', async () => {
    const html = render(await i18nFor('fr'), {
      ...base,
      showToggle: false,
      showAdminNote: true,
      enabled: true,
      blockMode: 'none',
      threshold: '',
    });
    assert.equal(html.includes('role="switch"'), false);
    assert.match(html, new RegExp(fr.settings.finePolicy.adminOnly));
    assert.match(html, /id="fine-daily-rate"/);
    assert.equal((html.match(/type="radio"/g) ?? []).length, 2);
  });
});
