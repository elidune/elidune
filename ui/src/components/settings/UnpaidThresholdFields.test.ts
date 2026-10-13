import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../../locales/de/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import es from '../../locales/es/translation.json' with { type: 'json' };
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import { UnpaidThresholdFields } from './UnpaidThresholdFields.tsx';

async function i18nFor(lng: 'fr' | 'en' | 'de' | 'es'): Promise<I18nInstance> {
  const i18n = createInstance();
  await i18n.init({
    lng,
    resources: {
      fr: { translation: fr },
      en: { translation: en },
      de: { translation: de },
      es: { translation: es },
    },
    interpolation: { escapeValue: false },
  });
  return i18n;
}

function renderFields(
  i18n: I18nInstance,
  props: Parameters<typeof UnpaidThresholdFields>[0],
): string {
  return renderToStaticMarkup(
    createElement(I18nextProvider, { i18n }, createElement(UnpaidThresholdFields, props)) as ReactElement,
  );
}

describe('UnpaidThresholdFields', () => {
  it('offers none and amount as a labelled radio group on the global policy', async () => {
    const html = renderFields(await i18nFor('en'), {
      scope: 'global',
      name: 'global-unpaid',
      mode: 'none',
      amount: '',
      onModeChange: () => undefined,
      onAmountChange: () => undefined,
    });
    assert.match(html, /<fieldset/);
    assert.match(html, /<legend/);
    assert.equal((html.match(/type="radio"/g) ?? []).length, 2);
    assert.equal((html.match(/name="global-unpaid"/g) ?? []).length, 2);
    assert.match(html, /value="none"/);
    assert.match(html, /value="amount"/);
    assert.equal(html.includes('value="inherit"'), false);
    assert.equal(html.includes('type="number"'), false);
  });

  it('shows a required euro amount only for amount mode and flags an invalid value', async () => {
    const html = renderFields(await i18nFor('fr'), {
      scope: 'audience',
      name: 'audience-unpaid',
      mode: 'amount',
      amount: '',
      error: fr.settings.finePolicy.invalidAmount,
      onModeChange: () => undefined,
      onAmountChange: () => undefined,
    });
    assert.equal((html.match(/type="radio"/g) ?? []).length, 3);
    assert.match(html, /type="number"/);
    assert.match(html, /required/);
    assert.match(html, /aria-invalid="true"/);
    assert.match(html, /aria-describedby="[^"]+"/);
    assert.match(html, new RegExp(fr.settings.finePolicy.invalidAmount));
    assert.equal(html.includes(fr.settings.publicTypes.unpaidFineThresholdInheritEmpty), false);
  });

  it('shows the inherited global amount as a hint and hides the amount field', async () => {
    const html = renderFields(await i18nFor('fr'), {
      scope: 'audience',
      name: 'audience-inherit',
      mode: 'inherit',
      amount: '9.00',
      inheritedAmount: '12.50',
      onModeChange: () => undefined,
      onAmountChange: () => undefined,
    });
    const inherit = html.match(/<input[^>]*value="inherit"[^>]*>/)?.[0] ?? '';
    assert.match(inherit, /checked/);
    assert.match(html, /12\.50/);
    assert.match(html, /aria-describedby="[^"]+"/);
    assert.equal(html.includes('type="number"'), false);
    assert.equal(html.includes('value="9.00"'), false);
  });

  it('says when the global setting does not block, in each locale', async () => {
    for (const lng of ['fr', 'en', 'de', 'es'] as const) {
      const i18n = await i18nFor(lng);
      const html = renderFields(i18n, {
        scope: 'audience',
        name: `inherit-${lng}`,
        mode: 'inherit',
        amount: '',
        inheritedAmount: null,
        onModeChange: () => undefined,
        onAmountChange: () => undefined,
      });
      const hint = {
        fr,
        en,
        de,
        es,
      }[lng].settings.publicTypes.unpaidFineThresholdInheritEmpty;
      assert.match(html, new RegExp(hint.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
    }
  });
});
