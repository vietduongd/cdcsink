import { create } from "zustand";

interface Stats {
  records_received: number;
  records_written: number;
  errors: number;
  uptime_seconds: number;
}

interface SystemStore {
  stats: Stats | null;
  connected: boolean;
  setStats: (stats: Stats) => void;
  setConnected: (connected: boolean) => void;
}

export const useSystemStore = create<SystemStore>((set) => ({
  stats: null,
  connected: false,
  setStats: (stats) => set({ stats }),
  setConnected: (connected) => set({ connected }),
}));
