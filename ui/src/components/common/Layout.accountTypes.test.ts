import assert from 'node:assert/strict';
import { after, describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import axios from 'axios';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import { MemoryRouter } from 'react-router-dom';
import de from '../../locales/de/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import es from '../../locales/es/translation.json' with { type: 'json' };
import fr from '../../locales/fr/translation.json' with { type: 'json' };

type DomNode = {
  nodeType: number;
  nodeName: string;
  tagName?: string;
  nodeValue: string | null;
  parentNode: DomNode | null;
  childNodes: DomNode[];
  ownerDocument: DomNode | null;
  namespaceURI: string | null;
  attributes: Map<string, string>;
  style: { setProperty: (name: string, value: string) => void };
  classList: { add: (...names: string[]) => void; remove: (...names: string[]) => void; contains: (name: string) => boolean };
  textContent: string;
  appendChild: (child: DomNode) => DomNode;
  removeChild: (child: DomNode) => DomNode;
  insertBefore: (child: DomNode, ref: DomNode | null) => DomNode;
  setAttribute: (name: string, value: string) => void;
  setAttributeNS: (ns: string | null, name: string, value: string) => void;
  getAttribute: (name: string) => string | null;
  removeAttribute: (name: string) => void;
  hasAttribute: (name: string) => boolean;
  addEventListener: () => void;
  removeEventListener: () => void;
  focus: () => void;
  contains: (other: DomNode | null) => boolean;
};

function createDomNode(nodeType: number, nodeName: string, owner: DomNode | null): DomNode {
  const node = {
    nodeType,
    nodeName,
    nodeValue: null as string | null,
    parentNode: null as DomNode | null,
    childNodes: [] as DomNode[],
    ownerDocument: owner,
    namespaceURI: null as string | null,
    attributes: new Map<string, string>(),
  } as DomNode;

  const detach = (child: DomNode) => {
    if (!child.parentNode) return;
    const siblings = child.parentNode.childNodes;
    const index = siblings.indexOf(child);
    if (index >= 0) siblings.splice(index, 1);
    child.parentNode = null;
  };

  node.appendChild = (child) => {
    detach(child);
    child.parentNode = node;
    node.childNodes.push(child);
    return child;
  };
  node.removeChild = (child) => {
    const index = node.childNodes.indexOf(child);
    if (index >= 0) node.childNodes.splice(index, 1);
    if (child.parentNode === node) child.parentNode = null;
    return child;
  };
  node.insertBefore = (child, ref) => {
    if (!ref) return node.appendChild(child);
    detach(child);
    const index = node.childNodes.indexOf(ref);
    child.parentNode = node;
    if (index < 0) node.childNodes.push(child);
    else node.childNodes.splice(index, 0, child);
    return child;
  };
  node.setAttribute = (name, value) => {
    node.attributes.set(name, String(value));
  };
  node.setAttributeNS = (_ns, name, value) => {
    node.setAttribute(name, value);
  };
  node.getAttribute = (name) => node.attributes.get(name) ?? null;
  node.removeAttribute = (name) => {
    node.attributes.delete(name);
  };
  node.hasAttribute = (name) => node.attributes.has(name);
  node.addEventListener = () => undefined;
  node.removeEventListener = () => undefined;
  node.focus = () => undefined;
  node.contains = (other) => {
    let current = other;
    while (current) {
      if (current === node) return true;
      current = current.parentNode;
    }
    return false;
  };
  node.classList = {
    add: (...names) => {
      const current = new Set((node.getAttribute('class') ?? '').split(/\s+/).filter(Boolean));
      for (const name of names) current.add(name);
      node.setAttribute('class', [...current].join(' '));
    },
    remove: (...names) => {
      const current = new Set((node.getAttribute('class') ?? '').split(/\s+/).filter(Boolean));
      for (const name of names) current.delete(name);
      node.setAttribute('class', [...current].join(' '));
    },
    contains: (name) => (node.getAttribute('class') ?? '').split(/\s+/).includes(name),
  };
  node.style = {
    setProperty: (name, value) => {
      (node.style as unknown as Record<string, string>)[name] = value;
    },
  };
  Object.defineProperty(node, 'textContent', {
    get() {
      if (node.nodeType === 3 || node.nodeType === 8) return node.nodeValue ?? '';
      return node.childNodes.map((child) => child.textContent).join('');
    },
    set(value: string) {
      node.childNodes = [];
      if (value) {
        const owner = (node.ownerDocument ?? node) as DomNode & {
          createTextNode?: (text: string) => DomNode;
        };
        const text = owner.createTextNode?.(String(value));
        if (text) node.appendChild(text);
        else node.nodeValue = String(value);
      }
    },
  });
  Object.defineProperty(node, 'firstChild', { get: () => node.childNodes[0] ?? null });
  Object.defineProperty(node, 'lastChild', { get: () => node.childNodes[node.childNodes.length - 1] ?? null });
  Object.defineProperty(node, 'nextSibling', {
    get: () => {
      const siblings = node.parentNode?.childNodes ?? [];
      const index = siblings.indexOf(node);
      return index >= 0 ? siblings[index + 1] ?? null : null;
    },
  });
  Object.defineProperty(node, 'options', {
    get: () => {
      const found: DomNode[] = [];
      const visit = (current: DomNode) => {
        const name = (current.tagName ?? current.nodeName).toUpperCase();
        if (name === 'OPTION') found.push(current);
        for (const child of current.childNodes) visit(child);
      };
      for (const child of node.childNodes) visit(child);
      return found;
    },
  });
  return node;
}

type DomDocument = DomNode & {
  documentElement: DomNode;
  body: DomNode;
  defaultView: typeof globalThis;
  createElement: (tag: string) => DomNode;
  createElementNS: (ns: string, tag: string) => DomNode;
  createTextNode: (text: string) => DomNode;
  createComment: (text: string) => DomNode;
  createDocumentFragment: () => DomNode;
  querySelectorAll: (selector: string) => DomNode[];
  querySelector: (selector: string) => DomNode | null;
  activeElement: DomNode | null;
};

function createDocument(): DomDocument {
  const doc = createDomNode(9, '#document', null) as DomDocument;
  doc.ownerDocument = doc;
  doc.createElement = (tag) => {
    const el = createDomNode(1, tag.toUpperCase(), doc);
    el.tagName = tag.toUpperCase();
    el.namespaceURI = 'http://www.w3.org/1999/xhtml';
    return el;
  };
  doc.createElementNS = (ns, tag) => {
    const el = doc.createElement(tag);
    el.namespaceURI = ns;
    el.nodeName = tag;
    el.tagName = tag;
    return el;
  };
  doc.createTextNode = (text) => {
    const node = createDomNode(3, '#text', doc);
    node.nodeValue = text;
    return node;
  };
  doc.createComment = (text) => {
    const node = createDomNode(8, '#comment', doc);
    node.nodeValue = text;
    return node;
  };
  doc.createDocumentFragment = () => createDomNode(11, '#document-fragment', doc);
  doc.querySelectorAll = () => [];
  doc.querySelector = () => null;
  doc.activeElement = null;
  doc.documentElement = doc.createElement('html');
  doc.body = doc.createElement('body');
  doc.appendChild(doc.documentElement);
  doc.documentElement.appendChild(doc.body);
  doc.defaultView = globalThis;
  return doc;
}

function installDom(): () => void {
  const doc = createDocument();
  const g = globalThis as unknown as Record<string, unknown>;
  const memory = new Map<string, string>();
  const had = {
    window: 'window' in globalThis,
    document: 'document' in globalThis,
    localStorage: 'localStorage' in globalThis,
  };
  g.window = globalThis;
  g.document = doc;
  g.HTMLIFrameElement = class HTMLIFrameElement {};
  g.IS_REACT_ACT_ENVIRONMENT = true;
  const storage = {
    getItem: (key: string) => memory.get(key) ?? null,
    setItem: (key: string, value: string) => {
      memory.set(key, value);
    },
    removeItem: (key: string) => {
      memory.delete(key);
    },
    clear: () => {
      memory.clear();
    },
  };
  // Node may expose a broken `localStorage` stub; always use an in-memory store for these tests.
  if (!had.localStorage || typeof (g.localStorage as typeof storage | undefined)?.getItem !== 'function') {
    g.localStorage = storage;
  }
  class ResizeObserverStub {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  g.ResizeObserver = ResizeObserverStub;
  g.matchMedia = (query: string) => ({
    matches: false,
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  });
  g.getComputedStyle = () => ({ display: 'block', visibility: 'visible' });
  g.requestAnimationFrame = (cb: () => void) => setTimeout(cb, 0);
  g.cancelAnimationFrame = () => undefined;
  if (typeof g.addEventListener !== 'function') {
    g.addEventListener = () => undefined;
    g.removeEventListener = () => undefined;
  }
  return () => {
    if (!had.window) delete g.window;
    if (!had.document) delete g.document;
  };
}

const restoreDom = installDom();
after(restoreDom);

function token(rights: Record<string, string>): string {
  const payload = Buffer.from(JSON.stringify({ rights })).toString('base64url');
  return `e30.${payload}.sig`;
}

const READER = token({
  itemsRights: 'Read',
  usersRights: 'Own',
  loansRights: 'Own',
  holdsRights: 'Own',
  settingsRights: 'Read',
  eventsRights: 'Read',
});

const STAFF = token({
  itemsRights: 'Write',
  usersRights: 'Write',
  loansRights: 'Write',
  holdsRights: 'Write',
  settingsRights: 'Write',
  eventsRights: 'Write',
});

interface CapturedRequest {
  url?: string;
  baseURL?: string;
  method?: string;
}

function requestUrl(config: CapturedRequest): string {
  return `${config.baseURL ?? ''}${config.url ?? ''}`;
}

async function i18nForEn(): Promise<I18nInstance> {
  const i18n = createInstance();
  await i18n.init({
    lng: 'en',
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

function renderedText(node: DomNode): string {
  const parts: string[] = [];
  const walk = (current: DomNode) => {
    if (current.nodeType === 3 || current.nodeType === 8) {
      parts.push(current.nodeValue ?? '');
      return;
    }
    const value = (current as DomNode & { value?: unknown }).value;
    if (typeof value === 'string' && value) parts.push(value);
    for (const attr of current.attributes.values()) parts.push(attr);
    for (const child of current.childNodes) walk(child);
  };
  walk(node);
  return parts.join('\n');
}

async function mountShell(
  accountType: string,
  authToken: string,
  calls: CapturedRequest[],
  screen: 'layout' | 'settings' = 'layout',
): Promise<{ root: Root; text: () => string }> {
  const api = (await import('../../services/api.ts')).default;
  const { AuthProvider } = await import('../../contexts/AuthContext.tsx');
  const { ThemeProvider } = await import('../../contexts/ThemeContext.tsx');
  const page = screen === 'settings'
    ? createElement((await import('../settings/AccountTypesSettings.tsx')).default)
    : createElement((await import('./Layout.tsx')).default, null, createElement('p', null, 'reader page'));
  api.setToken(authToken);

  const proto = axios.Axios.prototype as unknown as {
    request: (config: CapturedRequest) => Promise<unknown>;
  };
  const original = proto.request;
  proto.request = async function request(config: CapturedRequest) {
    calls.push(config);
    const url = requestUrl(config);
    if (url.includes('/account-types')) {
      return {
        data: [{ code: accountType, name: 'Desk reader' }],
        status: 200,
        statusText: 'OK',
        headers: {},
        config,
      };
    }
    if (url.includes('/auth/me')) {
      return {
        data: {
          id: '7',
          username: 'ada',
          login: 'ada',
          firstname: 'Ada',
          lastname: 'Lovelace',
          accountType,
        },
        status: 200,
        statusText: 'OK',
        headers: {},
        config,
      };
    }
    return { data: { version: 'test' }, status: 200, statusText: 'OK', headers: {}, config };
  };

  const doc = document as unknown as DomDocument;
  const container = doc.createElement('div');
  doc.body.appendChild(container);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const i18n = await i18nForEn();
  const tree = createElement(
    QueryClientProvider,
    { client },
    createElement(
      ThemeProvider,
      null,
      createElement(
        AuthProvider,
        null,
        createElement(
          I18nextProvider,
          { i18n },
          createElement(
            MemoryRouter,
            null,
            page,
          ),
        ),
      ),
    ),
  ) as ReactElement;
  const root = createRoot(container as unknown as Element);
  await act(async () => {
    root.render(tree);
  });
  for (let i = 0; i < 8; i += 1) {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }
  proto.request = original;
  return { root, text: () => renderedText(container) };
}

describe('layout account types query', { concurrency: 1 }, () => {
  it('does not request account types for a reader session', async () => {
    const calls: CapturedRequest[] = [];
    // Named like staff so a check on the account-type string would still fire the query.
    const { root, text } = await mountShell('Librarian', READER, calls);
    try {
      assert.equal(calls.some((call) => requestUrl(call).includes('/auth/me')), true);
      assert.equal(calls.some((call) => requestUrl(call).includes('/account-types')), false);
      assert.match(text(), /Librarian/);
      assert.equal(text().includes('Desk reader'), false);
    } finally {
      await act(async () => {
        root.unmount();
      });
      const api = (await import('../../services/api.ts')).default;
      api.logout();
    }
  });

  it('requests account types for a staff session that can read them', async () => {
    const calls: CapturedRequest[] = [];
    // Named like a reader so the gate has to follow settings write, not the account type.
    const { root, text } = await mountShell('reader', STAFF, calls);
    try {
      assert.equal(calls.some((call) => requestUrl(call).includes('/account-types')), true);
      assert.match(text(), /Desk reader/);
    } finally {
      await act(async () => {
        root.unmount();
      });
      const api = (await import('../../services/api.ts')).default;
      api.logout();
    }
  });

  it('still loads account types on the settings screen for settings write', async () => {
    const calls: CapturedRequest[] = [];
    const { root, text } = await mountShell('admin', STAFF, calls, 'settings');
    try {
      assert.equal(calls.some((call) => requestUrl(call).includes('/account-types')), true);
      assert.match(text(), /Desk reader/);
    } finally {
      await act(async () => {
        root.unmount();
      });
      const api = (await import('../../services/api.ts')).default;
      api.logout();
    }
  });
});
