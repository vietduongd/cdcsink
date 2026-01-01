import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import toast from "react-hot-toast";
import { api } from "../api";
import type {
  ConnectorConfigEntry,
  DestinationConfigEntry,
  FlowConfigEntry,
} from "../api";

export const useStats = () => {
  return useQuery({
    queryKey: ["stats"],
    queryFn: api.getStats,
    refetchInterval: 2000,
  });
};

export const useHealth = () => {
  return useQuery({
    queryKey: ["health"],
    queryFn: api.getHealth,
    refetchInterval: 5000,
  });
};

// Connectors
export const useConnectors = () => {
  return useQuery({
    queryKey: ["connectors"],
    queryFn: api.listConnectors,
  });
};

export const useConnectorMutations = () => {
  const queryClient = useQueryClient();

  const createMutation = useMutation({
    mutationFn: api.createConnector,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["connectors"] });
      toast.success("Connector created successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to create connector");
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({
      name,
      connector,
    }: {
      name: string;
      connector: ConnectorConfigEntry;
    }) => api.updateConnector(name, connector),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["connectors"] });
      toast.success("Connector updated successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to update connector");
    },
  });

  const deleteMutation = useMutation({
    mutationFn: api.deleteConnector,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["connectors"] });
      toast.success("Connector deleted successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to delete connector");
    },
  });

  const testMutation = useMutation({
    mutationFn: api.testConnector,
    onError: (error: Error) => {
      toast.error(error.message || "Connection test failed");
    },
  });

  return { createMutation, updateMutation, deleteMutation, testMutation };
};

// Destinations
export const useDestinations = () => {
  return useQuery({
    queryKey: ["destinations"],
    queryFn: api.listDestinations,
  });
};

export const useDestinationMutations = () => {
  const queryClient = useQueryClient();

  const createMutation = useMutation({
    mutationFn: api.createDestination,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["destinations"] });
      toast.success("Destination created successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to create destination");
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({
      name,
      destination,
    }: {
      name: string;
      destination: DestinationConfigEntry;
    }) => api.updateDestination(name, destination),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["destinations"] });
      toast.success("Destination updated successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to update destination");
    },
  });

  const deleteMutation = useMutation({
    mutationFn: api.deleteDestination,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["destinations"] });
      toast.success("Destination deleted successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to delete destination");
    },
  });

  const testMutation = useMutation({
    mutationFn: api.testDestination,
    onError: (error: Error) => {
      toast.error(error.message || "Connection test failed");
    },
  });

  return { createMutation, updateMutation, deleteMutation, testMutation };
};

// Flows
export const useFlows = () => {
  return useQuery({
    queryKey: ["flows"],
    queryFn: api.listFlows,
    refetchInterval: 5000,
  });
};

export const useFlowMutations = () => {
  const queryClient = useQueryClient();

  const createMutation = useMutation({
    mutationFn: api.createFlow,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["flows"] });
      toast.success("Flow created successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to create flow");
    },
  });

  const deleteMutation = useMutation({
    mutationFn: api.deleteFlow,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["flows"] });
      toast.success("Flow deleted successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to delete flow");
    },
  });

  const startMutation = useMutation({
    mutationFn: api.startFlow,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["flows"] });
      toast.success("Flow started successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to start flow");
    },
  });

  const stopMutation = useMutation({
    mutationFn: api.stopFlow,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["flows"] });
      toast.success("Flow stopped successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to stop flow");
    },
  });

  const restartMutation = useMutation({
    mutationFn: api.restartFlow,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["flows"] });
      toast.success("Flow restarted successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to restart flow");
    },
  });

  const pauseMutation = useMutation({
    mutationFn: api.pauseFlow,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["flows"] });
      toast.success("Flow paused successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to pause flow");
    },
  });

  const resumeMutation = useMutation({
    mutationFn: api.resumeFlow,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["flows"] });
      toast.success("Flow resumed successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to resume flow");
    },
  });

  return {
    createMutation,
    deleteMutation,
    startMutation,
    stopMutation,
    restartMutation,
    pauseMutation,
    resumeMutation,
  };
};
