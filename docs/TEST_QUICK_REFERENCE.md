# Tickle Testing Quick Reference

## Quick Start

```bash
# Run all tests
cargo test

# Run with output visible
cargo test -- --nocapture

# Run specific test
cargo test test_history_log_command_success
```

## Test Commands Cheat Sheet

| Command | Purpose |
|---------|---------|
| `cargo test` | Run all tests |
| `cargo test --lib` | Run only unit tests |
| `cargo test --test integration_tests` | Run only integration tests |
| `cargo test history` | Run tests matching "history" |
| `cargo test -- --nocapture` | Show println! output |
| `cargo test -- --test-threads=1` | Run tests serially |
| `cargo build` | Build debug binary |
| `cargo build --release` | Build optimized binary |
| `cargo clippy` | Run linter |
| `cargo fmt` | Format code |
| `cargo fmt -- --check` | Check formatting |
| `./test_runner.sh` | Run comprehensive test suite |

## Common Test Scenarios

### 1. Before Committing
```bash
cargo fmt
cargo clippy
cargo test
```

### 2. Full Test Suite
```bash
./test_runner.sh
```

### 3. Debug Failing Test
```bash
RUST_BACKTRACE=1 cargo test test_name -- --nocapture
```

### 4. Check Code Coverage
```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
# Open tarpaulin-report.html
```

### 5. Test Specific Module
```bash
cargo test history_manager
cargo test command_parsing
```

## Test File Locations

```
tickle/
├── src/
│   └── main.rs          # Unit tests at bottom (mod tests)
├── tests/
│   └── integration_tests.rs  # Integration tests
└── test_runner.sh       # Comprehensive test script
```

## Understanding Test Output

### Success
```
test test_name ... ok
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured
```

### Failure
```
test test_name ... FAILED

failures:
    test_name

test result: FAILED. 34 passed; 1 failed; 0 ignored; 0 measured
```

## Debugging Tips

### View Test Details
```bash
cargo test -- --nocapture --test-threads=1
```

### Run Single Test with Full Output
```bash
cargo test test_history_log_command_success -- --exact --nocapture
```

### See What Tests Exist
```bash
cargo test -- --list
```

### Test Temporary Files
```bash
# Tests create temp directories in /tmp/
ls -la /tmp/tickle_test_*
```

## Test Coverage Goals

- Unit Tests: >90% coverage
- Integration Tests: >80% coverage
- Overall: >85% coverage

## CI/CD Integration

### GitHub Actions
Place `.github/workflows/ci.yml` in your repo:
```bash
mkdir -p .github/workflows
cp github_actions_ci.yml .github/workflows/ci.yml
```

### Local CI Simulation
```bash
# Simulate what CI will run
./test_runner.sh
```

## Common Issues & Solutions

### Issue: Tests fail with permission errors
**Solution:** Tests use temporary directories, ensure /tmp is writable

### Issue: Integration tests can't find binary
**Solution:** Run `cargo build` first

### Issue: Concurrent test failures
**Solution:** Run tests serially: `cargo test -- --test-threads=1`

### Issue: History tests interfere with each other
**Solution:** Tests use unique temp directories, shouldn't happen

## Test Maintenance

### Adding New Tests
1. Write test function with `#[test]` attribute
2. Use descriptive name: `test_component_scenario`
3. Add to appropriate module (unit vs integration)
4. Document what it tests

### Updating Tests
- Modify existing tests when changing functionality
- Add regression tests for fixed bugs
- Keep test documentation current

## Performance Benchmarks

```bash
# Time all tests
time cargo test

# Time specific test
time cargo test test_history_large_number_of_entries
```

## Quick Validation

Before pushing code:
```bash
cargo fmt && \
cargo clippy -- -D warnings && \
cargo test && \
echo "✓ All checks passed!"
```

## Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Cargo Test Documentation](https://doc.rust-lang.org/cargo/commands/cargo-test.html)
- Full test documentation: `TEST_DOCUMENTATION.md`
