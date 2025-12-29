import React, { useState } from "react";
import {
  useFlows,
  useFlowMutations,
  useConnectors,
  useDestinations,
} from "../hooks/useQueries";
import type { FlowConfigEntry } from "../api";
import {
  Plus,
  Trash2,
  Play,
  Square,
  Activity,
  ArrowRight,
  Info,
  CheckCircle2,
} from "lucide-react";

export const FlowsPage: React.FC = () => {
  const { data: flows, isLoading } = useFlows();
  const { deleteMutation, startMutation, stopMutation } = useFlowMutations();
  const [isFormOpen, setIsFormOpen] = useState(false);

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
        {flows?.map((flow) => (
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
                  <button
                    className="p-2 text-amber-600 hover:bg-amber-50 rounded transition-colors"
                    title="Stop Flow"
                    onClick={() => stopMutation.mutate(flow.name.toString())}
                  >
                    <Square size={16} fill="currentColor" />
                  </button>
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
                <div className="flex-1 text-center">
                  <div className="text-[9px] font-bold text-slate-400 uppercase tracking-[0.2em] mb-1">
                    Source
                  </div>
                  <div className="text-xs font-bold text-slate-700">
                    {flow.connector_name}
                  </div>
                </div>
                <ArrowRight size={14} className="text-slate-300" />
                <div className="flex-1 text-center">
                  <div className="text-[9px] font-bold text-slate-400 uppercase tracking-[0.2em] mb-1">
                    Targets
                  </div>
                  <div className="flex flex-wrap justify-center gap-1">
                    {flow.destination_names.map((dest) => (
                      <span
                        key={dest}
                        className="px-1.5 py-0.5 bg-white border border-slate-200 rounded text-[9px] font-bold text-slate-500"
                      >
                        {dest}
                      </span>
                    ))}
                  </div>
                </div>
              </div>
            </div>

            <div className="px-6 py-4 border-t border-slate-100 flex items-center justify-between text-[10px] font-bold">
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
          </div>
        ))}
      </div>

      {isFormOpen && <FlowForm onClose={() => setIsFormOpen(false)} />}
    </div>
  );
};

const FlowForm: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { createMutation } = useFlowMutations();
  const { data: connectors } = useConnectors();
  const { data: destinations } = useDestinations();

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
    if (!formData.connector_name || formData.destination_names.length === 0) {
      alert("Please select a connector and at least one destination.");
      return;
    }
    createMutation.mutate(formData as any, { onSuccess: onClose });
  };

  const toggleDestination = (name: string) => {
    setFormData((prev) => ({
      ...prev,
      destination_names: prev.destination_names.includes(name)
        ? prev.destination_names.filter((n) => n !== name)
        : [...prev.destination_names, name],
    }));
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center p-6 sm:p-12 animate-in fade-in duration-300">
      <div
        className="absolute inset-0 bg-slate-900/40 backdrop-blur-md"
        onClick={onClose}
      />
      <div className="relative bg-white w-full max-w-2xl rounded-[2.5rem] shadow-2xl p-8 sm:p-10 animate-in zoom-in-95 duration-300 overflow-y-auto max-h-[90vh]">
        <h2 className="text-3xl font-black text-slate-900 mb-8 tracking-tight">
          Configure Sync Flow
        </h2>

        <form onSubmit={handleSubmit} className="space-y-8">
          <div className="grid grid-cols-2 gap-6">
            <div className="space-y-2">
              <label className="text-xs font-black text-slate-400 uppercase tracking-widest ml-1">
                Flow Name
              </label>
              <input
                type="text"
                required
                placeholder="e.g. daily-events-sync"
                className="w-full px-5 py-4 bg-slate-50 border border-slate-200 rounded-2xl text-slate-900 font-bold focus:outline-none focus:border-blue-500 focus:ring-4 focus:ring-blue-100 transition-all"
                value={formData.name}
                onChange={(e) =>
                  setFormData({ ...formData, name: e.target.value })
                }
              />
            </div>
            <div className="space-y-2">
              <label className="text-xs font-black text-slate-400 uppercase tracking-widest ml-1">
                Batch Size
              </label>
              <input
                type="number"
                required
                className="w-full px-5 py-4 bg-slate-50 border border-slate-200 rounded-2xl text-slate-900 font-bold focus:outline-none focus:border-blue-500 focus:ring-4 focus:ring-blue-100 transition-all"
                value={formData.batch_size}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    batch_size: parseInt(e.target.value),
                  })
                }
              />
            </div>
          </div>

          <div className="space-y-4">
            <label className="text-xs font-black text-slate-400 uppercase tracking-widest ml-1 block">
              1. Select Data Source
            </label>
            <div className="grid grid-cols-2 gap-3">
              {connectors?.map((c) => (
                <div
                  key={c.name.toString()}
                  onClick={() =>
                    setFormData({
                      ...formData,
                      connector_name: c.name.toString(),
                    })
                  }
                  className={`p-4 rounded-2xl border-2 cursor-pointer transition-all flex items-center justify-between ${
                    formData.connector_name === c.name.toString()
                      ? "border-blue-600 bg-blue-50"
                      : "border-slate-100 bg-slate-50 hover:border-slate-200"
                  }`}
                >
                  <div>
                    <div className="font-bold text-slate-800 text-sm">
                      {c.name}
                    </div>
                    <div className="text-[10px] text-slate-400 font-bold uppercase">
                      {c.connector_type}
                    </div>
                  </div>
                  {formData.connector_name === c.name.toString() && (
                    <CheckCircle2 size={18} className="text-blue-600" />
                  )}
                </div>
              ))}
            </div>
          </div>

          <div className="space-y-4">
            <label className="text-xs font-black text-slate-400 uppercase tracking-widest ml-1 block">
              2. Select Destinations
            </label>
            <div className="grid grid-cols-2 gap-3">
              {destinations?.map((d) => (
                <div
                  key={d.name.toString()}
                  onClick={() => toggleDestination(d.name.toString())}
                  className={`p-4 rounded-2xl border-2 cursor-pointer transition-all flex items-center justify-between ${
                    formData.destination_names.includes(d.name.toString())
                      ? "border-indigo-600 bg-indigo-50"
                      : "border-slate-100 bg-slate-50 hover:border-slate-200"
                  }`}
                >
                  <div>
                    <div className="font-bold text-slate-800 text-sm">
                      {d.name}
                    </div>
                    <div className="text-[10px] text-slate-400 font-bold uppercase">
                      {d.destination_type}
                    </div>
                  </div>
                  {formData.destination_names.includes(d.name.toString()) && (
                    <CheckCircle2 size={18} className="text-indigo-600" />
                  )}
                </div>
              ))}
            </div>
          </div>

          <div className="flex items-center gap-3 p-4 bg-slate-50 rounded-2xl border border-slate-100">
            <input
              type="checkbox"
              id="autoStart"
              className="w-5 h-5 rounded-lg border-slate-300 text-blue-600 focus:ring-blue-500"
              checked={formData.auto_start}
              onChange={(e) =>
                setFormData({ ...formData, auto_start: e.target.checked })
              }
            />
            <label
              htmlFor="autoStart"
              className="text-sm font-bold text-slate-700 cursor-pointer"
            >
              Automatically start this flow on system launch
            </label>
          </div>

          <div className="flex gap-4 pt-4">
            <button
              type="button"
              className="flex-1 px-8 py-4 bg-white border border-slate-200 rounded-2xl text-slate-700 font-bold hover:bg-slate-50 transition-all"
              onClick={onClose}
            >
              Cancel
            </button>
            <button
              type="submit"
              className="flex-1 px-8 py-4 bg-blue-600 rounded-2xl text-white font-bold hover:bg-blue-700 transition-all shadow-xl shadow-blue-200 active:scale-95"
            >
              Configure Sync Flow
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
