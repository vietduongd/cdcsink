-- Sample data for testing configuration management

-- Insert sample connectors
INSERT INTO connectors (name, connector_type, config, description, tags) VALUES
('nats-local', 'nats', 
 '{"servers": ["nats://nats:4222"], "subject": "cdc.events", "consumer_group": null, "use_jetstream": false}'::jsonb,
 'Local NATS server for development',
 ARRAY['development', 'local']
) ON CONFLICT (name) DO NOTHING;

INSERT INTO connectors (name, connector_type, config, description, tags) VALUES
('nats-production', 'nats',
 '{"servers": ["nats://prod-nats:4222"], "subject": "events.*", "consumer_group": "cdc-prod-group", "use_jetstream": true}'::jsonb,
 'Production NATS cluster',
 ARRAY['production', 'primary']
) ON CONFLICT (name) DO NOTHING;

-- Insert sample destinations
INSERT INTO destinations (name, destination_type, config, description, tags) VALUES
('postgres-local', 'postgres',
 '{"url": "postgresql://postgres:postgres@postgres:5432/cdc", "max_connections": 10, "schema": "public", "conflict_resolution": "upsert"}'::jsonb,
 'Local PostgreSQL database',
 ARRAY['development', 'local']
) ON CONFLICT (name) DO NOTHING;

INSERT INTO destinations (name, destination_type, config, description, tags) VALUES
('postgres-production', 'postgres',
 '{"url": "postgresql://prod_user:prod_pass@prod-db:5432/cdc_prod", "max_connections": 20, "schema": "public", "conflict_resolution": "upsert"}'::jsonb,
 'Production PostgreSQL primary database',
 ARRAY['production', 'primary']
) ON CONFLICT (name) DO NOTHING;

INSERT INTO destinations (name, destination_type, config, description, tags) VALUES
('postgres-backup', 'postgres',
 '{"url": "postgresql://backup_user:backup_pass@backup-db:5432/cdc_backup", "max_connections": 10, "schema": "public", "conflict_resolution": "upsert"}'::jsonb,
 'Production PostgreSQL backup database',
 ARRAY['production', 'backup']
) ON CONFLICT (name) DO NOTHING;

-- Insert sample flows
INSERT INTO flows (name, connector_name, destination_names, batch_size, auto_start, description) VALUES
('local-dev-flow', 'nats-local', ARRAY['postgres-local'], 100, true,
 'Development flow for local testing')
ON CONFLICT (name) DO NOTHING;
