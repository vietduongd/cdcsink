# MySQL Destination Plugin

MySQL destination for the CDC system.

## Status

⚠️ **Stub Implementation** - This is a template/example destination.

To implement:

1. Uncomment `sqlx` MySQL feature in `Cargo.toml`
2. Implement actual MySQL connection and writes in `mysql_destination.rs`
3. Add conflict resolution strategies
4. Add batch optimization

## Configuration

```yaml
flows:
  - name: "events-to-mysql"
    connector:
      type: "nats"
      config:
        subject: "events"
    destinations:
      - type: "mysql"
        config:
          url: "mysql://user:pass@localhost/mydb"
          max_connections: 10
          database: "mydb"
    batch_size: 100
```

## Usage

Register in CLI:

```rust
use cdc_mysql_destination::MysqlDestinationFactory;

registry.register_destination(Arc::new(MysqlDestinationFactory));
```
