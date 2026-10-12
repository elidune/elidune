import { useTranslation } from 'react-i18next';
import type { StatEntry, UserAggregateStats } from '@/types';
import { translateStatLabel } from '@/utils/codeLabels';

interface UserAggregateStatsViewProps {
  stats: UserAggregateStats;
}

function seriesLabel(
  t: (key: string) => string,
  entry: StatEntry,
  category: 'publicType' | 'accountType' | 'sex' | 'ageBand',
): string {
  return translateStatLabel(t, entry.label, category);
}

function StatSeries({
  title,
  entries,
  category,
}: {
  title: string;
  entries: StatEntry[] | undefined;
  category: 'publicType' | 'accountType' | 'sex' | 'ageBand';
}) {
  const { t } = useTranslation();
  if (!entries?.length) return null;
  return (
    <div>
      <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{title}</h3>
      <ul className="space-y-1">
        {entries.map((entry) => (
          <li key={`${category}-${entry.label}`} className="flex justify-between text-sm">
            <span className="text-gray-600 dark:text-gray-400">{seriesLabel(t, entry, category)}</span>
            <span className="font-medium">{entry.value.toLocaleString()}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

/**
 * `GET /stats/users` aggregates. Counts and `{ label, value }` series only.
 * There is no nominative ranking.
 */
export default function UserAggregateStatsView({ stats }: UserAggregateStatsViewProps) {
  const { t } = useTranslation();
  const publicTypeColumns =
    (stats.usersByPublicType?.length ?? 0) > 0 ||
    (stats.newUsersByPublicType?.length ?? 0) > 0 ||
    (stats.newUsersBySex?.length ?? 0) > 0 ||
    (stats.activeBorrowersByPublicType?.length ?? 0) > 0;
  const breakdown =
    (stats.usersByAgeBand?.length ?? 0) > 0 ||
    (stats.usersByAccountType?.length ?? 0) > 0 ||
    (stats.usersBySex?.length ?? 0) > 0;

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="p-4 rounded-xl bg-blue-50 dark:bg-blue-900/20">
          <p className="text-sm text-gray-600 dark:text-gray-400">{t('stats.usersSection.usersTotal')}</p>
          <p className="text-2xl font-bold text-blue-600 dark:text-blue-400">{stats.usersTotal.toLocaleString()}</p>
        </div>
        <div className="p-4 rounded-xl bg-purple-50 dark:bg-purple-900/20">
          <p className="text-sm text-gray-600 dark:text-gray-400">{t('stats.usersSection.newUsersTotal')}</p>
          <p className="text-2xl font-bold text-purple-600 dark:text-purple-400">{stats.newUsersTotal.toLocaleString()}</p>
        </div>
        <div className="p-4 rounded-xl bg-indigo-50 dark:bg-indigo-900/20">
          <p className="text-sm text-gray-600 dark:text-gray-400">{t('stats.usersSection.activeBorrowersTotal')}</p>
          <p className="text-2xl font-bold text-indigo-600 dark:text-indigo-400">
            {stats.activeBorrowersTotal.toLocaleString()}
          </p>
        </div>
        <div className="p-4 rounded-xl bg-emerald-50 dark:bg-emerald-900/20">
          <p className="text-sm text-gray-600 dark:text-gray-400">{t('stats.usersSection.groupsTotal')}</p>
          <p className="text-2xl font-bold text-emerald-600 dark:text-emerald-400">
            {(stats.groupsTotal ?? 0).toLocaleString()}
          </p>
        </div>
      </div>
      {publicTypeColumns ? (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6">
          <div className="space-y-4">
            <StatSeries
              title={t('stats.usersSection.usersByPublicType')}
              entries={stats.usersByPublicType}
              category="publicType"
            />
          </div>
          <div className="space-y-4">
            <StatSeries
              title={t('stats.usersSection.newUsersByPublicType')}
              entries={stats.newUsersByPublicType}
              category="publicType"
            />
            <StatSeries
              title={t('stats.usersSection.newUsersBySex')}
              entries={stats.newUsersBySex}
              category="sex"
            />
          </div>
          <div className="space-y-4">
            <StatSeries
              title={t('stats.usersSection.activeBorrowersByPublicType')}
              entries={stats.activeBorrowersByPublicType}
              category="publicType"
            />
          </div>
        </div>
      ) : null}
      {breakdown ? (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6">
          <StatSeries
            title={t('stats.usersSection.usersByAgeBand')}
            entries={stats.usersByAgeBand}
            category="ageBand"
          />
          <StatSeries
            title={t('stats.usersSection.usersByAccountType')}
            entries={stats.usersByAccountType}
            category="accountType"
          />
          <StatSeries title={t('stats.usersSection.usersBySex')} entries={stats.usersBySex} category="sex" />
        </div>
      ) : null}
    </div>
  );
}
