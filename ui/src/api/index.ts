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

export interface ConnectorConfigEntry {
  name: String;
  connector_type: String;
  config: any;
  description?: string;
  tags?: string[];
  created_at?: string;
  updated_at?: string;
}

export interface DestinationConfigEntry {
  name: String;
  destination_type: String;
  config: any;
  description?: string;
  tags?: string[];
  created_at?: string;
  updated_at?: string;
}

export interface FlowConfigEntry {
  name: String;
  connector_name: String;
  destination_names: string[];
  batch_size: number;
  auto_start: boolean;
  active?: boolean;
  description?: string;
  created_at?: string;
  updated_at?: string;
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

  // Connectors
  listConnectors: async (): Promise<ConnectorConfigEntry[]> => {
    const response = await axios.get<ConnectorConfigEntry[]>(
      `${API_BASE_URL}/api/connectors`
    );
    return response.data;
  },

  getConnector: async (name: string): Promise<ConnectorConfigEntry> => {
    const response = await axios.get<ConnectorConfigEntry>(
      `${API_BASE_URL}/api/connectors/${name}`
    );
    return response.data;
  },

  createConnector: async (connector: ConnectorConfigEntry): Promise<void> => {
    await axios.post(`${API_BASE_URL}/api/connectors`, connector);
  },

  updateConnector: async (
    name: string,
    connector: ConnectorConfigEntry
  ): Promise<void> => {
    await axios.put(`${API_BASE_URL}/api/connectors/${name}`, connector);
  },

  deleteConnector: async (name: string): Promise<void> => {
    await axios.delete(`${API_BASE_URL}/api/connectors/${name}`);
  },

  testConnector: async (name: string): Promise<string> => {
    const response = await axios.post(
      `${API_BASE_URL}/api/connectors/${name}/test`
    );
    return response.data;
  },

  testConnectorConfig: async (
    config: ConnectorConfigEntry
  ): Promise<string> => {
    const response = await axios.post(
      `${API_BASE_URL}/api/connectors/test-config`,
      config
    );
    return response.data;
  },

  // Destinations
  listDestinations: async (): Promise<DestinationConfigEntry[]> => {
    const response = await axios.get<DestinationConfigEntry[]>(
      `${API_BASE_URL}/api/destinations`
    );
    return response.data;
  },

  getDestination: async (name: string): Promise<DestinationConfigEntry> => {
    const response = await axios.get<DestinationConfigEntry>(
      `${API_BASE_URL}/api/destinations/${name}`
    );
    return response.data;
  },

  createDestination: async (
    destination: DestinationConfigEntry
  ): Promise<void> => {
    await axios.post(`${API_BASE_URL}/api/destinations`, destination);
  },

  updateDestination: async (
    name: string,
    destination: DestinationConfigEntry
  ): Promise<void> => {
    await axios.put(`${API_BASE_URL}/api/destinations/${name}`, destination);
  },

  deleteDestination: async (name: string): Promise<void> => {
    await axios.delete(`${API_BASE_URL}/api/destinations/${name}`);
  },

  testDestination: async (name: string): Promise<string> => {
    const response = await axios.post(
      `${API_BASE_URL}/api/destinations/${name}/test`
    );
    return response.data;
  },

  testDestinationConfig: async (
    config: DestinationConfigEntry
  ): Promise<string> => {
    const response = await axios.post(
      `${API_BASE_URL}/api/destinations/test-config`,
      config
    );
    return response.data;
  },

  // Flows
  listFlows: async (): Promise<FlowConfigEntry[]> => {
    const response = await axios.get<FlowConfigEntry[]>(
      `${API_BASE_URL}/api/flows`
    );
    return response.data;
  },

  getFlow: async (name: string): Promise<FlowConfigEntry> => {
    const response = await axios.get<FlowConfigEntry>(
      `${API_BASE_URL}/api/flows/${name}`
    );
    return response.data;
  },

  createFlow: async (flow: FlowConfigEntry): Promise<void> => {
    await axios.post(`${API_BASE_URL}/api/flows`, flow);
  },

  deleteFlow: async (name: string): Promise<void> => {
    await axios.delete(`${API_BASE_URL}/api/flows/${name}`);
  },

  startFlow: async (name: string): Promise<void> => {
    await axios.put(`${API_BASE_URL}/api/flows/${name}/start`);
  },

  stopFlow: async (name: string): Promise<void> => {
    await axios.put(`${API_BASE_URL}/api/flows/${name}/stop`);
  },
};
