import { useQuery } from "@tanstack/react-query";
import { api } from "../api";

export const useStats = () => {
  return useQuery({
    queryKey: ["stats"],
    queryFn: api.getStats,
    refetchInterval: 2000, // Refresh every 2 seconds
  });
};

export const useHealth = () => {
  return useQuery({
    queryKey: ["health"],
    queryFn: api.getHealth,
    refetchInterval: 5000, // Refresh every 5 seconds
  });
};
