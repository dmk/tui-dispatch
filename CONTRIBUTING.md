# Contributing to tui-dispatch

Thanks for your interest in contributing!

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/tui-dispatch.git`
3. Create a branch: `git checkout -b feature/your-feature`

## Development

### Prerequisites

- Rust 1.85+ (edition 2024)
- Make (optional, for convenience)

### Building

```bash
make build          # Debug build
make release        # Release build
```

Or use cargo directly:

```bash
cargo build
cargo build --release
```

### Testing

```bash
make test
# or
cargo test
```

## Code Quality

Before submitting a PR, please ensure:

1. **Format**: Code is formatted with `rustfmt`
   ```bash
   make fmt
   ```

2. **Lint**: No clippy warnings
   ```bash
   make clippy
   ```

3. **Tests**: All tests pass
   ```bash
   make test
   ```

4. **Full check**: Run the complete verification suite
   ```bash
   make verify
   ```

## Pull Request Process

1. Update the README if needed
2. Add tests for new functionality
3. Update the CHANGELOG
4. Ensure `make verify` passes
5. Submit PR with clear description

## Questions?

Open an issue or discussion on GitHub!
