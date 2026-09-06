import {
  Area,
  AreaChart,
  CartesianGrid,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';

export interface LoansAreaPoint {
  date: string;
  loans: number;
  returns: number;
}

interface LoansAreaChartProps {
  data: LoansAreaPoint[];
  formatTick: (date: string) => string;
  formatTooltipLabel: (label: string) => string;
  loansName: string;
  returnsName: string;
  heightClass?: string;
  gradientPrefix?: string;
}

export default function LoansAreaChart({
  data,
  formatTick,
  formatTooltipLabel,
  loansName,
  returnsName,
  heightClass = 'h-80',
  gradientPrefix = 'loans',
}: LoansAreaChartProps) {
  const loansFill = `url(#${gradientPrefix}Loans)`;
  const returnsFill = `url(#${gradientPrefix}Returns)`;

  return (
    <div className={heightClass}>
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={data} margin={{ top: 10, right: 30, left: 0, bottom: 0 }}>
          <defs>
            <linearGradient id={`${gradientPrefix}Loans`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor="#6366f1" stopOpacity={0.3} />
              <stop offset="95%" stopColor="#6366f1" stopOpacity={0} />
            </linearGradient>
            <linearGradient id={`${gradientPrefix}Returns`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor="#10b981" stopOpacity={0.3} />
              <stop offset="95%" stopColor="#10b981" stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" className="stroke-gray-200 dark:stroke-gray-700" />
          <XAxis
            dataKey="date"
            tickFormatter={formatTick}
            tick={{ fill: 'currentColor', fontSize: 12 }}
            className="text-gray-500"
          />
          <YAxis tick={{ fill: 'currentColor', fontSize: 12 }} className="text-gray-500" />
          <Tooltip
            contentStyle={{
              backgroundColor: 'var(--tooltip-bg, #fff)',
              borderColor: 'var(--tooltip-border, #e5e7eb)',
              borderRadius: '0.5rem',
            }}
            labelFormatter={(label) => formatTooltipLabel(String(label ?? ''))}
          />
          <Legend />
          <Area
            type="monotone"
            dataKey="loans"
            name={loansName}
            stroke="#6366f1"
            strokeWidth={2}
            fill={loansFill}
          />
          <Area
            type="monotone"
            dataKey="returns"
            name={returnsName}
            stroke="#10b981"
            strokeWidth={2}
            fill={returnsFill}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
