import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';
import type { LibraryInfo, ScheduleSlot } from '@/types';
import { useLibraryInfoQuery } from '@/hooks/common/useLibraryInfoQuery';
import { useActiveScheduleSlotsQuery } from '@/hooks/common/useScheduleQueries';

const STORAGE_KEY = 'elidune_library_name';

interface LibraryContextValue {
  libraryName: string | null;
  libraryInfo: LibraryInfo | null;
  scheduleSlots: ScheduleSlot[];
  refreshSchedule: () => Promise<void>;
  refreshLibraryInfo: () => Promise<void>;
}

const LibraryContext = createContext<LibraryContextValue>({
  libraryName: null,
  libraryInfo: null,
  scheduleSlots: [],
  refreshSchedule: async () => {},
  refreshLibraryInfo: async () => {},
});

export function LibraryProvider({ children }: { children: React.ReactNode }) {
  const { data: libraryInfo, refetch: refetchLibraryInfo } = useLibraryInfoQuery();
  const { data: scheduleSlots = [], refetch: refetchScheduleSlots } = useActiveScheduleSlotsQuery();
  const [storedName] = useState<string | null>(() => localStorage.getItem(STORAGE_KEY));
  const refreshScheduleInFlightRef = useRef<Promise<void> | null>(null);
  const libraryName = libraryInfo !== undefined ? (libraryInfo.name ?? null) : storedName;

  useEffect(() => {
    if (libraryInfo === undefined) return;
    const name = libraryInfo.name ?? null;
    if (name) localStorage.setItem(STORAGE_KEY, name);
    else localStorage.removeItem(STORAGE_KEY);
  }, [libraryInfo]);

  const refreshLibraryInfo = useCallback(async () => {
    await refetchLibraryInfo();
  }, [refetchLibraryInfo]);

  const refreshSchedule = useCallback(async () => {
    if (refreshScheduleInFlightRef.current) {
      return refreshScheduleInFlightRef.current;
    }
    const request = (async () => {
      try {
        await refetchScheduleSlots();
      } finally {
        refreshScheduleInFlightRef.current = null;
      }
    })();
    refreshScheduleInFlightRef.current = request;
    return request;
  }, [refetchScheduleSlots]);

  return (
    <LibraryContext.Provider
      value={{
        libraryName,
        libraryInfo: libraryInfo ?? null,
        scheduleSlots,
        refreshSchedule,
        refreshLibraryInfo,
      }}
    >
      {children}
    </LibraryContext.Provider>
  );
}

export function useLibrary() {
  return useContext(LibraryContext);
}
