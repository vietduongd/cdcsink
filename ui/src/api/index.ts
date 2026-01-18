import axios from "axios";

const API_BASE_URL = import.meta.env.VITE_API_URL || "http://localhost:4000";

// Standardized API Response format
interface ApiResponse<T> {
  data: T | null;
  message: string;
  code: string;
  errors: string[];
}

// Helper function to extract data from API response
const extractData = <T>(response: ApiResponse<T>): T => {
  if (response.code !== "SUCCESS") {
    const errorMessage =
      response.message || response.errors.join(", ") || "Unknown error";
    throw new Error(errorMessage);
  }
  if (response.data === null) {
    throw new Error("No data in response");
  }
  return response.data;
};

// Helper for operations without data
const checkSuccess = (response: ApiResponse<void>): void => {
  if (response.code !== "SUCCESS") {
    const errorMessage =
      response.message || response.errors.join(", ") || "Unknown error";
    throw new Error(errorMessage);
  }
};

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
  name: string;
  connector_type: string;
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
  schemas_includes?: string[];
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
  uptime_seconds?: number;
  records_processed?: number;
  messages_received?: number;
}

export const api = {
  getHealth: async (): Promise<HealthResponse> => {
    const response = await axios.get<ApiResponse<HealthResponse>>(
      `${API_BASE_URL}/health`,
    );
    return extractData(response.data);
  },

  getStats: async (): Promise<StatsResponse> => {
    const response = await axios.get<ApiResponse<StatsResponse>>(
      `${API_BASE_URL}/api/stats`,
    );
    return extractData(response.data);
  },

  resetStats: async (): Promise<void> => {
    const response = await axios.post<ApiResponse<void>>(
      `${API_BASE_URL}/api/stats/reset`,
    );
    checkSuccess(response.data);
  },

  // Connectors
  listConnectors: async (): Promise<ConnectorConfigEntry[]> => {
    const response = await axios.get<ApiResponse<ConnectorConfigEntry[]>>(
      `${API_BASE_URL}/api/connectors`,
    );
    return extractData(response.data);
  },

  getConnector: async (name: string): Promise<ConnectorConfigEntry> => {
    const response = await axios.get<ApiResponse<ConnectorConfigEntry>>(
      `${API_BASE_URL}/api/connectors/${name}`,
    );
    return extractData(response.data);
  },

  createConnector: async (connector: ConnectorConfigEntry): Promise<void> => {
    const response = await axios.post<ApiResponse<void>>(
      `${API_BASE_URL}/api/connectors`,
      connector,
    );
    checkSuccess(response.data);
  },

  updateConnector: async (
    name: string,
    connector: ConnectorConfigEntry,
  ): Promise<void> => {
    const response = await axios.put<ApiResponse<void>>(
      `${API_BASE_URL}/api/connectors/${name}`,
      connector,
    );
    checkSuccess(response.data);
  },

  deleteConnector: async (name: string): Promise<void> => {
    const response = await axios.delete<ApiResponse<void>>(
      `${API_BASE_URL}/api/connectors/${name}`,
    );
    checkSuccess(response.data);
  },

  testConnector: async (name: string): Promise<string> => {
    const response = await axios.post<ApiResponse<void>>(
      `${API_BASE_URL}/api/connectors/${name}/test`,
    );
    checkSuccess(response.data);
    return response.data.message;
  },

  testConnectorConfig: async (
    config: ConnectorConfigEntry,
  ): Promise<string> => {
    const response = await axios.post<ApiResponse<void>>(
      `${API_BASE_URL}/api/connectors/test-config`,
      config,
    );
    checkSuccess(response.data);
    return response.data.message;
  },

  // Destinations
  listDestinations: async (): Promise<DestinationConfigEntry[]> => {
    const response = await axios.get<ApiResponse<DestinationConfigEntry[]>>(
      `${API_BASE_URL}/api/destinations`,
    );
    return extractData(response.data);
  },

  getDestination: async (name: string): Promise<DestinationConfigEntry> => {
    const response = await axios.get<ApiResponse<DestinationConfigEntry>>(
      `${API_BASE_URL}/api/destinations/${name}`,
    );
    return extractData(response.data);
  },

  createDestination: async (
    destination: DestinationConfigEntry,
  ): Promise<void> => {
    const response = await axios.post<ApiResponse<void>>(
      `${API_BASE_URL}/api/destinations`,
      destination,
    );
    checkSuccess(response.data);
  },

  updateDestination: async (
    name: string,
    destination: DestinationConfigEntry,
  ): Promise<void> => {
    const response = await axios.put<ApiResponse<void>>(
      `${API_BASE_URL}/api/destinations/${name}`,
      destination,
    );
    checkSuccess(response.data);
  },

  deleteDestination: async (name: string): Promise<void> => {
    const response = await axios.delete<ApiResponse<void>>(
      `${API_BASE_URL}/api/destinations/${name}`,
    );
    checkSuccess(response.data);
  },

  testDestination: async (name: string): Promise<string> => {
    const response = await axios.post<ApiResponse<void>>(
      `${API_BASE_URL}/api/destinations/${name}/test`,
    );
    checkSuccess(response.data);
    return response.data.message;
  },

  testDestinationConfig: async (
    config: DestinationConfigEntry,
  ): Promise<string> => {
    const response = await axios.post<ApiResponse<void>>(
      `${API_BASE_URL}/api/destinations/test-config`,
      config,
    );
    checkSuccess(response.data);
    return response.data.message;
  },

  // Flows
  listFlows: async (): Promise<FlowConfigEntry[]> => {
    const response = await axios.get<ApiResponse<FlowConfigEntry[]>>(
      `${API_BASE_URL}/api/flows`,
    );
    return extractData(response.data);
  },

  getFlow: async (name: string): Promise<FlowConfigEntry> => {
    const response = await axios.get<ApiResponse<FlowConfigEntry>>(
      `${API_BASE_URL}/api/flows/${name}`,
    );
    return extractData(response.data);
  },

  createFlow: async (flow: FlowConfigEntry): Promise<void> => {
    const response = await axios.post<ApiResponse<void>>(
      `${API_BASE_URL}/api/flows`,
      flow,
    );
    checkSuccess(response.data);
  },

  deleteFlow: async (name: string): Promise<void> => {
    const response = await axios.delete<ApiResponse<void>>(
      `${API_BASE_URL}/api/flows/${name}`,
    );
    checkSuccess(response.data);
  },

  startFlow: async (name: string): Promise<void> => {
    const response = await axios.put<ApiResponse<void>>(
      `${API_BASE_URL}/api/flows/${name}/start`,
    );
    checkSuccess(response.data);
  },

  stopFlow: async (name: string): Promise<void> => {
    const response = await axios.put<ApiResponse<void>>(
      `${API_BASE_URL}/api/flows/${name}/stop`,
    );
    checkSuccess(response.data);
  },

  restartFlow: async (name: string): Promise<void> => {
    const response = await axios.put<ApiResponse<void>>(
      `${API_BASE_URL}/api/flows/${name}/restart`,
    );
    checkSuccess(response.data);
  },

  pauseFlow: async (name: string): Promise<void> => {
    const response = await axios.put<ApiResponse<void>>(
      `${API_BASE_URL}/api/flows/${name}/pause`,
    );
    checkSuccess(response.data);
  },

  resumeFlow: async (name: string): Promise<void> => {
    const response = await axios.put<ApiResponse<void>>(
      `${API_BASE_URL}/api/flows/${name}/resume`,
    );
    checkSuccess(response.data);
  },
};
