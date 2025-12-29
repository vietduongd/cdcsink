import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
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
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["connectors"] }),
  });

  const updateMutation = useMutation({
    mutationFn: ({
      name,
      connector,
    }: {
      name: string;
      connector: ConnectorConfigEntry;
    }) => api.updateConnector(name, connector),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["connectors"] }),
  });

  const deleteMutation = useMutation({
    mutationFn: api.deleteConnector,
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["connectors"] }),
  });

  const testMutation = useMutation({
    mutationFn: api.testConnector,
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
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["destinations"] }),
  });

  const updateMutation = useMutation({
    mutationFn: ({
      name,
      destination,
    }: {
      name: string;
      destination: DestinationConfigEntry;
    }) => api.updateDestination(name, destination),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["destinations"] }),
  });

  const deleteMutation = useMutation({
    mutationFn: api.deleteDestination,
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["destinations"] }),
  });

  const testMutation = useMutation({
    mutationFn: api.testDestination,
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
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["flows"] }),
  });

  const deleteMutation = useMutation({
    mutationFn: api.deleteFlow,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["flows"] }),
  });

  const startMutation = useMutation({
    mutationFn: api.startFlow,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["flows"] }),
  });

  const stopMutation = useMutation({
    mutationFn: api.stopFlow,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["flows"] }),
  });

  return { createMutation, deleteMutation, startMutation, stopMutation };
};
