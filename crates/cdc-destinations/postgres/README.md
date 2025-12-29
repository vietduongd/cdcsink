# PostgreSQL Destination Plugin

PostgreSQL destination for the CDC system.

## Status

✅ **Production Ready**

## Features

- Connection pooling
- Batch write optimization
- Three conflict resolution strategies:
  - **Upsert**: INSERT...ON CONFLICT DO UPDATE
  - **Replace**: Standard INSERT
  - **Ignore**: INSERT...ON CONFLICT DO NOTHING
- Transaction support
- Dynamic SQL generation

## Configuration

```yaml
flows:
  - name: "events-to-postgres"
    connector:
      type: "nats"
      config:
        subject: "events"
    destinations:
      - type: "postgres"
        config:
          url: "postgresql://user:pass@localhost/mydb"
          max_connections: 10
          schema: "public"
          conflict_resolution: "upsert"
    batch_size: 100
```

## Configuration Options

- `url`: PostgreSQL connection URL
- `max_connections`: Maximum connections in the pool
- `schema`: Target schema name
- `conflict_resolution`: `"upsert"`, `"replace"`, or `"ignore"`
