import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Route, Routes } from "react-router-dom";
import { Toaster } from "react-hot-toast";
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
      <Toaster
        position="top-right"
        toastOptions={{
          duration: 4000,
          style: {
            background: "#363636",
            color: "#fff",
          },
          success: {
            duration: 3000,
            iconTheme: {
              primary: "#10b981",
              secondary: "#fff",
            },
          },
          error: {
            duration: 5000,
            iconTheme: {
              primary: "#ef4444",
              secondary: "#fff",
            },
          },
        }}
      />
    </QueryClientProvider>
  );
}

export default App;
