/** Node test runs have no browser storage. Import this before any module that reads it. */
if (typeof globalThis.localStorage === 'undefined') {
  const memoryStore = new Map<string, string>();
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key: string) => memoryStore.get(key) ?? null,
      setItem: (key: string, value: string) => {
        memoryStore.set(key, value);
      },
      removeItem: (key: string) => {
        memoryStore.delete(key);
      },
      clear: () => {
        memoryStore.clear();
      },
    },
  });
}
