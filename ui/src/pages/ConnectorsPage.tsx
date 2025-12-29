import React, { useState } from "react";
import { useConnectors, useConnectorMutations } from "../hooks/useQueries";
import type { ConnectorConfigEntry } from "../api";
import { Plus, Edit2, Trash2, Play, Cable, X, Check } from "lucide-react";

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
                <div className="w-8 h-8 rounded bg-indigo-50 text-indigo-600 flex items-center justify-center">
                  <Cable size={18} />
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

const ConnectorForm: React.FC<{
  connector: ConnectorConfigEntry | null;
  onClose: () => void;
}> = ({ connector, onClose }) => {
  const { createMutation, updateMutation } = useConnectorMutations();
  const [formData, setFormData] = useState({
    name: connector?.name || "",
    connector_type: connector?.connector_type || "nats",
    config: connector?.config || {},
    description: connector?.description || "",
    tags: connector?.tags || [],
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (connector) {
      updateMutation.mutate(
        { name: connector.name.toString(), connector: formData as any },
        { onSuccess: onClose }
      );
    } else {
      createMutation.mutate(formData as any, { onSuccess: onClose });
    }
  };

  return (
    <div className="fixed inset-0 z-100 flex items-center justify-center p-6 sm:p-12 animate-in fade-in duration-300">
      <div
        className="absolute inset-0 bg-slate-900/40 backdrop-blur-sm"
        onClick={onClose}
      />
      <div className="relative bg-white w-full max-w-xl rounded-xl shadow-2xl p-8 animate-in zoom-in-95 duration-300 border border-slate-200">
        <h2 className="text-xl font-bold text-slate-900 mb-6">
          {connector ? "Edit Connector" : "Add New Connector"}
        </h2>

        <form onSubmit={handleSubmit} className="space-y-6">
          <div className="space-y-2">
            <label className="text-[10px] font-bold text-slate-400 uppercase tracking-wider ml-1">
              Connector Name
            </label>
            <input
              type="text"
              required
              disabled={!!connector}
              className="w-full px-4 py-3 bg-slate-50 border border-slate-200 rounded-lg text-sm font-bold text-slate-900 focus:outline-none focus:border-indigo-500 transition-all disabled:opacity-50"
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
              className="w-full px-4 py-3 bg-slate-50 border border-slate-200 rounded-lg text-sm font-bold text-slate-900 focus:outline-none focus:border-indigo-500 transition-all"
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

          <div className="space-y-2">
            <label className="text-[10px] font-bold text-slate-400 uppercase tracking-wider ml-1">
              Description
            </label>
            <textarea
              rows={3}
              className="w-full px-4 py-3 bg-slate-50 border border-slate-200 rounded-lg text-sm font-medium text-slate-900 focus:outline-none focus:border-indigo-500 transition-all resize-none"
              value={formData.description}
              onChange={(e) =>
                setFormData({ ...formData, description: e.target.value })
              }
            />
          </div>

          <div className="flex gap-3 pt-4 border-t border-slate-100">
            <button
              type="button"
              className="flex-1 px-4 py-2.5 bg-white border border-slate-200 rounded-lg text-sm font-bold text-slate-600 hover:bg-slate-50 transition-all"
              onClick={onClose}
            >
              Cancel
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
