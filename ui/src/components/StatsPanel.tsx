import { useStats } from "../hooks/useQueries";
import { Download, Upload, AlertTriangle, Clock } from "lucide-react";

interface StatCardProps {
  title: string;
  value: number | string;
  icon: React.ReactNode;
  color: string;
  bgColor: string;
}

const StatCard = ({ title, value, icon, color, bgColor }: StatCardProps) => {
  return (
    <div className="bg-white p-5 rounded-xl border border-slate-200 group hover:border-indigo-300 transition-colors">
      <div className="flex items-start justify-between mb-3">
        <div
          className={`w-10 h-10 rounded-lg flex items-center justify-center ${bgColor} ${color}`}
        >
          {icon}
        </div>
        <div className="flex items-center gap-1 px-2 py-0.5 bg-slate-50 rounded border border-slate-100">
          <span className="text-[10px] font-bold text-slate-400">Trend</span>
          <div className="w-2 h-2 rounded-full bg-emerald-400" />
        </div>
      </div>
      <div>
        <div className="text-[10px] font-bold text-slate-400 uppercase tracking-wider mb-1">
          {title}
        </div>
        <div className="text-xl font-bold text-slate-900 tabular-nums">
          {value}
        </div>
      </div>
    </div>
  );
};

export const StatsPanel = () => {
  const { data: stats, isLoading, error } = useStats();

  if (isLoading) {
    return (
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {[1, 2, 3, 4].map((i) => (
          <div
            key={i}
            className="h-32 bg-white rounded-xl border border-slate-100 animate-pulse"
          ></div>
        ))}
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6 bg-red-50 rounded-xl border border-red-100 text-red-600 text-xs font-bold flex items-center gap-3">
        <AlertTriangle size={16} />
        System Metrics Unavailable
      </div>
    );
  }

  const formatUptime = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${minutes}m`;
  };

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
      <StatCard
        title="Records Recv"
        value={stats?.records_received || 0}
        icon={<Download size={20} />}
        color="text-indigo-600"
        bgColor="bg-indigo-50"
      />
      <StatCard
        title="Records Written"
        value={stats?.records_written || 0}
        icon={<Upload size={20} />}
        color="text-emerald-600"
        bgColor="bg-emerald-50"
      />
      <StatCard
        title="Critical Errors"
        value={stats?.errors || 0}
        icon={<AlertTriangle size={20} />}
        color="text-rose-600"
        bgColor="bg-rose-50"
      />
      <StatCard
        title="Session Uptime"
        value={formatUptime(stats?.uptime_seconds || 0)}
        icon={<Clock size={20} />}
        color="text-amber-600"
        bgColor="bg-amber-50"
      />
    </div>
  );
};
