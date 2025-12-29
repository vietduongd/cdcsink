# CDC Destinations

This directory contains all destination plugins for the CDC system.

## Available Destinations

### ✅ PostgreSQL Destination

**Status:** Production Ready  
**Path:** `postgres/`  
**Type:** `"postgres"`

Writes CDC events to PostgreSQL database with conflict resolution.

[View Documentation](./postgres/README.md)

---

### 🚧 MySQL Destination

**Status:** Stub/Template  
**Path:** `mysql/`  
**Type:** `"mysql"`

Template for MySQL destination. Needs implementation.

[View Documentation](./mysql/README.md)

---

## Adding a New Destination

1. Create new directory: `crates/cdc-destinations/your-destination/`

2. Create `Cargo.toml`:

```toml
[package]
name = "cdc-your-destination"
version.workspace = true
edition.workspace = true

[dependencies]
cdc-core = { path = "../../cdc-core" }
# Your destination-specific dependencies
```

3. Implement `Destination` trait in `src/your_destination.rs`

4. Create `Factory` in `src/factory.rs`:

```rust
pub struct YourDestinationFactory;

impl DestinationFactory for YourDestinationFactory {
    fn name(&self) -> &str { "your-destination" }
    fn create(&self, config: Value) -> Result<Box<dyn Destination>> {
        // Create and return your destination
    }
}
```

5. Add to workspace in root `Cargo.toml`:

```toml
members = [
    "crates/cdc-destinations/your-destination",
]
```

6. Register in CLI (`crates/cdc-cli/src/main.rs`):

```rust
use cdc_your_destination::YourDestinationFactory;

registry.register_destination(Arc::new(YourDestinationFactory));
```

7. Use in config:

```yaml
flows:
  - name: "my-flow"
    connector:
      type: "nats"
      config:
        subject: "events"
    destinations:
      - type: "your-destination"
        config:
          # Your destination-specific config
```

That's it! No core code changes needed.
