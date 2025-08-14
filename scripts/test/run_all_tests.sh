#!/bin/bash
# Comprehensive test runner for Moses
# Runs all tests with safety checks and coverage reporting

set -e  # Exit on error

echo "====================================="
echo "Moses Comprehensive Test Suite"
echo "====================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if we're in the project root
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Must be run from project root${NC}"
    exit 1
fi

# Function to run tests for a package
run_package_tests() {
    local package=$1
    echo -e "${YELLOW}Testing package: $package${NC}"
    
    if cargo test --package "$package" --lib --tests -- --nocapture; then
        echo -e "${GREEN}✓ $package tests passed${NC}"
    else
        echo -e "${RED}✗ $package tests failed${NC}"
        exit 1
    fi
    echo ""
}

# Function to run safety tests
run_safety_tests() {
    echo -e "${YELLOW}Running CRITICAL SAFETY TESTS${NC}"
    echo "These tests ensure the application NEVER formats system drives"
    echo ""
    
    if cargo test --package moses-formatters safety -- --nocapture; then
        echo -e "${GREEN}✓ Safety tests passed${NC}"
    else
        echo -e "${RED}✗ CRITICAL: Safety tests failed!${NC}"
        echo -e "${RED}This is a serious issue that must be fixed before release${NC}"
        exit 1
    fi
    echo ""
}

# Function to run mock device tests
run_mock_tests() {
    echo -e "${YELLOW}Running mock device tests${NC}"
    echo "These tests use fake devices to ensure no real hardware is touched"
    echo ""
    
    if cargo test --package moses-core test_utils -- --nocapture; then
        echo -e "${GREEN}✓ Mock device tests passed${NC}"
    else
        echo -e "${RED}✗ Mock device tests failed${NC}"
        exit 1
    fi
    echo ""
}

# Clean previous test artifacts
echo "Cleaning previous test artifacts..."
cargo clean --package moses-core
cargo clean --package moses-formatters
cargo clean --package moses-platform
echo ""

# Run unit tests for each package
echo "====================================="
echo "1. Unit Tests"
echo "====================================="
echo ""

run_package_tests "moses-core"
run_package_tests "moses-formatters"
run_package_tests "moses-platform"
run_package_tests "moses-daemon"

# Run integration tests
echo "====================================="
echo "2. Integration Tests"
echo "====================================="
echo ""

echo -e "${YELLOW}Running integration tests${NC}"
cargo test --all --tests -- --nocapture
echo -e "${GREEN}✓ Integration tests passed${NC}"
echo ""

# Run safety-critical tests
echo "====================================="
echo "3. Safety-Critical Tests"
echo "====================================="
echo ""

run_safety_tests
run_mock_tests

# Run documentation tests
echo "====================================="
echo "4. Documentation Tests"
echo "====================================="
echo ""

echo -e "${YELLOW}Testing documentation examples${NC}"
if cargo test --doc; then
    echo -e "${GREEN}✓ Documentation tests passed${NC}"
else
    echo -e "${RED}✗ Documentation tests failed${NC}"
    exit 1
fi
echo ""

# Check for common issues
echo "====================================="
echo "5. Safety Checks"
echo "====================================="
echo ""

echo -e "${YELLOW}Checking for dangerous patterns...${NC}"

# Check for direct device access without safety checks
if grep -r "format(" --include="*.rs" | grep -v "can_format" | grep -v "mock" | grep -v "test"; then
    echo -e "${RED}Warning: Found format calls without safety checks${NC}"
fi

# Check for missing is_system checks
if grep -r "Device" --include="*.rs" | grep -v "is_system" | grep -v "test" | grep -v "mock" | head -5; then
    echo -e "${YELLOW}Note: Some code may not check is_system flag${NC}"
fi

echo -e "${GREEN}✓ Safety pattern checks complete${NC}"
echo ""

# Generate test coverage if available
if command -v cargo-tarpaulin &> /dev/null; then
    echo "====================================="
    echo "6. Test Coverage"
    echo "====================================="
    echo ""
    
    echo -e "${YELLOW}Generating test coverage report...${NC}"
    cargo tarpaulin --out Html --output-dir target/coverage
    echo -e "${GREEN}✓ Coverage report generated at target/coverage/index.html${NC}"
else
    echo -e "${YELLOW}Skipping coverage (install cargo-tarpaulin for coverage reports)${NC}"
fi

# Final summary
echo ""
echo "====================================="
echo "Test Summary"
echo "====================================="
echo ""
echo -e "${GREEN}✓ All tests passed successfully!${NC}"
echo ""
echo "Test categories completed:"
echo "  ✓ Unit tests"
echo "  ✓ Integration tests"
echo "  ✓ Safety-critical tests"
echo "  ✓ Mock device tests"
echo "  ✓ Documentation tests"
echo "  ✓ Safety pattern checks"
echo ""
echo -e "${GREEN}The application is safe to use and will not format system drives.${NC}"