# CDC Connectors

This directory contains all connector plugins for the CDC system.

## Available Connectors

### ✅ NATS Connector

**Status:** Production Ready  
**Path:** `nats/`  
**Type:** `"nats"`

Connects to NATS message broker for consuming CDC events.

[View Documentation](./nats/README.md)

---

### 🚧 Kafka Connector

**Status:** Stub/Template  
**Path:** `kafka/`  
**Type:** `"kafka"`

Template for Kafka connector. Needs implementation.

[View Documentation](./kafka/README.md)

---

## Adding a New Connector

1. Create new directory: `crates/cdc-connectors/your-connector/`

2. Create `Cargo.toml`:

```toml
[package]
name = "cdc-your-connector"
version.workspace = true
edition.workspace = true

[dependencies]
cdc-core = { path = "../../cdc-core" }
# Your connector-specific dependencies
```

3. Implement `Connector` trait in `src/your_connector.rs`

4. Create `Factory` in `src/factory.rs`:

```rust
pub struct YourConnectorFactory;

impl ConnectorFactory for YourConnectorFactory {
    fn name(&self) -> &str { "your-connector" }
    fn create(&self, config: Value) -> Result<Box<dyn Connector>> {
        // Create and return your connector
    }
}
```

5. Add to workspace in root `Cargo.toml`:

```toml
members = [
    "crates/cdc-connectors/your-connector",
]
```

6. Register in CLI (`crates/cdc-cli/src/main.rs`):

```rust
use cdc_your_connector::YourConnectorFactory;

registry.register_connector(Arc::new(YourConnectorFactory));
```

7. Use in config:

```yaml
flows:
  - name: "my-flow"
    connector:
      type: "your-connector"
      config:
        # Your connector-specific config
```

That's it! No core code changes needed.
