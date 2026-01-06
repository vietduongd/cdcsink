import {
  Activity,
  ArrowRight,
  Cable,
  CheckCircle2,
  Clock,
  Database,
  Info,
  Play,
  Plus,
  RotateCw,
  Square,
  Trash2,
  TrendingUp,
} from "lucide-react";
import React, { useState } from "react";
import {
  useConnectors,
  useDestinations,
  useFlowMutations,
  useFlows,
} from "../hooks/useQueries";

const getConnectorLogo = (type?: string) => {
  switch (type) {
    case "nats":
      return "/logos/nats.svg";
    case "kafka":
      return "/logos/kafka.svg";
    case "postgres_cdc":
      return "/logos/postgresql.svg";
    default:
      return null;
  }
};

const getDestinationLogo = (type?: string) => {
  switch (type) {
    case "postgres":
      return "/logos/postgresql.svg";
    case "mysql":
      return "/logos/mysql.svg";
    case "clickhouse":
      return "/logos/clickhouse.svg";
    case "elasticsearch":
      return "/logos/elasticsearch.svg";
    default:
      return null;
  }
};

// Format uptime in seconds to human-readable format
const formatUptime = (seconds: number): string => {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  } else if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  } else {
    return `${secs}s`;
  }
};

export const FlowsPage: React.FC = () => {
  const { data: flows, isLoading } = useFlows();
  const { data: connectors } = useConnectors();
  const { data: destinations } = useDestinations();
  const { deleteMutation, startMutation, stopMutation, restartMutation } =
    useFlowMutations();
  const [isFormOpen, setIsFormOpen] = useState(false);

  const connectorMap = React.useMemo(() => {
    const map: Record<string, string> = {};
    connectors?.forEach(
      (c) => (map[c.name.toString()] = c.connector_type.toString())
    );
    return map;
  }, [connectors]);

  const destinationMap = React.useMemo(() => {
    const map: Record<string, string> = {};
    destinations?.forEach(
      (d) => (map[d.name.toString()] = d.destination_type.toString())
    );
    return map;
  }, [destinations]);

  const handleDelete = (name: string) => {
    if (window.confirm(`Are you sure you want to delete flow "${name}"?`)) {
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
            Sync Flows
          </h1>
          <p className="text-xs font-bold text-slate-500 uppercase tracking-widest mt-1">
            Data pipeline management
          </p>
        </div>
        <button
          className="flex items-center gap-2 px-4 py-2 bg-indigo-600 rounded-lg text-white text-xs font-bold hover:bg-indigo-700 transition-all active:scale-95 shadow-sm shadow-indigo-100"
          onClick={() => setIsFormOpen(true)}
        >
          <Plus size={16} />
          Create Flow
        </button>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
        {flows?.map((flow) => {
          const connectorType = connectorMap[flow.connector_name.toString()];
          const connectorLogo = getConnectorLogo(connectorType);

          return (
            <div
              key={flow.name.toString()}
              className="bg-white rounded-xl border border-slate-200 overflow-hidden flex flex-col group hover:border-indigo-300 transition-colors"
            >
              <div className="px-6 py-4 border-b border-slate-100 flex justify-between items-center bg-slate-50/30">
                <div className="flex items-center gap-3">
                  <div
                    className={`w-8 h-8 rounded flex items-center justify-center ${
                      flow.active
                        ? "bg-emerald-50 text-emerald-600"
                        : "bg-slate-100 text-slate-400"
                    }`}
                  >
                    <Activity size={18} />
                  </div>
                  <div>
                    <h3 className="text-sm font-bold text-slate-900">
                      {flow.name}
                    </h3>
                    <div className="flex items-center gap-1.5 mt-0.5">
                      <div
                        className={`w-1.5 h-1.5 rounded-full ${
                          flow.active ? "bg-emerald-500" : "bg-slate-300"
                        }`}
                      />
                      <span className="text-[10px] font-bold uppercase tracking-widest text-slate-400">
                        {flow.active ? "Active" : "Offline"}
                      </span>
                    </div>
                  </div>
                </div>

                <div className="flex gap-2">
                  {flow.active ? (
                    <>
                      <button
                        className="p-2 text-blue-600 hover:bg-blue-50 rounded transition-colors"
                        title="Restart Flow"
                        onClick={() =>
                          restartMutation.mutate(flow.name.toString())
                        }
                        disabled={restartMutation.isPending}
                      >
                        <RotateCw
                          size={16}
                          className={
                            restartMutation.isPending ? "animate-spin" : ""
                          }
                        />
                      </button>
                      <button
                        className="p-2 text-amber-600 hover:bg-amber-50 rounded transition-colors"
                        title="Stop Flow"
                        onClick={() =>
                          stopMutation.mutate(flow.name.toString())
                        }
                      >
                        <Square size={16} fill="currentColor" />
                      </button>
                    </>
                  ) : (
                    <button
                      className="p-2 text-emerald-600 hover:bg-emerald-50 rounded transition-colors"
                      title="Start Flow"
                      onClick={() => startMutation.mutate(flow.name.toString())}
                    >
                      <Play size={16} fill="currentColor" />
                    </button>
                  )}
                  <button
                    className="p-2 text-rose-600 hover:bg-rose-50 rounded transition-colors"
                    title="Delete"
                    onClick={() => handleDelete(flow.name.toString())}
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              </div>

              <div className="p-6">
                <div className="flex items-center gap-4 bg-slate-50 rounded-lg p-4 border border-slate-100">
                  <div className="flex-1 flex flex-col items-center">
                    <div className="text-[9px] font-bold text-slate-400 uppercase tracking-[0.2em] mb-2">
                      Source
                    </div>
                    <div className="flex items-center gap-2">
                      <div className="w-6 h-6 rounded bg-white border border-slate-200 flex items-center justify-center p-1">
                        {connectorLogo ? (
                          <img
                            src={connectorLogo}
                            className="w-full h-full object-contain"
                            alt=""
                          />
                        ) : (
                          <Cable size={12} className="text-slate-400" />
                        )}
                      </div>
                      <div className="text-xs font-bold text-slate-700">
                        {flow.connector_name}
                      </div>
                    </div>
                  </div>
                  <ArrowRight size={14} className="text-slate-300" />
                  <div className="flex-1 flex flex-col items-center">
                    <div className="text-[9px] font-bold text-slate-400 uppercase tracking-[0.2em] mb-2">
                      Targets
                    </div>
                    <div className="flex flex-wrap justify-center gap-1.5">
                      {flow.destination_names.map((dest) => {
                        const destType = destinationMap[dest];
                        const destLogo = getDestinationLogo(destType);
                        return (
                          <div
                            key={dest}
                            className="flex items-center gap-1.5 px-2 py-1 bg-white border border-slate-200 rounded"
                          >
                            <div className="w-4 h-4 flex items-center justify-center">
                              {destLogo ? (
                                <img
                                  src={destLogo}
                                  className="w-full h-full object-contain"
                                  alt=""
                                />
                              ) : (
                                <Database
                                  size={10}
                                  className="text-slate-400"
                                />
                              )}
                            </div>
                            <span className="text-[9px] font-bold text-slate-600">
                              {dest}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                </div>
              </div>

              <div className="px-6 py-4 border-t border-slate-100">
                <div className="flex items-center justify-between text-[10px] font-bold mb-3">
                  <div className="flex items-center gap-6">
                    <div className="flex flex-col">
                      <span className="text-slate-400 uppercase tracking-widest">
                        Batch Size
                      </span>
                      <span className="text-slate-800 uppercase">
                        {flow.batch_size} events
                      </span>
                    </div>
                    <div className="flex flex-col">
                      <span className="text-slate-400 uppercase tracking-widest">
                        Auto Start
                      </span>
                      <span className="text-slate-800 uppercase">
                        {flow.auto_start ? "Enabled" : "Disabled"}
                      </span>
                    </div>
                  </div>
                  <button className="flex items-center gap-1.5 text-indigo-600 hover:text-indigo-700">
                    <Info size={14} />
                    Logs
                  </button>
                </div>

                {/* Metrics Display */}
                {flow.active &&
                  (flow.uptime_seconds !== undefined ||
                    flow.records_processed !== undefined ||
                    flow.messages_received !== undefined) && (
                    <div className="flex items-center gap-4 pt-3 border-t border-slate-100">
                      {flow.uptime_seconds !== undefined && (
                        <div className="flex items-center gap-2">
                          <div className="p-1.5 bg-blue-50 rounded">
                            <Clock size={12} className="text-blue-600" />
                          </div>
                          <div className="flex flex-col">
                            <span className="text-[9px] text-slate-400 uppercase tracking-widest">
                              Uptime
                            </span>
                            <span className="text-xs font-bold text-slate-800">
                              {formatUptime(flow.uptime_seconds)}
                            </span>
                          </div>
                        </div>
                      )}
                      {flow.messages_received !== undefined && (
                        <div className="flex items-center gap-2">
                          <div className="p-1.5 bg-purple-50 rounded">
                            <Activity size={12} className="text-purple-600" />
                          </div>
                          <div className="flex flex-col">
                            <span className="text-[9px] text-slate-400 uppercase tracking-widest">
                              Messages
                            </span>
                            <span className="text-xs font-bold text-slate-800">
                              {flow.messages_received.toLocaleString()}
                            </span>
                          </div>
                        </div>
                      )}
                      {flow.records_processed !== undefined && (
                        <div className="flex items-center gap-2">
                          <div className="p-1.5 bg-emerald-50 rounded">
                            <TrendingUp
                              size={12}
                              className="text-emerald-600"
                            />
                          </div>
                          <div className="flex flex-col">
                            <span className="text-[9px] text-slate-400 uppercase tracking-widest">
                              Records
                            </span>
                            <span className="text-xs font-bold text-slate-800">
                              {flow.records_processed.toLocaleString()}
                            </span>
                          </div>
                        </div>
                      )}
                    </div>
                  )}
              </div>
            </div>
          );
        })}
      </div>

      {isFormOpen && <FlowForm onClose={() => setIsFormOpen(false)} />}
    </div>
  );
};

const FlowForm: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { data: connectors } = useConnectors();
  const { data: destinations } = useDestinations();
  const { createMutation } = useFlowMutations();

  const [formData, setFormData] = useState({
    name: "",
    connector_name: "",
    destination_names: [] as string[],
    batch_size: 100,
    auto_start: true,
    description: "",
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    createMutation.mutate(formData as any, { onSuccess: onClose });
  };

  const toggleDestination = (destName: string) => {
    setFormData((prev) => ({
      ...prev,
      destination_names: prev.destination_names.includes(destName)
        ? prev.destination_names.filter((d) => d !== destName)
        : [...prev.destination_names, destName],
    }));
  };

  return (
    <div className="fixed inset-0 z-100 flex items-center justify-center p-6 sm:p-12 animate-in fade-in duration-300">
      <div
        className="absolute inset-0 bg-slate-900/40 backdrop-blur-sm"
        onClick={onClose}
      />
      <div className="relative bg-white w-full max-w-2xl rounded-xl shadow-2xl p-8 animate-in zoom-in-95 duration-300 border border-slate-200">
        <h2 className="text-xl font-bold text-slate-900 mb-6">
          Create New Sync Flow
        </h2>

        <form onSubmit={handleSubmit} className="space-y-6">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="space-y-2">
              <label className="text-[10px] font-bold text-slate-400 uppercase tracking-wider ml-1">
                Flow Name
              </label>
              <input
                type="text"
                required
                className="w-full px-4 py-3 bg-slate-50 border border-slate-200 rounded-lg text-sm font-bold text-slate-900 focus:outline-none focus:border-indigo-500 transition-all"
                value={formData.name}
                onChange={(e) =>
                  setFormData({ ...formData, name: e.target.value })
                }
                placeholder="e.target-events-sync"
              />
            </div>

            <div className="space-y-2">
              <label className="text-[10px] font-bold text-slate-400 uppercase tracking-wider ml-1">
                Source Connector
              </label>
              <select
                required
                className="w-full px-4 py-3 bg-slate-50 border border-slate-200 rounded-lg text-sm font-bold text-slate-900 focus:outline-none focus:border-indigo-500 transition-all"
                value={formData.connector_name}
                onChange={(e) =>
                  setFormData({ ...formData, connector_name: e.target.value })
                }
              >
                <option value="">Select a source</option>
                {connectors?.map((c) => (
                  <option key={c.name.toString()} value={c.name.toString()}>
                    {c.name} ({c.connector_type})
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="space-y-2">
            <label className="text-[10px] font-bold text-slate-400 uppercase tracking-wider ml-1">
              Target Destinations
            </label>
            <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
              {destinations?.map((d) => {
                const isSelected = formData.destination_names.includes(
                  d.name.toString()
                );
                return (
                  <button
                    key={d.name.toString()}
                    type="button"
                    onClick={() => toggleDestination(d.name.toString())}
                    className={`px-3 py-2 rounded-lg border text-xs font-bold transition-all text-left flex items-center justify-between ${
                      isSelected
                        ? "bg-indigo-50 border-indigo-200 text-indigo-700"
                        : "bg-white border-slate-200 text-slate-600 hover:border-slate-300"
                    }`}
                  >
                    <span>{d.name}</span>
                    {isSelected && <CheckCircle2 size={14} />}
                  </button>
                );
              })}
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="space-y-2">
              <label className="text-[10px] font-bold text-slate-400 uppercase tracking-wider ml-1">
                Batch Size
              </label>
              <input
                type="number"
                className="w-full px-4 py-3 bg-slate-50 border border-slate-200 rounded-lg text-sm font-bold text-slate-900 focus:outline-none focus:border-indigo-500 transition-all"
                value={formData.batch_size}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    batch_size: parseInt(e.target.value),
                  })
                }
              />
            </div>
            <div className="flex items-center gap-3 pt-6">
              <input
                type="checkbox"
                id="auto_start"
                className="w-4 h-4 text-indigo-600 rounded border-slate-300 focus:ring-indigo-500"
                checked={formData.auto_start}
                onChange={(e) =>
                  setFormData({ ...formData, auto_start: e.target.checked })
                }
              />
              <label
                htmlFor="auto_start"
                className="text-xs font-bold text-slate-700"
              >
                Auto-start on creation
              </label>
            </div>
          </div>

          <div className="flex gap-3 pt-6 border-t border-slate-100">
            <button
              type="button"
              className="px-6 py-2.5 bg-white border border-slate-200 rounded-lg text-sm font-bold text-slate-600 hover:bg-slate-50 transition-all"
              onClick={onClose}
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={
                !formData.name ||
                !formData.connector_name ||
                formData.destination_names.length === 0
              }
              className="flex-1 px-6 py-2.5 bg-indigo-600 rounded-lg text-white font-bold text-sm hover:bg-indigo-700 transition-all shadow-sm shadow-indigo-100 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Create Sync Flow
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
