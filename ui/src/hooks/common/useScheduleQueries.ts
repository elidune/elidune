import { useQuery, type QueryClient } from '@tanstack/react-query';
import api from '@/services/api';
import type { ScheduleClosure, SchedulePeriod, ScheduleSlot } from '@/types';

export const SCHEDULE_ACTIVE_SLOTS_KEY = ['schedule', 'active-slots'] as const;
export const SCHEDULE_ADMIN_KEY = ['schedule', 'admin'] as const;

async function fetchActiveScheduleSlots(): Promise<ScheduleSlot[]> {
  const periods = await api.getSchedulePeriods();
  const today = new Date().toISOString().split('T')[0];
  const activePeriod = periods.find((p) => p.startDate <= today && p.endDate >= today);
  if (!activePeriod) return [];
  return api.getScheduleSlots(activePeriod.id);
}

export function useActiveScheduleSlotsQuery(enabled = true) {
  return useQuery({
    queryKey: SCHEDULE_ACTIVE_SLOTS_KEY,
    queryFn: fetchActiveScheduleSlots,
    enabled,
    staleTime: 5 * 60 * 1000,
  });
}

export interface ScheduleAdminData {
  periods: SchedulePeriod[];
  closures: ScheduleClosure[];
  slots: Record<string, ScheduleSlot[]>;
}

async function fetchScheduleAdmin(): Promise<ScheduleAdminData> {
  const [perList, closList] = await Promise.all([
    api.getSchedulePeriods(),
    api.getScheduleClosures(),
  ]);
  const closures = [...closList].sort((a, b) => a.closureDate.localeCompare(b.closureDate));
  const slotMap: Record<string, ScheduleSlot[]> = {};
  await Promise.all(
    perList.map(async (p) => {
      slotMap[p.id] = await api.getScheduleSlots(p.id);
    }),
  );
  return { periods: perList, closures, slots: slotMap };
}

export function useScheduleAdminQuery(enabled = true) {
  return useQuery({
    queryKey: SCHEDULE_ADMIN_KEY,
    queryFn: fetchScheduleAdmin,
    enabled,
    staleTime: 60 * 1000,
  });
}

export async function invalidateScheduleQueries(queryClient: QueryClient) {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: SCHEDULE_ADMIN_KEY }),
    queryClient.invalidateQueries({ queryKey: SCHEDULE_ACTIVE_SLOTS_KEY }),
  ]);
}
