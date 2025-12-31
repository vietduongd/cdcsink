import React, { useState, useEffect } from "react";
import { useConnectors, useConnectorMutations } from "../hooks/useQueries";
import type { ConnectorConfigEntry } from "../api";
import {
  Plus,
  Edit2,
  Trash2,
  Play,
  Cable,
  X,
  Check,
  AlertCircle,
  Loader2,
} from "lucide-react";
import { api } from "../api";

export const ConnectorsPage: React.FC = () => {
  const { data: connectors, isLoading } = useConnectors();
  const { deleteMutation, testMutation } = useConnectorMutations();
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [editingConnector, setEditingConnector] =
    useState<ConnectorConfigEntry | null>(null);

  const handleDelete = (name: string) => {
    if (
      window.confirm(`Are you sure you want to delete connector "${name}"?`)
    ) {
      deleteMutation.mutate(name);
    }
  };

  if (isLoading)
    return (
      <div className="flex items-center justify-center p-20">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-600"></div>
      </div>
    );

  return (
    <div className="space-y-8 animate-in fade-in duration-500">
      <div className="flex justify-between items-end">
        <div>
          <h1 className="text-2xl font-bold text-slate-900 tracking-tight">
            Data Connectors
          </h1>
          <p className="text-xs font-bold text-slate-500 uppercase tracking-widest mt-1">
            Source configuration management
          </p>
        </div>
        <button
          className="flex items-center gap-2 px-4 py-2 bg-indigo-600 rounded-lg text-white text-xs font-bold hover:bg-indigo-700 transition-all active:scale-95 shadow-sm shadow-indigo-100"
          onClick={() => {
            setEditingConnector(null);
            setIsFormOpen(true);
          }}
        >
          <Plus size={16} />
          Add Connector
        </button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
        {connectors?.map((connector) => (
          <div
            key={connector.name.toString()}
            className="bg-white rounded-xl border border-slate-200 overflow-hidden flex flex-col group hover:border-indigo-300 transition-colors"
          >
            <div className="px-6 py-4 border-b border-slate-100 flex justify-between items-center bg-slate-50/30">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded bg-white border border-slate-100 flex items-center justify-center overflow-hidden p-1.5 shadow-sm">
                  {connector.connector_type === "nats" && (
                    <img
                      src="/logos/nats.svg"
                      className="w-full h-full object-contain"
                      alt="NATS"
                    />
                  )}
                  {connector.connector_type === "kafka" && (
                    <img
                      src="/logos/kafka.svg"
                      className="w-full h-full object-contain"
                      alt="Kafka"
                    />
                  )}
                  {connector.connector_type === "postgres_cdc" && (
                    <img
                      src="/logos/postgresql.svg"
                      className="w-full h-full object-contain"
                      alt="Postgres"
                    />
                  )}
                  {!["nats", "kafka", "postgres_cdc"].includes(
                    connector.connector_type.toString()
                  ) && <Cable size={18} className="text-indigo-600" />}
                </div>
                <div>
                  <h3 className="text-sm font-bold text-slate-900">
                    {connector.name}
                  </h3>
                  <span className="text-[10px] font-bold uppercase tracking-widest text-slate-400">
                    {connector.connector_type}
                  </span>
                </div>
              </div>
              <div className="flex gap-1">
                <button
                  className="p-2 text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 rounded transition-colors"
                  onClick={() => {
                    setEditingConnector(connector);
                    setIsFormOpen(true);
                  }}
                >
                  <Edit2 size={16} />
                </button>
                <button
                  className="p-2 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded transition-colors"
                  onClick={() => handleDelete(connector.name.toString())}
                >
                  <Trash2 size={16} />
                </button>
              </div>
            </div>

            <div className="p-6 flex-1">
              <p className="text-xs text-slate-500 font-medium line-clamp-2 min-h-[2rem]">
                {connector.description ||
                  "No description provided for this connector."}
              </p>

              <div className="mt-4 flex flex-wrap gap-1">
                {connector.tags?.map((tag) => (
                  <span
                    key={tag}
                    className="px-2 py-0.5 bg-slate-100 rounded text-[9px] font-bold text-slate-500 uppercase"
                  >
                    {tag}
                  </span>
                ))}
              </div>
            </div>

            <div className="px-6 py-4 border-t border-slate-100 bg-slate-50/20 flex justify-between items-center">
              <button
                className="text-[10px] font-bold text-indigo-600 hover:text-indigo-700 flex items-center gap-1.5"
                onClick={() => testMutation.mutate(connector.name.toString())}
              >
                <Play size={12} fill="currentColor" />
                Test Connection
              </button>
              <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest">
                v1.0.4
              </div>
            </div>
          </div>
        ))}
      </div>

      {isFormOpen && (
        <ConnectorForm
          connector={editingConnector}
          onClose={() => setIsFormOpen(false)}
        />
      )}
    </div>
  );
};

// Helper function to get default config for each connector type
const getDefaultConfig = (connectorType: string): Record<string, any> => {
  switch (connectorType) {
    case "nats":
      return {
        servers: "nats://localhost:4222",
        subject: "cdc.events",
        use_jetstream: false,
        connection_timeout: 30,
        max_reconnect_attempts: 10,
        reconnect_wait: 2,
        ping_interval: 120,
        tls_enabled: false,
      };
    case "kafka":
      return {
        topic: "cdc-events",
        group_id: "cdc-consumer",
        auto_offset_reset: "earliest",
        session_timeout_ms: 10000,
        max_poll_records: 500,
        enable_auto_commit: true,
        security_protocol: "PLAINTEXT",
      };
    case "postgres_cdc":
      return {
        host: "localhost",
        port: 5432,
        username: "postgres",
        slot_name: "cdc_slot",
        publication_name: "cdc_publication",
        poll_interval_ms: 1000,
        snapshot_mode: "initial",
      };
    default:
      return {};
  }
};

const ConnectorForm: React.FC<{
  connector: ConnectorConfigEntry | null;
  onClose: () => void;
}> = ({ connector, onClose }) => {
  const { createMutation, updateMutation } = useConnectorMutations();

  const [formData, setFormData] = useState({
    name: connector?.name || "",
    connector_type: connector?.connector_type || "nats",
    description: connector?.description || "",
  });

  // Initialize config with defaults merged with existing config
  const [config, setConfig] = useState<Record<string, any>>(() => {
    const defaults = getDefaultConfig(connector?.connector_type || "nats");
    const existingConfig = (connector?.config as Record<string, any>) || {};
    return { ...defaults, ...existingConfig };
  });
  const [tags, setTags] = useState<string[]>(connector?.tags || []);
  const [tagInput, setTagInput] = useState("");
  const [testStatus, setTestStatus] = useState<
    "idle" | "loading" | "success" | "error"
  >("idle");
  const [testMessage, setTestMessage] = useState("");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [authType, setAuthType] = useState<"none" | "userpass" | "token">(
    "none"
  );

  const updateConfig = (key: string, value: any) => {
    setConfig((prev) => ({ ...prev, [key]: value }));
  };

  const addTag = () => {
    if (tagInput.trim() && !tags.includes(tagInput.trim())) {
      setTags([...tags, tagInput.trim()]);
      setTagInput("");
    }
  };

  const removeTag = (t: string) => setTags(tags.filter((tag) => tag !== t));

  const handleTestConnection = async () => {
    setTestStatus("loading");
    setTestMessage("");

    let finalConfig = { ...config };
    if (
      formData.connector_type === "nats" &&
      typeof finalConfig.servers === "string"
    ) {
      finalConfig.servers = finalConfig.servers
        .split(",")
        .map((s: string) => s.trim());
    }
    if (
      formData.connector_type === "kafka" &&
      typeof finalConfig.brokers === "string"
    ) {
      finalConfig.brokers = finalConfig.brokers
        .split(",")
        .map((b: string) => b.trim());
    }

    const testConfig: ConnectorConfigEntry = {
      name: formData.name || "test",
      connector_type: formData.connector_type,
      config: finalConfig,
      description: formData.description,
      tags,
    };

    try {
      const result = await api.testConnectorConfig(testConfig);
      setTestStatus("success");
      setTestMessage(result || "Connection successful!");
    } catch (error: any) {
      setTestStatus("error");
      const errorMessage =
        error.response?.data?.message || error.message || "Connection failed";
      setTestMessage(errorMessage);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    let finalConfig = { ...config };
    if (
      formData.connector_type === "nats" &&
      typeof finalConfig.servers === "string"
    ) {
      finalConfig.servers = finalConfig.servers
        .split(",")
        .map((s: string) => s.trim());
    }
    if (
      formData.connector_type === "kafka" &&
      typeof finalConfig.brokers === "string"
    ) {
      finalConfig.brokers = finalConfig.brokers
        .split(",")
        .map((b: string) => b.trim());
    }

    const payload = {
      ...formData,
      config: finalConfig,
      tags,
    };

    if (connector) {
      updateMutation.mutate(
        { name: connector.name.toString(), connector: payload as any },
        { onSuccess: onClose }
      );
    } else {
      createMutation.mutate(payload as any, { onSuccess: onClose });
    }
  };

  return (
    <div className="fixed inset-0 z-100 flex items-center justify-center p-6 sm:p-12 animate-in fade-in duration-300">
      <div
        className="absolute inset-0 bg-slate-900/40 backdrop-blur-sm"
        onClick={onClose}
      />
      <div className="relative bg-white w-full max-w-2xl rounded-xl shadow-2xl p-8 animate-in zoom-in-95 duration-300 border border-slate-200 flex flex-col max-h-[90vh]">
        <h2 className="text-xl font-bold text-slate-900 mb-6 sticky top-0 bg-white pb-2 flex justify-between items-center z-10">
          <span>{connector ? "Edit Connector" : "Add New Connector"}</span>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-600 transition-colors"
          >
            <X size={20} />
          </button>
        </h2>

        <form
          onSubmit={handleSubmit}
          className="space-y-6 overflow-y-auto pr-2 pb-4 scrollbar-thin"
        >
          <div className="grid grid-cols-2 gap-6">
            <div className="space-y-2">
              <label className="text-[10px] font-bold text-slate-400 uppercase tracking-wider ml-1">
                Connector Name
              </label>
              <input
                type="text"
                required
                disabled={!!connector}
                className="w-full px-4 py-2.5 bg-slate-50 border border-slate-200 rounded-lg text-sm font-bold text-slate-900 focus:outline-none focus:border-indigo-500 transition-all disabled:opacity-50"
                value={formData.name.toString()}
                onChange={(e) =>
                  setFormData({ ...formData, name: e.target.value })
                }
              />
            </div>

            <div className="space-y-2">
              <label className="text-[10px] font-bold text-slate-400 uppercase tracking-wider ml-1">
                Type
              </label>
              <select
                className="w-full px-4 py-2.5 bg-slate-50 border border-slate-200 rounded-lg text-sm font-bold text-slate-900 focus:outline-none focus:border-indigo-500 transition-all"
                value={formData.connector_type.toString()}
                onChange={(e) =>
                  setFormData({ ...formData, connector_type: e.target.value })
                }
              >
                <option value="nats">NATS JetStream</option>
                <option value="kafka">Apache Kafka</option>
                <option value="postgres_cdc">PostgreSQL CDC</option>
              </select>
            </div>
          </div>

          <div className="space-y-2">
            <label className="text-[10px] font-bold text-slate-400 uppercase tracking-wider ml-1">
              Description
            </label>
            <textarea
              rows={2}
              className="w-full px-4 py-2 bg-slate-50 border border-slate-200 rounded-lg text-sm font-medium text-slate-900 focus:outline-none focus:border-indigo-500 transition-all resize-none"
              value={formData.description}
              onChange={(e) =>
                setFormData({ ...formData, description: e.target.value })
              }
              placeholder="System description or notes..."
            />
          </div>

          <div className="pt-2">
            <div className="text-[10px] font-bold text-indigo-600 uppercase tracking-wider mb-4 border-b border-indigo-100 pb-2">
              Configuration Specifics
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              {formData.connector_type === "nats" && (
                <>
                  <div className="space-y-2 md:col-span-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Server URLs (comma-separated)
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={
                        Array.isArray(config.servers)
                          ? config.servers.join(", ")
                          : config.servers || "nats://localhost:4222"
                      }
                      onChange={(e) => updateConfig("servers", e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Subject
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.subject || "cdc.events"}
                      onChange={(e) => updateConfig("subject", e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Consumer Group
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.consumer_group || ""}
                      onChange={(e) =>
                        updateConfig("consumer_group", e.target.value)
                      }
                      placeholder="Optional"
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Consumer Name
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.consumer_name || ""}
                      onChange={(e) =>
                        updateConfig("consumer_name", e.target.value)
                      }
                      placeholder="Optional (durable name for JetStream)"
                    />
                  </div>
                  <div className="flex items-center gap-3 pt-2">
                    <input
                      type="checkbox"
                      id="use_jetstream"
                      className="w-4 h-4 text-indigo-600 rounded border-slate-300 focus:ring-indigo-500"
                      checked={config.use_jetstream || false}
                      onChange={(e) =>
                        updateConfig("use_jetstream", e.target.checked)
                      }
                    />
                    <label
                      htmlFor="use_jetstream"
                      className="text-xs font-bold text-slate-700"
                    >
                      Enable JetStream
                    </label>
                  </div>

                  {/* Advanced Settings for NATS */}
                  <div className="md:col-span-2 mt-4">
                    <button
                      type="button"
                      onClick={() => setShowAdvanced(!showAdvanced)}
                      className="flex items-center gap-2 text-xs font-bold text-indigo-600 hover:text-indigo-700 transition-colors"
                    >
                      <span>{showAdvanced ? "▼" : "▶"}</span>
                      Advanced Settings
                    </button>

                    {showAdvanced && (
                      <div className="mt-4 p-4 bg-slate-50/50 rounded-lg border border-slate-200 space-y-4">
                        <div className="space-y-2">
                          <label className="text-[10px] font-bold text-slate-500 uppercase">
                            Authentication
                          </label>
                          <select
                            className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                            value={authType}
                            onChange={(e) => {
                              const newAuthType = e.target.value as
                                | "none"
                                | "userpass"
                                | "token";
                              setAuthType(newAuthType);
                              if (newAuthType === "none") {
                                updateConfig("username", undefined);
                                updateConfig("password", undefined);
                                updateConfig("token", undefined);
                              }
                            }}
                          >
                            <option value="none">None</option>
                            <option value="userpass">Username/Password</option>
                            <option value="token">Token</option>
                          </select>
                        </div>

                        {authType === "userpass" && (
                          <>
                            <div className="space-y-2">
                              <label className="text-[10px] font-bold text-slate-500 uppercase">
                                Username
                              </label>
                              <input
                                type="text"
                                className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                                value={config.username || ""}
                                onChange={(e) =>
                                  updateConfig("username", e.target.value)
                                }
                                placeholder="Enter username"
                              />
                            </div>
                            <div className="space-y-2">
                              <label className="text-[10px] font-bold text-slate-500 uppercase">
                                Password
                              </label>
                              <input
                                type="password"
                                className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                                value={config.password || ""}
                                onChange={(e) =>
                                  updateConfig("password", e.target.value)
                                }
                                placeholder="Enter password"
                              />
                            </div>
                          </>
                        )}

                        {authType === "token" && (
                          <div className="space-y-2">
                            <label className="text-[10px] font-bold text-slate-500 uppercase">
                              Token
                            </label>
                            <input
                              type="password"
                              className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                              value={config.token || ""}
                              onChange={(e) =>
                                updateConfig("token", e.target.value)
                              }
                              placeholder="Enter authentication token"
                            />
                          </div>
                        )}

                        {/* Connection Settings */}
                        <div className="pt-4 border-t border-slate-200">
                          <div className="text-[10px] font-bold text-slate-600 uppercase mb-3">
                            Connection Settings
                          </div>
                          <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-2">
                              <label className="text-[10px] font-bold text-slate-500 uppercase">
                                Connection Timeout (s)
                              </label>
                              <input
                                type="number"
                                className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                                value={config.connection_timeout || 30}
                                onChange={(e) =>
                                  updateConfig(
                                    "connection_timeout",
                                    parseInt(e.target.value)
                                  )
                                }
                                min="1"
                              />
                            </div>
                            <div className="space-y-2">
                              <label className="text-[10px] font-bold text-slate-500 uppercase">
                                Max Reconnect Attempts
                              </label>
                              <input
                                type="number"
                                className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                                value={config.max_reconnect_attempts || 10}
                                onChange={(e) =>
                                  updateConfig(
                                    "max_reconnect_attempts",
                                    parseInt(e.target.value)
                                  )
                                }
                                min="0"
                              />
                            </div>
                            <div className="space-y-2">
                              <label className="text-[10px] font-bold text-slate-500 uppercase">
                                Reconnect Wait (s)
                              </label>
                              <input
                                type="number"
                                className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                                value={config.reconnect_wait || 2}
                                onChange={(e) =>
                                  updateConfig(
                                    "reconnect_wait",
                                    parseInt(e.target.value)
                                  )
                                }
                                min="1"
                              />
                            </div>
                            <div className="space-y-2">
                              <label className="text-[10px] font-bold text-slate-500 uppercase">
                                Ping Interval (s)
                              </label>
                              <input
                                type="number"
                                className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                                value={config.ping_interval || 120}
                                onChange={(e) =>
                                  updateConfig(
                                    "ping_interval",
                                    parseInt(e.target.value)
                                  )
                                }
                                min="1"
                              />
                            </div>
                          </div>
                          <div className="flex items-center gap-3 mt-4">
                            <input
                              type="checkbox"
                              id="tls_enabled"
                              className="w-4 h-4 text-indigo-600 rounded border-slate-300 focus:ring-indigo-500"
                              checked={config.tls_enabled || false}
                              onChange={(e) =>
                                updateConfig("tls_enabled", e.target.checked)
                              }
                            />
                            <label
                              htmlFor="tls_enabled"
                              className="text-xs font-bold text-slate-700"
                            >
                              Enable TLS/SSL
                            </label>
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                </>
              )}

              {formData.connector_type === "kafka" && (
                <>
                  <div className="space-y-2 md:col-span-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Brokers (comma-separated)
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={
                        Array.isArray(config.brokers)
                          ? config.brokers.join(", ")
                          : config.brokers || "localhost:9092"
                      }
                      onChange={(e) => updateConfig("brokers", e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Topic
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.topic || "cdc-events"}
                      onChange={(e) => updateConfig("topic", e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Group ID
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.group_id || "cdc-consumer"}
                      onChange={(e) => updateConfig("group_id", e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Offset Reset
                    </label>
                    <select
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.auto_offset_reset || "earliest"}
                      onChange={(e) =>
                        updateConfig("auto_offset_reset", e.target.value)
                      }
                    >
                      <option value="earliest">Earliest</option>
                      <option value="latest">Latest</option>
                    </select>
                  </div>

                  {/* Advanced Settings for Kafka */}
                  <div className="md:col-span-2 mt-4">
                    <button
                      type="button"
                      onClick={() => setShowAdvanced(!showAdvanced)}
                      className="flex items-center gap-2 text-xs font-bold text-indigo-600 hover:text-indigo-700 transition-colors"
                    >
                      <span>{showAdvanced ? "▼" : "▶"}</span>
                      Advanced Settings
                    </button>

                    {showAdvanced && (
                      <div className="mt-4 p-4 bg-slate-50/50 rounded-lg border border-slate-200 space-y-4">
                        <div className="grid grid-cols-2 gap-4">
                          <div className="space-y-2">
                            <label className="text-[10px] font-bold text-slate-500 uppercase">
                              Session Timeout (ms)
                            </label>
                            <input
                              type="number"
                              className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                              value={config.session_timeout_ms || 10000}
                              onChange={(e) =>
                                updateConfig(
                                  "session_timeout_ms",
                                  parseInt(e.target.value)
                                )
                              }
                              min="1"
                            />
                          </div>
                          <div className="space-y-2">
                            <label className="text-[10px] font-bold text-slate-500 uppercase">
                              Max Poll Records
                            </label>
                            <input
                              type="number"
                              className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                              value={config.max_poll_records || 500}
                              onChange={(e) =>
                                updateConfig(
                                  "max_poll_records",
                                  parseInt(e.target.value)
                                )
                              }
                              min="1"
                            />
                          </div>
                        </div>

                        <div className="flex items-center gap-3">
                          <input
                            type="checkbox"
                            id="enable_auto_commit"
                            className="w-4 h-4 text-indigo-600 rounded border-slate-300 focus:ring-indigo-500"
                            checked={config.enable_auto_commit !== false}
                            onChange={(e) =>
                              updateConfig(
                                "enable_auto_commit",
                                e.target.checked
                              )
                            }
                          />
                          <label
                            htmlFor="enable_auto_commit"
                            className="text-xs font-bold text-slate-700"
                          >
                            Enable Auto Commit
                          </label>
                        </div>

                        <div className="pt-4 border-t border-slate-200">
                          <div className="text-[10px] font-bold text-slate-600 uppercase mb-3">
                            Security Settings
                          </div>
                          <div className="space-y-4">
                            <div className="space-y-2">
                              <label className="text-[10px] font-bold text-slate-500 uppercase">
                                Security Protocol
                              </label>
                              <select
                                className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                                value={config.security_protocol || "PLAINTEXT"}
                                onChange={(e) =>
                                  updateConfig(
                                    "security_protocol",
                                    e.target.value
                                  )
                                }
                              >
                                <option value="PLAINTEXT">PLAINTEXT</option>
                                <option value="SSL">SSL</option>
                                <option value="SASL_PLAINTEXT">
                                  SASL_PLAINTEXT
                                </option>
                                <option value="SASL_SSL">SASL_SSL</option>
                              </select>
                            </div>

                            {(config.security_protocol === "SASL_PLAINTEXT" ||
                              config.security_protocol === "SASL_SSL") && (
                              <>
                                <div className="space-y-2">
                                  <label className="text-[10px] font-bold text-slate-500 uppercase">
                                    SASL Mechanism
                                  </label>
                                  <select
                                    className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                                    value={config.sasl_mechanism || "PLAIN"}
                                    onChange={(e) =>
                                      updateConfig(
                                        "sasl_mechanism",
                                        e.target.value
                                      )
                                    }
                                  >
                                    <option value="PLAIN">PLAIN</option>
                                    <option value="SCRAM-SHA-256">
                                      SCRAM-SHA-256
                                    </option>
                                    <option value="SCRAM-SHA-512">
                                      SCRAM-SHA-512
                                    </option>
                                  </select>
                                </div>
                                <div className="grid grid-cols-2 gap-4">
                                  <div className="space-y-2">
                                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                                      SASL Username
                                    </label>
                                    <input
                                      type="text"
                                      className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                                      value={config.sasl_username || ""}
                                      onChange={(e) =>
                                        updateConfig(
                                          "sasl_username",
                                          e.target.value
                                        )
                                      }
                                      placeholder="Enter username"
                                    />
                                  </div>
                                  <div className="space-y-2">
                                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                                      SASL Password
                                    </label>
                                    <input
                                      type="password"
                                      className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                                      value={config.sasl_password || ""}
                                      onChange={(e) =>
                                        updateConfig(
                                          "sasl_password",
                                          e.target.value
                                        )
                                      }
                                      placeholder="Enter password"
                                    />
                                  </div>
                                </div>
                              </>
                            )}
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                </>
              )}

              {formData.connector_type === "postgres_cdc" && (
                <>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Host
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.host || "localhost"}
                      onChange={(e) => updateConfig("host", e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Port
                    </label>
                    <input
                      type="number"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.port || 5432}
                      onChange={(e) =>
                        updateConfig("port", parseInt(e.target.value))
                      }
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      User
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.username || "postgres"}
                      onChange={(e) => updateConfig("username", e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Database
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.database || ""}
                      onChange={(e) => updateConfig("database", e.target.value)}
                    />
                  </div>

                  {/* Advanced Settings for PostgreSQL CDC */}
                  <div className="md:col-span-2 mt-4">
                    <button
                      type="button"
                      onClick={() => setShowAdvanced(!showAdvanced)}
                      className="flex items-center gap-2 text-xs font-bold text-indigo-600 hover:text-indigo-700 transition-colors"
                    >
                      <span>{showAdvanced ? "▼" : "▶"}</span>
                      Advanced Settings
                    </button>

                    {showAdvanced && (
                      <div className="mt-4 p-4 bg-slate-50/50 rounded-lg border border-slate-200 space-y-4">
                        <div className="grid grid-cols-2 gap-4">
                          <div className="space-y-2">
                            <label className="text-[10px] font-bold text-slate-500 uppercase">
                              Slot Name
                            </label>
                            <input
                              type="text"
                              className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                              value={config.slot_name || "cdc_slot"}
                              onChange={(e) =>
                                updateConfig("slot_name", e.target.value)
                              }
                              placeholder="Replication slot name"
                            />
                          </div>
                          <div className="space-y-2">
                            <label className="text-[10px] font-bold text-slate-500 uppercase">
                              Publication Name
                            </label>
                            <input
                              type="text"
                              className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                              value={
                                config.publication_name || "cdc_publication"
                              }
                              onChange={(e) =>
                                updateConfig("publication_name", e.target.value)
                              }
                              placeholder="Publication name"
                            />
                          </div>
                          <div className="space-y-2">
                            <label className="text-[10px] font-bold text-slate-500 uppercase">
                              Poll Interval (ms)
                            </label>
                            <input
                              type="number"
                              className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                              value={config.poll_interval_ms || 1000}
                              onChange={(e) =>
                                updateConfig(
                                  "poll_interval_ms",
                                  parseInt(e.target.value)
                                )
                              }
                              min="100"
                            />
                          </div>
                          <div className="space-y-2">
                            <label className="text-[10px] font-bold text-slate-500 uppercase">
                              Snapshot Mode
                            </label>
                            <select
                              className="w-full px-3 py-2 bg-white border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                              value={config.snapshot_mode || "initial"}
                              onChange={(e) =>
                                updateConfig("snapshot_mode", e.target.value)
                              }
                            >
                              <option value="initial">Initial</option>
                              <option value="never">Never</option>
                              <option value="always">Always</option>
                              <option value="when_needed">When Needed</option>
                            </select>
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                </>
              )}
            </div>
          </div>

          <div className="space-y-2 pt-2">
            <label className="text-[10px] font-bold text-slate-400 uppercase tracking-wider ml-1">
              Tags
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                className="flex-1 px-4 py-2 bg-slate-50 border border-slate-200 rounded-lg text-xs font-bold text-slate-900 focus:outline-none focus:border-indigo-500 transition-all"
                value={tagInput}
                onChange={(e) => setTagInput(e.target.value)}
                onKeyDown={(e) =>
                  e.key === "Enter" && (e.preventDefault(), addTag())
                }
                placeholder="Press Enter to add tag..."
              />
              <button
                type="button"
                onClick={addTag}
                className="px-4 py-2 bg-slate-100 text-slate-600 rounded-lg text-xs font-bold hover:bg-slate-200 transition-all font-mono"
              >
                +
              </button>
            </div>
            <div className="flex flex-wrap gap-1.5 mt-2">
              {tags.map((t) => (
                <span
                  key={t}
                  className="flex items-center gap-1 px-2 py-1 bg-indigo-50 border border-indigo-100 text-indigo-700 rounded text-[10px] font-bold"
                >
                  {t}
                  <button
                    type="button"
                    onClick={() => removeTag(t)}
                    className="hover:text-rose-600"
                  >
                    <X size={10} strokeWidth={3} />
                  </button>
                </span>
              ))}
            </div>
          </div>

          {testStatus !== "idle" && (
            <div
              className={`p-3 rounded-lg flex items-center gap-2 text-sm font-medium ${
                testStatus === "success"
                  ? "bg-emerald-50 text-emerald-700 border border-emerald-200"
                  : testStatus === "error"
                  ? "bg-rose-50 text-rose-700 border border-rose-200"
                  : "bg-slate-50 text-slate-700 border border-slate-200"
              }`}
            >
              {testStatus === "loading" && (
                <Loader2 size={16} className="animate-spin" />
              )}
              {testStatus === "success" && <Check size={16} />}
              {testStatus === "error" && <AlertCircle size={16} />}
              <span>{testMessage || "Testing connection..."}</span>
            </div>
          )}

          <div className="flex gap-3 pt-6 border-t border-slate-100 sticky bottom-0 bg-white shadow-[0_-10px_10px_-10px_rgba(0,0,0,0.05)]">
            <button
              type="button"
              className="px-6 py-2.5 bg-white border border-slate-200 rounded-lg text-sm font-bold text-slate-600 hover:bg-slate-50 transition-all"
              onClick={onClose}
            >
              Cancel
            </button>
            <button
              type="button"
              className="px-6 py-2.5 bg-amber-50 border border-amber-200 rounded-lg text-sm font-bold text-amber-700 hover:bg-amber-100 transition-all flex items-center gap-2"
              onClick={handleTestConnection}
              disabled={testStatus === "loading"}
            >
              {testStatus === "loading" ? (
                <Loader2 size={16} className="animate-spin" />
              ) : (
                <Play size={16} />
              )}
              Test Connection
            </button>
            <button
              type="submit"
              className="flex-1 px-4 py-2.5 bg-indigo-600 rounded-lg text-white font-bold text-sm hover:bg-indigo-700 transition-all shadow-sm shadow-indigo-100"
            >
              {connector ? "Save Changes" : "Create Connector"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
