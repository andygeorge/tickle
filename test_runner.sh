#!/bin/bash
# test_runner.sh - Comprehensive test runner for tickle

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
TESTS_PASSED=0
TESTS_FAILED=0
TESTS_TOTAL=0

echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}    Tickle Comprehensive Test Suite Runner     ${NC}"
echo -e "${BLUE}================================================${NC}"
echo ""

# Function to print section header
print_section() {
    echo ""
    echo -e "${YELLOW}▶ $1${NC}"
    echo "----------------------------------------"
}

# Function to print success
print_success() {
    echo -e "${GREEN}✓ $1${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
}

# Function to print failure
print_failure() {
    echo -e "${RED}✗ $1${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
}

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: cargo not found. Please install Rust.${NC}"
    exit 1
fi

# 1. Format Check
print_section "Code Formatting Check"
if cargo fmt -- --check 2>&1 | grep -q "Diff"; then
    print_failure "Code formatting issues found"
    echo "Run: cargo fmt"
else
    print_success "Code formatting is correct"
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

# 2. Clippy (linting)
print_section "Clippy Linting"
if ! cargo clippy --version &> /dev/null; then
    echo -e "${YELLOW}⚠ cargo-clippy not installed${NC}"
    echo "  Install with: rustup component add clippy"
else
    if cargo clippy --all-targets --all-features -- -D warnings &> /dev/null; then
        print_success "Clippy checks passed"
    else
        print_failure "Clippy found issues"
        cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -20
    fi
    TESTS_TOTAL=$((TESTS_TOTAL + 1))
fi

# 3. Build
print_section "Build Project"
if cargo build 2>&1 | grep -q "error"; then
    print_failure "Build failed"
    cargo build 2>&1 | grep "error" | head -10
    exit 1
else
    print_success "Build successful"
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

# 4. Unit Tests
print_section "Unit Tests"
echo "Running unit tests..."
if cargo test --bin tickle -- --test-threads=1 2>&1 | tee /tmp/tickle_unit_tests.log | grep -q "test result: ok"; then
    UNIT_COUNT=$(grep "test result: ok" /tmp/tickle_unit_tests.log | grep -oP '\d+ passed' | grep -oP '\d+')
    print_success "Unit tests passed ($UNIT_COUNT tests)"
    TESTS_TOTAL=$((TESTS_TOTAL + UNIT_COUNT))
    TESTS_PASSED=$((TESTS_PASSED + UNIT_COUNT - 1))  # -1 because we already counted this category
else
    print_failure "Some unit tests failed"
    grep "FAILED" /tmp/tickle_unit_tests.log || true
fi

# 5. Integration Tests
print_section "Integration Tests"
echo "Running integration tests..."
if cargo test --test integration_tests -- --test-threads=1 2>&1 | tee /tmp/tickle_integration_tests.log | grep -q "test result: ok"; then
    INT_COUNT=$(grep "test result: ok" /tmp/tickle_integration_tests.log | grep -oP '\d+ passed' | grep -oP '\d+')
    print_success "Integration tests passed ($INT_COUNT tests)"
    TESTS_TOTAL=$((TESTS_TOTAL + INT_COUNT))
    TESTS_PASSED=$((TESTS_PASSED + INT_COUNT - 1))
else
    print_failure "Some integration tests failed"
    grep "FAILED" /tmp/tickle_integration_tests.log || true
fi

# 6. Documentation Tests
print_section "Documentation Tests"
if cargo metadata --no-deps 2>/dev/null | grep -q '"lib"'; then
    if cargo test --doc 2>&1 | grep -q "test result: ok"; then
        print_success "Documentation tests passed"
    else
        print_failure "Documentation tests failed"
    fi
    TESTS_TOTAL=$((TESTS_TOTAL + 1))
else
    echo -e "${YELLOW}⚠ No library target found, skipping doc tests${NC}"
fi

# 7. Release Build
print_section "Release Build"
if cargo build --release &> /dev/null; then
    print_success "Release build successful"
    
    # Check binary size
    BINARY_SIZE=$(du -h target/release/tickle 2>/dev/null | cut -f1)
    echo "  Binary size: $BINARY_SIZE"
else
    print_failure "Release build failed"
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

# 8. Test Coverage (if tarpaulin is installed)
print_section "Code Coverage"
if command -v cargo-tarpaulin &> /dev/null; then
    echo "Generating coverage report..."
    if cargo tarpaulin --out Stdout --output-dir coverage --skip-clean 2>&1 | tee /tmp/tickle_coverage.log | grep -q "Coverage:"; then
        COVERAGE=$(grep "Coverage:" /tmp/tickle_coverage.log | tail -1)
        print_success "Coverage report generated"
        echo "  $COVERAGE"
    else
        print_failure "Coverage generation failed"
    fi
else
    echo -e "${YELLOW}⚠ cargo-tarpaulin not installed${NC}"
    echo "  Install with: cargo install cargo-tarpaulin"
    echo "  To generate coverage reports"
fi

# 9. Binary Functionality Test
print_section "Binary Smoke Tests"
BINARY="./target/release/tickle"

if [ -f "$BINARY" ]; then
    # Test --help
    if $BINARY --help &> /dev/null; then
        print_success "Binary --help works"
    else
        print_failure "Binary --help failed"
    fi
    TESTS_TOTAL=$((TESTS_TOTAL + 1))
    
    # Test --version
    if $BINARY --version &> /dev/null; then
        print_success "Binary --version works"
    else
        print_failure "Binary --version failed"
    fi
    TESTS_TOTAL=$((TESTS_TOTAL + 1))
    
    # Test history command
    if HOME=/tmp/tickle_smoke_test $BINARY history &> /dev/null; then
        print_success "Binary history command works"
        rm -rf /tmp/tickle_smoke_test
    else
        print_failure "Binary history command failed"
    fi
    TESTS_TOTAL=$((TESTS_TOTAL + 1))
else
    print_failure "Release binary not found"
    TESTS_TOTAL=$((TESTS_TOTAL + 3))
fi

# Summary
echo ""
echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}              Test Summary                      ${NC}"
echo -e "${BLUE}================================================${NC}"
echo ""
echo "Total Tests:  $TESTS_TOTAL"
echo -e "Passed:       ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed:       ${RED}$TESTS_FAILED${NC}"
echo ""

# Calculate success rate
if [ $TESTS_TOTAL -gt 0 ]; then
    SUCCESS_RATE=$((TESTS_PASSED * 100 / TESTS_TOTAL))
    echo "Success Rate: $SUCCESS_RATE%"
fi

# Overall result
echo ""
if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some tests failed.${NC}"
    echo ""
    echo "Review the output above for details."
    echo "Run individual test suites for more information:"
    echo "  cargo test --lib           # Unit tests"
    echo "  cargo test --test '*'      # Integration tests"
    exit 1
fi
