# fatrs-adapters TODO

## ~~Header Rotation for NOR Flash~~ (IMPLEMENTED)

**Status: Implemented** - See `HeaderRotatingDevice` in `src/adapters/header_rotating_device.rs`

### Implementation Summary

Created `HeaderRotatingDevice<D>` wrapper that provides header rotation for any `BlockDevice<4096>`:

```rust
use fatrs_adapters::{HeaderRotatingDevice, HeaderRotationConfig};

// Wrap any block device with header rotation
let config = HeaderRotationConfig::new(4); // 4-page rotation
let mut device = HeaderRotatingDevice::new(inner_device, config);

// Consumer scans header slots to find current valid header
for slot in 0..device.rotation_pages() {
    device.read_header_slot(slot, &mut buf).await?;
    // Parse sequence number, find highest valid
}
device.set_current_slot(best_slot);

// Use normally - header writes auto-rotate
```

### Design Decisions

1. **Separate wrapper type** - Composable, follows existing patterns
2. **Consumer reads sequence numbers** - Adapter doesn't know header format
3. **`set_current_slot()` for init** - Consumer controls initialization

### Features

- `HeaderRotationConfig` - Configure 1-8 rotation pages
- `HeaderRotatingDevice<D>` - Generic wrapper for any BlockDevice<4096>
- `read_header_slot()` - Read specific slot for init scanning
- Logical-to-physical page mapping
- Automatic slot advancement on header writes
- Reports logical size (physical - reserved pages)
