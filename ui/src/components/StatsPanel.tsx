import { useStats } from "../hooks/useQueries";

interface StatCardProps {
  title: string;
  value: number | string;
  icon: string;
  color: string;
}

const StatCard = ({ title, value, icon, color }: StatCardProps) => {
  return (
    <div className="stat-card">
      <div className="stat-icon" style={{ backgroundColor: color }}>
        {icon}
      </div>
      <div className="stat-content">
        <div className="stat-title">{title}</div>
        <div className="stat-value">{value}</div>
      </div>
    </div>
  );
};

export const StatsPanel = () => {
  const { data: stats, isLoading, error } = useStats();

  if (isLoading) {
    return <div className="loading">Loading stats...</div>;
  }

  if (error) {
    return <div className="error">Failed to load stats</div>;
  }

  const formatUptime = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    return `${hours}h ${minutes}m ${secs}s`;
  };

  return (
    <div className="stats-grid">
      <StatCard
        title="Records Received"
        value={stats?.records_received || 0}
        icon="📥"
        color="rgba(96, 165, 250, 0.2)"
      />
      <StatCard
        title="Records Written"
        value={stats?.records_written || 0}
        icon="📤"
        color="rgba(52, 211, 153, 0.2)"
      />
      <StatCard
        title="Errors"
        value={stats?.errors || 0}
        icon="⚠️"
        color="rgba(239, 68, 68, 0.2)"
      />
      <StatCard
        title="Uptime"
        value={formatUptime(stats?.uptime_seconds || 0)}
        icon="⏱️"
        color="rgba(168, 85, 247, 0.2)"
      />
    </div>
  );
};
