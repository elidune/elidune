import {
  createContext,
  useCallback,
  useContext,
  useLayoutEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';

type MobilePageTitleContextValue = {
  mobileTitle: ReactNode | null;
  setMobileTitle: (title: ReactNode | null) => void;
};

const MobilePageTitleContext = createContext<MobilePageTitleContextValue | null>(null);

export function MobilePageTitleProvider({ children }: { children: ReactNode }) {
  const [mobileTitle, setMobileTitleState] = useState<ReactNode | null>(null);
  const setMobileTitle = useCallback((title: ReactNode | null) => {
    setMobileTitleState(title);
  }, []);
  const value = useMemo(
    () => ({ mobileTitle, setMobileTitle }),
    [mobileTitle, setMobileTitle],
  );
  return (
    <MobilePageTitleContext.Provider value={value}>{children}</MobilePageTitleContext.Provider>
  );
}

function useMobilePageTitleContext(): MobilePageTitleContextValue {
  const ctx = useContext(MobilePageTitleContext);
  if (!ctx) {
    throw new Error('useMobilePageTitleContext requires MobilePageTitleProvider');
  }
  return ctx;
}

/** Sets the sticky mobile app header title; cleared on unmount. */
export function useMobilePageTitle(title: ReactNode | null) {
  const { setMobileTitle } = useMobilePageTitleContext();
  useLayoutEffect(() => {
    setMobileTitle(title);
    return () => setMobileTitle(null);
  }, [title, setMobileTitle]);
}

export function useMobilePageTitleDisplay() {
  return useMobilePageTitleContext().mobileTitle;
}
