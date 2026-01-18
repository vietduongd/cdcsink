-- Initialize database schema for CDC destination

-- CDC Events table
CREATE TABLE IF NOT EXISTS cdc_events (
    id UUID PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL,
    source VARCHAR(255) NOT NULL,
    table_name VARCHAR(255) NOT NULL,
    operation VARCHAR(50) NOT NULL,
    data JSONB NOT NULL,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cdc_events_timestamp ON cdc_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_cdc_events_source ON cdc_events(source);
CREATE INDEX IF NOT EXISTS idx_cdc_events_table ON cdc_events(table_name);
CREATE INDEX IF NOT EXISTS idx_cdc_events_operation ON cdc_events(operation);

-- ============================================================
-- Configuration Management Tables
-- ============================================================

-- Connector configurations
CREATE TABLE IF NOT EXISTS connectors (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    connector_type VARCHAR(100) NOT NULL,
    config JSONB NOT NULL,
    description TEXT,
    tags TEXT[],
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_connectors_name ON connectors(name);
CREATE INDEX IF NOT EXISTS idx_connectors_type ON connectors(connector_type);
CREATE INDEX IF NOT EXISTS idx_connectors_tags ON connectors USING GIN(tags);

-- Destination configurations
CREATE TABLE IF NOT EXISTS destinations (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    destination_type VARCHAR(100) NOT NULL,
    config JSONB NOT NULL,
    description TEXT,
    tags TEXT[],
    schemas_includes TEXT[],
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_destinations_name ON destinations(name);
CREATE INDEX IF NOT EXISTS idx_destinations_type ON destinations(destination_type);
CREATE INDEX IF NOT EXISTS idx_destinations_tags ON destinations USING GIN(tags);

-- Flow configurations
CREATE TABLE IF NOT EXISTS flows (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    connector_name VARCHAR(255) NOT NULL REFERENCES connectors(name) ON DELETE RESTRICT,
    destination_names TEXT[] NOT NULL,
    batch_size INTEGER NOT NULL DEFAULT 100,
    auto_start BOOLEAN NOT NULL DEFAULT true,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_flows_name ON flows(name);
CREATE INDEX IF NOT EXISTS idx_flows_connector ON flows(connector_name);
CREATE INDEX IF NOT EXISTS idx_flows_auto_start ON flows(auto_start);

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_connectors_updated_at BEFORE UPDATE ON connectors
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_destinations_updated_at BEFORE UPDATE ON destinations
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_flows_updated_at BEFORE UPDATE ON flows
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
