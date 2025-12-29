import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Route, Routes } from "react-router-dom";
import "./App.css";
import { Layout } from "./components/Layout";
import { ConnectorsPage } from "./pages/ConnectorsPage";
import { DestinationsPage } from "./pages/DestinationsPage";
import { FlowsPage } from "./pages/FlowsPage";
import { Dashboard } from "./pages/Dashboard";

const queryClient = new QueryClient();

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Layout>
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/connectors" element={<ConnectorsPage />} />
            <Route path="/destinations" element={<DestinationsPage />} />
            <Route path="/flows" element={<FlowsPage />} />
          </Routes>
        </Layout>
      </BrowserRouter>
    </QueryClientProvider>
  );
}

export default App;
