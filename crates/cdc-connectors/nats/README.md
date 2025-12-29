# NATS Connector Plugin

NATS connector for the CDC system.

## Status

✅ **Production Ready**

## Features

- Subscribe to NATS subjects/streams
- Automatic reconnection
- Consumer group support
- NATS JetStream support
- Message deserialization

## Configuration

```yaml
flows:
  - name: "nats-events"
    connector:
      type: "nats"
      config:
        servers: ["nats://localhost:4222"]
        subject: "cdc.events"
        consumer_group: "cdc-group" # Optional
        use_jetstream: false
    destinations:
      - type: "postgres"
        config:
          url: "postgresql://localhost/db"
    batch_size: 100
```

## Configuration Options

- `servers`: List of NATS server URLs
- `subject`: Subject pattern to subscribe to
- `consumer_group`: Optional consumer group for load balancing
- `use_jetstream`: Enable JetStream for guaranteed delivery
