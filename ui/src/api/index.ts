import axios from "axios";

const API_BASE_URL = import.meta.env.VITE_API_URL || "http://localhost:3000";

export interface HealthResponse {
  status: string;
  uptime_seconds: number;
}

export interface StatsResponse {
  records_received: number;
  records_written: number;
  errors: number;
  uptime_seconds: number;
}

export const api = {
  getHealth: async (): Promise<HealthResponse> => {
    const response = await axios.get<HealthResponse>(`${API_BASE_URL}/health`);
    return response.data;
  },

  getStats: async (): Promise<StatsResponse> => {
    const response = await axios.get<StatsResponse>(
      `${API_BASE_URL}/api/stats`
    );
    return response.data;
  },

  resetStats: async (): Promise<void> => {
    await axios.post(`${API_BASE_URL}/api/stats/reset`);
  },
};
