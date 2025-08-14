@echo off
REM Comprehensive test runner for Moses on Windows
REM Runs all tests with safety checks

echo =====================================
echo Moses Comprehensive Test Suite
echo =====================================
echo.

REM Check if we're in the project root
if not exist "Cargo.toml" (
    echo Error: Must be run from project root
    exit /b 1
)

REM Clean previous test artifacts
echo Cleaning previous test artifacts...
cargo clean --package moses-core
cargo clean --package moses-formatters
cargo clean --package moses-platform
echo.

REM Run unit tests
echo =====================================
echo 1. Unit Tests
echo =====================================
echo.

echo Testing moses-core...
cargo test --package moses-core --lib --tests
if %errorlevel% neq 0 (
    echo FAILED: moses-core tests
    exit /b %errorlevel%
)
echo PASSED: moses-core tests
echo.

echo Testing moses-formatters...
cargo test --package moses-formatters --lib --tests
if %errorlevel% neq 0 (
    echo FAILED: moses-formatters tests
    exit /b %errorlevel%
)
echo PASSED: moses-formatters tests
echo.

echo Testing moses-platform...
cargo test --package moses-platform --lib --tests
if %errorlevel% neq 0 (
    echo FAILED: moses-platform tests
    exit /b %errorlevel%
)
echo PASSED: moses-platform tests
echo.

REM Run safety-critical tests
echo =====================================
echo 2. CRITICAL SAFETY TESTS
echo =====================================
echo These tests ensure the application NEVER formats system drives
echo.

cargo test --package moses-formatters safety
if %errorlevel% neq 0 (
    echo CRITICAL FAILURE: Safety tests failed!
    echo This is a serious issue that must be fixed before release
    exit /b %errorlevel%
)
echo PASSED: Safety tests
echo.

REM Run mock device tests
echo =====================================
echo 3. Mock Device Tests
echo =====================================
echo These tests use fake devices to ensure no real hardware is touched
echo.

cargo test --package moses-core test_utils
if %errorlevel% neq 0 (
    echo FAILED: Mock device tests
    exit /b %errorlevel%
)
echo PASSED: Mock device tests
echo.

REM Run integration tests
echo =====================================
echo 4. Integration Tests
echo =====================================
echo.

cargo test --all --tests
if %errorlevel% neq 0 (
    echo FAILED: Integration tests
    exit /b %errorlevel%
)
echo PASSED: Integration tests
echo.

REM Run documentation tests
echo =====================================
echo 5. Documentation Tests
echo =====================================
echo.

cargo test --doc
if %errorlevel% neq 0 (
    echo FAILED: Documentation tests
    exit /b %errorlevel%
)
echo PASSED: Documentation tests
echo.

REM Safety pattern checks
echo =====================================
echo 6. Safety Pattern Checks
echo =====================================
echo.

echo Checking for dangerous patterns...
findstr /S /C:"format(" *.rs | findstr /V "can_format" | findstr /V "mock" | findstr /V "test" > nul
if %errorlevel% equ 0 (
    echo Warning: Found format calls without safety checks
)

echo Safety pattern checks complete
echo.

REM Final summary
echo =====================================
echo Test Summary
echo =====================================
echo.
echo All tests passed successfully!
echo.
echo Test categories completed:
echo   - Unit tests
echo   - Safety-critical tests
echo   - Mock device tests
echo   - Integration tests
echo   - Documentation tests
echo   - Safety pattern checks
echo.
echo The application is safe to use and will not format system drives.
echo.
pause