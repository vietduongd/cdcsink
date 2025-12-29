import React from "react";
import { Link, useLocation } from "react-router-dom";
import {
  LayoutDashboard,
  Cable,
  Database,
  Activity,
  Box,
  Shield,
  Search,
  Bell,
  User,
} from "lucide-react";

interface LayoutProps {
  children: React.ReactNode;
}

export const Layout: React.FC<LayoutProps> = ({ children }) => {
  const location = useLocation();

  const navItems = [
    { path: "/", label: "Dashboard", icon: <LayoutDashboard size={18} /> },
    { path: "/connectors", label: "Connectors", icon: <Box size={18} /> },
    {
      path: "/destinations",
      label: "Destinations",
      icon: <Database size={18} />,
    },
    { path: "/flows", label: "Sync Flows", icon: <Cable size={18} /> },
  ];

  return (
    <div className="flex min-h-screen bg-slate-50 font-sans text-slate-900">
      {/* Sidebar */}
      <aside className="fixed h-screen w-64 bg-slate-900 border-r border-slate-800 flex flex-col z-50">
        <div className="p-6 border-b border-slate-800">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 bg-indigo-600 rounded flex items-center justify-center text-white font-black">
              C
            </div>
            <span className="text-lg font-bold text-white tracking-tight">
              CDC SINK
            </span>
          </div>
        </div>

        <div className="px-4 py-8 flex-1">
          <div className="text-[10px] font-bold text-slate-500 uppercase tracking-[0.2em] mb-4 px-2">
            Main Menu
          </div>
          <nav className="space-y-1">
            {navItems.map((item) => {
              const isActive = location.pathname === item.path;
              return (
                <Link
                  key={item.path}
                  to={item.path}
                  className={`flex items-center gap-3 px-3 py-2.5 rounded-lg transition-all text-sm font-semibold group ${
                    isActive
                      ? "bg-indigo-600 text-white"
                      : "text-slate-400 hover:bg-slate-800 hover:text-white"
                  }`}
                >
                  <span
                    className={
                      isActive
                        ? "text-white"
                        : "text-slate-500 group-hover:text-slate-300"
                    }
                  >
                    {item.icon}
                  </span>
                  <span>{item.label}</span>
                </Link>
              );
            })}
          </nav>
        </div>

        <div className="p-4 border-t border-slate-800">
          <div className="p-3 bg-slate-800/50 rounded-lg flex items-center gap-3">
            <div className="w-2 h-2 bg-emerald-500 rounded-full animate-pulse" />
            <div className="flex flex-col">
              <span className="text-[10px] font-bold text-slate-400 uppercase tracking-wider">
                System Status
              </span>
              <span className="text-xs font-bold text-slate-200">
                Production Ready
              </span>
            </div>
          </div>
        </div>
      </aside>

      {/* Main Content */}
      <div className="flex-1 ml-64 flex flex-col min-h-screen">
        {/* Top Header */}
        <header className="h-16 bg-white border-b border-slate-200 sticky top-0 z-40 px-8 flex items-center justify-between">
          <div className="flex items-center gap-4 text-slate-400">
            <Search size={18} />
            <input
              type="text"
              placeholder="Search records, entities..."
              className="bg-transparent border-none focus:outline-none text-sm font-medium w-64"
            />
          </div>
          <div className="flex items-center gap-6">
            <button className="text-slate-400 hover:text-indigo-600 transition-colors">
              <Bell size={20} />
            </button>
            <div className="h-8 w-[1px] bg-slate-200" />
            <div className="flex items-center gap-3">
              <div className="text-right">
                <div className="text-xs font-bold text-slate-900 leading-none">
                  Admin User
                </div>
                <div className="text-[10px] font-bold text-slate-500 uppercase mt-1">
                  Super Admin
                </div>
              </div>
              <div className="w-9 h-9 bg-slate-100 rounded-full border border-slate-200 flex items-center justify-center text-slate-600">
                <User size={20} />
              </div>
            </div>
          </div>
        </header>

        <main className="p-8 max-w-7xl mx-auto w-full flex-1">{children}</main>
      </div>
    </div>
  );
};
