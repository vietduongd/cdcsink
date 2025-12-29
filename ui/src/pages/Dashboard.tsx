import React from "react";
import { StatsPanel as StatsComponent } from "../components/StatsPanel";
import { MetricsChart } from "../components/MetricsChart";
import {
  Zap,
  Activity,
  ArrowRight,
  ShieldCheck,
  Clock,
  Filter,
  Download,
} from "lucide-react";

export const Dashboard: React.FC = () => {
  return (
    <div className="space-y-8 animate-in fade-in duration-500">
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-2xl font-bold text-slate-900 tracking-tight">
            System Performance
          </h1>
          <div className="flex items-center gap-2 mt-1">
            <span className="text-xs font-bold text-slate-500 uppercase tracking-wider">
              Metrics
            </span>
            <div className="w-1 h-1 bg-slate-300 rounded-full" />
            <span className="text-xs font-bold text-slate-400">
              Real-time monitoring enabled
            </span>
          </div>
        </div>
        <div className="flex gap-2">
          <button className="flex items-center gap-2 px-4 py-2 bg-white border border-slate-200 rounded-lg text-xs font-bold text-slate-600 hover:bg-slate-50 transition-all">
            <Filter size={14} />
            Filter
          </button>
          <button className="flex items-center gap-2 px-4 py-2 bg-white border border-slate-200 rounded-lg text-xs font-bold text-slate-600 hover:bg-slate-50 transition-all">
            <Download size={14} />
            Export
          </button>
          <button className="flex items-center gap-2 px-4 py-2 bg-indigo-600 rounded-lg text-xs font-bold text-white hover:bg-indigo-700 transition-all active:scale-95">
            <Zap size={14} />
            Manual Sync
          </button>
        </div>
      </div>

      <StatsComponent />

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2 bg-white border border-slate-200 rounded-xl flex flex-col overflow-hidden">
          <div className="px-6 py-4 border-b border-slate-100 flex justify-between items-center bg-slate-50/50">
            <div className="flex items-center gap-2">
              <div className="w-1 h-4 bg-indigo-600 rounded-full" />
              <h3 className="text-sm font-bold text-slate-800 uppercase tracking-wider">
                Throughput (RPM)
              </h3>
            </div>
            <div className="flex items-center gap-1.5 px-2 py-1 bg-emerald-50 rounded border border-emerald-100">
              <div className="w-1.5 h-1.5 bg-emerald-500 rounded-full animate-pulse" />
              <span className="text-[10px] font-black text-emerald-600 uppercase">
                Live
              </span>
            </div>
          </div>
          <div className="p-6 h-[350px]">
            <MetricsChart />
          </div>
        </div>

        <div className="bg-white border border-slate-200 rounded-xl flex flex-col overflow-hidden">
          <div className="px-6 py-4 border-b border-slate-100 bg-slate-50/50">
            <h3 className="text-sm font-bold text-slate-800 uppercase tracking-wider">
              System Insights
            </h3>
          </div>
          <div className="p-6 space-y-6 flex-1">
            <InfoItem
              icon={<Activity size={18} />}
              label="Active Stream"
              value="cdc.events.primary"
              color="text-indigo-600"
              bgColor="bg-indigo-50"
            />
            <InfoItem
              icon={<ShieldCheck size={18} />}
              label="Security Mode"
              value="TLS 1.3 / mTLS"
              color="text-emerald-600"
              bgColor="bg-emerald-50"
            />
            <InfoItem
              icon={<Clock size={18} />}
              label="Uptime SLA"
              value="99.99% Guaranteed"
              color="text-amber-600"
              bgColor="bg-amber-50"
            />
          </div>
          <div className="px-6 py-4 border-t border-slate-100 bg-slate-50/30">
            <button className="flex items-center gap-2 text-indigo-600 font-bold text-xs hover:text-indigo-700 group w-full justify-between">
              <span>View detailed system log</span>
              <ArrowRight
                size={14}
                className="transition-transform group-hover:translate-x-0.5"
              />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

const InfoItem = ({ icon, label, value, color, bgColor }: any) => (
  <div className="flex items-center gap-4 group">
    <div
      className={`w-10 h-10 rounded-lg ${bgColor} ${color} flex items-center justify-center transition-colors`}
    >
      {icon}
    </div>
    <div className="flex flex-col">
      <span className="text-[10px] font-bold text-slate-400 uppercase tracking-widest">
        {label}
      </span>
      <span className="text-sm font-bold text-slate-700">{value}</span>
    </div>
  </div>
);
