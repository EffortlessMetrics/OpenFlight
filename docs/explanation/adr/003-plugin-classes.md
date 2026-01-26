# ADR-003: Plugin Classification System

## Status
Accepted

## Context

Flight Hub needs extensibility for custom functionality while maintaining real-time guarantees and security. Different plugin types have different requirements for performance, security, and capabilities. A one-size-fits-all approach would either be too restrictive or compromise safety.

## Decision

We implement a three-tier plugin classification system:

### 1. WASM Plugins (Sandboxed)
- **Runtime**: WebAssembly with capability-based security
- **Frequency**: 20-120Hz execution
- **Capabilities**: Declared in manifest, enforced at runtime
- **Isolation**: Complete sandboxing, no file/network access by default
- **Use Cases**: Custom telemetry processing, panel logic

### 2. Native Fast-Path Plugins (Isolated)
- **Runtime**: Native code in separate helper process
- **Frequency**: Per-tick execution with μs budget
- **Communication**: Shared memory SPSC queues
- **Isolation**: Process boundary, watchdog protection
- **Use Cases**: High-performance signal processing

### 3. Service Plugins (Managed)
- **Runtime**: Native code in service process
- **Frequency**: Event-driven, no RT constraints
- **Capabilities**: Full system access with user consent
- **Isolation**: Thread-level with resource monitoring
- **Use Cases**: Hardware drivers, external integrations

### Plugin Manifest Example

```json
{
  "name": "custom-telemetry",
  "type": "wasm",
  "version": "1.0.0",
  "capabilities": ["read_bus", "emit_panel"],
  "frequency_hz": 60,
  "signature": "sha256:..."
}
```

## Consequences

### Positive
- Clear security boundaries
- Performance isolation protects RT spine
- Flexible capability system
- Gradual trust model (WASM → Native → Service)

### Negative
- Complex plugin development model
- Multiple runtime environments to maintain
- Capability system adds overhead

## Alternatives Considered

1. **Single Plugin Type**: Rejected due to conflicting requirements
2. **Lua Scripting**: Rejected due to performance limitations
3. **Dynamic Libraries**: Rejected due to security concerns

## Implementation Details

- WASM plugins use wasmtime with capability enforcement
- Native plugins have 100μs budget with overrun quarantine
- Service plugins monitored for resource usage
- All plugins require signature validation

## Security Model

- Capabilities declared upfront, no runtime escalation
- Plugin signature verification required
- Quarantine system isolates misbehaving plugins
- User consent required for privileged operations

## References

- Flight Hub Requirements: PLUG-01, SEC-01
- [WebAssembly Security Model](https://example.com)
- [Capability-Based Security](https://example.com)