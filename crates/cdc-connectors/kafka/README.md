# Kafka Connector Plugin

Kafka connector for the CDC system.

## Status

⚠️ **Stub Implementation** - This is a template/example connector.

To implement:

1. Uncomment `rdkafka` dependency in `Cargo.toml`
2. Implement actual Kafka consumer in `kafka_connector.rs`
3. Add error handling and reconnection logic

## Configuration

```yaml
flows:
  - name: "kafka-events"
    connector:
      type: "kafka"
      config:
        brokers: ["localhost:9092"]
        topic: "events"
        group_id: "cdc-consumer"
        auto_offset_reset: "earliest"
    destinations:
      - type: "postgres"
        config:
          url: "postgresql://localhost/db"
    batch_size: 100
```

## Usage

Register in CLI:

```rust
use cdc_kafka_connector::KafkaConnectorFactory;

registry.register_connector(Arc::new(KafkaConnectorFactory));
```
