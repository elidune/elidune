import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';
import api from '@/services/api';
import type { LibraryInfo, ScheduleSlot } from '@/types';
import { useLibraryInfoQuery } from '@/hooks/common/useLibraryInfoQuery';

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
  const [storedName] = useState<string | null>(() => localStorage.getItem(STORAGE_KEY));
  const [scheduleSlots, setScheduleSlots] = useState<ScheduleSlot[]>([]);
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
        const periods = await api.getSchedulePeriods();
        const today = new Date().toISOString().split('T')[0];
        const activePeriod = periods.find(
          (p) => p.startDate <= today && p.endDate >= today
        );
        if (activePeriod) {
          const slots = await api.getScheduleSlots(activePeriod.id);
          setScheduleSlots(slots);
        } else {
          setScheduleSlots([]);
        }
      } catch {
        // silent fail — schedule is non-critical
      } finally {
        refreshScheduleInFlightRef.current = null;
      }
    })();
    refreshScheduleInFlightRef.current = request;
    return request;
  }, []);

  // Fetch schedule on mount if already authenticated (returning user)
  useEffect(() => {
    refreshSchedule();
  }, [refreshSchedule]);

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
