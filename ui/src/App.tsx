import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StatsPanel } from "./components/StatsPanel";
import { MetricsChart } from "./components/MetricsChart";
import { useHealth } from "./hooks/useQueries";
import "./App.css";

const queryClient = new QueryClient();

const Dashboard = () => {
  const { data: health } = useHealth();

  return (
    <div className="dashboard">
      <header className="header">
        <div className="header-content">
          <h1 className="logo">
            <span className="logo-icon">⚡</span>
            CDC Data Sync
          </h1>
          <div className="status-indicator">
            <div
              className={`status-dot ${
                health?.status === "healthy" ? "online" : "offline"
              }`}
            />
            <span className="status-text">
              {health?.status === "healthy"
                ? "System Online"
                : "System Offline"}
            </span>
          </div>
        </div>
      </header>

      <main className="main-content">
        <section className="section">
          <h2 className="section-title">System Metrics</h2>
          <StatsPanel />
        </section>

        <section className="section">
          <h2 className="section-title">Real-time Throughput</h2>
          <MetricsChart />
        </section>

        <section className="section">
          <h2 className="section-title">System Information</h2>
          <div className="info-panel">
            <div className="info-item">
              <span className="info-label">NATS Subject:</span>
              <span className="info-value">cdc.events</span>
            </div>
            <div className="info-item">
              <span className="info-label">PostgreSQL Schema:</span>
              <span className="info-value">public</span>
            </div>
            <div className="info-item">
              <span className="info-label">Batch Size:</span>
              <span className="info-value">100 records</span>
            </div>
          </div>
        </section>
      </main>
    </div>
  );
};

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Dashboard />
    </QueryClientProvider>
  );
}

export default App;
