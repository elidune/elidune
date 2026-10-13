import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createInstance, type TFunction } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { loginFailureMessage, rateLimitWaitMessage, retryWaitAmount } from './apiError.ts';

type Lang = 'fr' | 'en' | 'de' | 'es';

const MINUTE = /minutes|minute|Minuten|Minute|minutos|minuto/u;
const SECOND = /seconds|second|secondes|seconde|Sekunden|Sekunde|segundos|segundo/u;

async function tFor(lng: Lang): Promise<TFunction> {
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
  return i18n.t.bind(i18n);
}

function rateLimited(seconds: number) {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: {
      status: 429,
      headers: { 'retry-after': String(seconds) },
      data: {
        code: 'rate_limited',
        message: `Too many attempts. Try again in ${seconds} seconds.`,
        retryAfter: seconds,
      },
    },
  };
}

describe('rate-limit wait', () => {
  it('rounds a long wait to minutes and keeps a short wait in seconds', () => {
    assert.deepEqual(retryWaitAmount(636), { unit: 'minute', count: 11 });
    assert.deepEqual(retryWaitAmount(45), { unit: 'second', count: 45 });
    assert.deepEqual(retryWaitAmount(60), { unit: 'minute', count: 1 });
    assert.deepEqual(retryWaitAmount(90), { unit: 'minute', count: 2 });
  });

  it('does not show 636 seconds on the login error', async () => {
    const expected = {
      en: 'Too many attempts. Please wait about 11 minutes before trying again.',
      fr: 'Trop de tentatives. Veuillez patienter environ 11 minutes avant de réessayer.',
      de: 'Zu viele Versuche. Bitte warten Sie etwa 11 Minuten, bevor Sie es erneut versuchen.',
      es: 'Demasiados intentos. Espere aproximadamente 11 minutos antes de volver a intentarlo.',
    } as const;

    for (const lang of ['en', 'fr', 'de', 'es'] as const) {
      const t = await tFor(lang);
      const message = rateLimitWaitMessage(636, t, lang);
      assert.equal(message, expected[lang]);
      assert.equal(message.includes('636'), false);
      assert.equal(SECOND.test(message), false);
      assert.equal(MINUTE.test(message), true);
      assert.equal(message.includes('patron'), false);
      assert.equal(message.includes('usager'), false);
    }
  });

  it('ignores the English server sentence and keeps an exact wait under a minute', async () => {
    const t = await tFor('en');
    const message = loginFailureMessage(rateLimited(636), t);
    assert.equal(message.includes('636'), false);
    assert.equal(message.includes('Try again in'), false);
    assert.equal(MINUTE.test(message), true);

    const exact = rateLimitWaitMessage(45, t, 'en');
    assert.equal(exact, 'Too many attempts. Please wait 45 seconds before trying again.');
    assert.equal(exact.includes('about'), false);
  });
});
