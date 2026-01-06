import {
  AlertCircle,
  Check,
  Database,
  Edit2,
  Loader2,
  Play,
  Plus,
  Trash2,
  X,
} from "lucide-react";
import React, { useState } from "react";
import type { DestinationConfigEntry } from "../api";
import { api } from "../api";
import { useDestinationMutations, useDestinations } from "../hooks/useQueries";

export const DestinationsPage: React.FC = () => {
  const { data: destinations, isLoading } = useDestinations();
  const { deleteMutation, testMutation } = useDestinationMutations();
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [editingDestination, setEditingDestination] =
    useState<DestinationConfigEntry | null>(null);

  const handleDelete = (name: string) => {
    if (
      window.confirm(`Are you sure you want to delete destination "${name}"?`)
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
            Data Destinations
          </h1>
          <p className="text-xs font-bold text-slate-500 uppercase tracking-widest mt-1">
            Sink configuration management
          </p>
        </div>
        <button
          className="flex items-center gap-2 px-4 py-2 bg-indigo-600 rounded-lg text-white text-xs font-bold hover:bg-indigo-700 transition-all active:scale-95 shadow-sm shadow-indigo-100"
          onClick={() => {
            setEditingDestination(null);
            setIsFormOpen(true);
          }}
        >
          <Plus size={16} />
          Add Destination
        </button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
        {destinations?.map((destination) => (
          <div
            key={destination.name.toString()}
            className="bg-white rounded-xl border border-slate-200 overflow-hidden flex flex-col group hover:border-indigo-300 transition-colors"
          >
            <div className="px-6 py-4 border-b border-slate-100 flex justify-between items-center bg-slate-50/30">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded bg-white border border-slate-100 flex items-center justify-center overflow-hidden p-1 shadow-sm">
                  {destination.destination_type === "postgres" && (
                    <img
                      src="/logos/postgresql.svg"
                      className="w-full h-full object-contain"
                      alt="Postgres"
                    />
                  )}
                  {destination.destination_type === "mysql" && (
                    <img
                      src="/logos/mysql.svg"
                      className="w-full h-full object-contain"
                      alt="MySQL"
                    />
                  )}
                  {destination.destination_type === "clickhouse" && (
                    <img
                      src="/logos/clickhouse.svg"
                      className="w-full h-full object-contain"
                      alt="ClickHouse"
                    />
                  )}
                  {destination.destination_type === "elasticsearch" && (
                    <img
                      src="/logos/elasticsearch.svg"
                      className="w-full h-full object-contain"
                      alt="Elasticsearch"
                    />
                  )}
                  {![
                    "postgres",
                    "mysql",
                    "clickhouse",
                    "elasticsearch",
                  ].includes(destination.destination_type.toString()) && (
                    <Database size={18} className="text-emerald-600" />
                  )}
                </div>
                <div>
                  <h3 className="text-sm font-bold text-slate-900">
                    {destination.name}
                  </h3>
                  <span className="text-[10px] font-bold uppercase tracking-widest text-slate-400">
                    {destination.destination_type}
                  </span>
                </div>
              </div>
              <div className="flex gap-1">
                <button
                  className="p-2 text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 rounded transition-colors"
                  onClick={() => {
                    setEditingDestination(destination);
                    setIsFormOpen(true);
                  }}
                >
                  <Edit2 size={16} />
                </button>
                <button
                  className="p-2 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded transition-colors"
                  onClick={() => handleDelete(destination.name.toString())}
                >
                  <Trash2 size={16} />
                </button>
              </div>
            </div>

            <div className="p-6 flex-1">
              <p className="text-xs text-slate-500 font-medium line-clamp-2 min-h-[2rem]">
                {destination.description ||
                  "No description provided for this destination."}
              </p>

              <div className="mt-4 flex flex-wrap gap-1">
                {destination.tags?.map((tag) => (
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
                onClick={() => testMutation.mutate(destination.name.toString())}
              >
                <Play size={12} fill="currentColor" />
                Test Connection
              </button>
              <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest text-emerald-600/60 font-mono">
                SINK OK
              </div>
            </div>
          </div>
        ))}
      </div>

      {isFormOpen && (
        <DestinationForm
          destination={editingDestination}
          onClose={() => setIsFormOpen(false)}
        />
      )}
    </div>
  );
};

// Helper function to get default config for each destination type
const getDefaultDestinationConfig = (
  destinationType: string
): Record<string, any> => {
  switch (destinationType) {
    case "postgres":
      return {
        host: "localhost",
        port: 5432,
        username: "postgres",
      };
    case "kafka":
      return {
        topic: "cdc-output",
      };
    default:
      return {};
  }
};

const DestinationForm: React.FC<{
  destination: DestinationConfigEntry | null;
  onClose: () => void;
}> = ({ destination, onClose }) => {
  const { createMutation, updateMutation } = useDestinationMutations();
  const [formData, setFormData] = useState({
    name: destination?.name || "",
    destination_type: destination?.destination_type || "postgres",
    description: destination?.description || "",
  });

  // Initialize config with defaults merged with existing config
  const [config, setConfig] = useState<Record<string, any>>(() => {
    const defaults = getDefaultDestinationConfig(
      (destination?.destination_type as string) || "postgres"
    );
    const existingConfig = (destination?.config as Record<string, any>) || {};
    return { ...defaults, ...existingConfig };
  });
  const [tags, setTags] = useState<string[]>(destination?.tags || []);
  const [tagInput, setTagInput] = useState("");
  const [testStatus, setTestStatus] = useState<
    "idle" | "loading" | "success" | "error"
  >("idle");
  const [testMessage, setTestMessage] = useState("");

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

    const testConfig: DestinationConfigEntry = {
      name: formData.name || "test",
      destination_type: formData.destination_type,
      config,
      description: formData.description,
      tags,
    };

    try {
      const result = await api.testDestinationConfig(testConfig);
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

    const payload = {
      ...formData,
      config,
      tags,
    };

    if (destination) {
      updateMutation.mutate(
        { name: destination.name.toString(), destination: payload as any },
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
          <span>
            {destination ? "Edit Destination" : "Add New Destination"}
          </span>
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
                Destination Name
              </label>
              <input
                type="text"
                required
                disabled={!!destination}
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
                value={formData.destination_type.toString()}
                onChange={(e) =>
                  setFormData({ ...formData, destination_type: e.target.value })
                }
              >
                <option value="postgres">PostgreSQL</option>
                <option value="mysql">MySQL</option>
                <option value="clickhouse">ClickHouse</option>
                <option value="elasticsearch">Elasticsearch</option>
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
              {(formData.destination_type === "postgres" ||
                formData.destination_type === "mysql") && (
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
                      value={
                        config.port ||
                        (formData.destination_type === "postgres" ? 5432 : 3306)
                      }
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
                      value={config.username || "root"}
                      onChange={(e) => updateConfig("username", e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Password
                    </label>
                    <input
                      type="password"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.password || ""}
                      onChange={(e) => updateConfig("password", e.target.value)}
                    />
                  </div>
                  <div className="space-y-2 md:col-span-2">
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
                </>
              )}

              {formData.destination_type === "clickhouse" && (
                <>
                  <div className="space-y-2 md:col-span-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      HTTP URL
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.url || "http://localhost:8123"}
                      onChange={(e) => updateConfig("url", e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Username
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.username || "default"}
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
                      value={config.database || "default"}
                      onChange={(e) => updateConfig("database", e.target.value)}
                    />
                  </div>
                </>
              )}

              {formData.destination_type === "elasticsearch" && (
                <>
                  <div className="space-y-2 md:col-span-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Nodes (comma-separated)
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.nodes || "http://localhost:9200"}
                      onChange={(e) => updateConfig("nodes", e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Index Prefix
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.index_prefix || "cdc-"}
                      onChange={(e) =>
                        updateConfig("index_prefix", e.target.value)
                      }
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-[10px] font-bold text-slate-500 uppercase">
                      Cloud ID
                    </label>
                    <input
                      type="text"
                      className="w-full px-3 py-2 bg-slate-50 border border-slate-200 rounded-md text-xs font-bold focus:outline-none focus:border-indigo-400"
                      value={config.cloud_id || ""}
                      onChange={(e) => updateConfig("cloud_id", e.target.value)}
                      placeholder="Optional"
                    />
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
              {destination ? "Save Changes" : "Create Destination"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
