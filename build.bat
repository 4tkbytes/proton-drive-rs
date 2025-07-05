@echo off
setlocal enabledelayedexpansion

REM Colors for output (using Windows color codes)
set "RED=[91m"
set "GREEN=[92m"
set "YELLOW=[93m"
set "BLUE=[94m"
set "NC=[0m"

REM Function to print colored output
goto :main

:print_step
echo %BLUE%🔄 %~1%NC%
goto :eof

:print_success
echo %GREEN%✅ %~1%NC%
goto :eof

:print_warning
echo %YELLOW%⚠️  %~1%NC%
goto :eof

:print_error
echo %RED%❌ %~1%NC%
goto :eof

:run_command
set "cmd=%~1"
set "description=%~2"
set "cwd=%~3"

call :print_step "%description%"

if not "%cwd%"=="" (
    echo    Running in: %cwd%
    pushd "%cwd%"
    %cmd%
    set "exit_code=!errorlevel!"
    popd
) else (
    %cmd%
    set "exit_code=!errorlevel!"
)

if !exit_code! neq 0 (
    call :print_error "Failed: %description% (exit code: !exit_code!)"
    exit /b !exit_code!
)
goto :eof

:check_dependencies
echo %BLUE%🔍 Checking dependencies...%NC%

REM Check dotnet
dotnet --version >nul 2>&1
if errorlevel 1 (
    call :print_error "Dotnet is not installed or not in PATH"
    exit /b 1
)

for /f "tokens=*" %%i in ('dotnet --version') do set "dotnet_version=%%i"

REM Extract major.minor version (simplified)
for /f "tokens=1,2 delims=." %%a in ("%dotnet_version%") do (
    set /a "version_num=%%a*10+%%b"
)

if !version_num! lss 90 (
    call :print_error "Dotnet version is %dotnet_version%, which is too low. Please upgrade to dotnet 9.0+"
    exit /b 1
) else (
    call :print_success "Dotnet version %dotnet_version% is supported!"
)

REM Check rust/cargo
cargo --version >nul 2>&1
if errorlevel 1 (
    call :print_error "Cargo is not installed or not in PATH"
    exit /b 1
)

for /f "tokens=*" %%i in ('cargo --version') do set "cargo_version=%%i"
call :print_success "%cargo_version%"
goto :eof

:update_submodules
call :print_step "📦 Updating git submodules..."
call :run_command "git submodule update --init --recursive" "Updating submodules"
if errorlevel 1 (
    call :print_warning "Failed to update submodules, continuing anyway..."
)
goto :eof

:build_dotnet
call :print_step "🔨 Compiling Proton.SDK components..."

if not exist "Proton.SDK" (
    call :print_error "Proton.SDK directory not found!"
    exit /b 1
)

call :run_command "dotnet build -c Release" "Building Proton.SDK" "Proton.SDK"
if errorlevel 1 (
    call :print_error "Failed to build Proton.SDK"
    exit /b 1
)
goto :eof

:copy_native_libs
call :print_step "📋 Copying native libraries..."

REM Create native-libs directory
if not exist "native-libs" mkdir "native-libs"

REM Find the actual build output directory
set "base_path=Proton.SDK\src"

REM Look for the C export projects and find their actual output
set "export_projects=Proton.Sdk.CExports Proton.Sdk.Drive.CExports Proton.Sdk.Instrumentation.CExport"
set "copied_count=0"

for %%p in (%export_projects%) do (
    echo Looking for libraries in %%p...
    
    set "project_path=%base_path%\%%p\bin\Release"
    if not exist "!project_path!" (
        call :print_warning "Directory not found: !project_path!"
    ) else (
        echo Searching in: !project_path!
        
        REM Find all .dll files in the project's bin directory
        for /r "!project_path!" %%f in (*.dll) do (
            set "source_path=%%f"
            set "lib_name=%%~nxf"
            echo Processing: !lib_name!
            
            REM Check if filename contains "libproton" or "proton"
            echo !lib_name! | findstr /i "libproton proton" >nul
            if not errorlevel 1 (
                set "target_path=native-libs\!lib_name!"
                echo   Copying: !source_path! -^> !target_path!
                
                copy "!source_path!" "!target_path!" >nul 2>&1
                if not errorlevel 1 (
                    call :print_success "Copied !lib_name!"
                    set /a "copied_count+=1"
                ) else (
                    call :print_warning "Failed to copy !source_path!"
                )
            ) else (
                echo   Skipping: !lib_name! (doesn't match pattern)
            )
        )
    )
)

echo Copy operation completed. Copied !copied_count! files.

if !copied_count! gtr 0 (
    call :print_success "Successfully copied !copied_count! libraries to native-libs/"
    
    REM List what was actually copied
    echo Files in native-libs/:
    dir /b native-libs\ 2>nul || echo Could not list native-libs directory
) else (
    call :print_warning "No libraries were copied!"
    
    REM Debug: show what's actually in the build directories
    echo Debug: Contents of build directories:
    for %%p in (%export_projects%) do (
        set "project_path=%base_path%\%%p\bin\Release"
        if exist "!project_path!" (
            echo Contents of !project_path!:
            dir /b "!project_path!" 2>nul || echo   Could not list files
        )
    )
    exit /b 1
)
goto :eof

:build_rust
call :print_step "🦀 Building Rust components..."

REM Build the sys crate first
call :run_command "cargo build -p proton-sdk-sys" "Building proton-sdk-sys"
if errorlevel 1 (
    call :print_warning "Failed to build proton-sdk-sys, trying to continue..."
)

REM Build the safe wrapper
call :run_command "cargo build -p proton-sdk-rs" "Building proton-sdk-rs"
if errorlevel 1 (
    call :print_warning "Failed to build proton-sdk-rs, trying to continue..."
)

REM Build everything
call :run_command "cargo build --workspace" "Building entire workspace"
if errorlevel 1 (
    call :print_error "Failed to build workspace"
    exit /b 1
)
goto :eof

:run_tests
call :print_step "🧪 Running tests..."
call :run_command "cargo test --workspace" "Running Rust tests"
if errorlevel 1 (
    call :print_warning "Some tests failed, but continuing..."
)
goto :eof

:main
echo %BLUE%🚀 Proton Drive Rust SDK Build Script%NC%
echo ==================================================

REM Step 1: Check dependencies
echo Step 1: Checking dependencies...
call :check_dependencies
if errorlevel 1 (
    call :print_error "Dependency check failed"
    exit /b 1
)

REM Step 2: Update submodules
echo Step 2: Updating submodules...
call :update_submodules

REM Step 3: Build .NET components
echo Step 3: Building .NET components...
call :build_dotnet
if errorlevel 1 (
    call :print_error ".NET build failed"
    exit /b 1
)

REM Step 4: Copy native libraries
echo Step 4: Copying native libraries...
call :copy_native_libs
if errorlevel 1 (
    call :print_error "Failed to copy native libraries"
    exit /b 1
)

REM Step 5: Build Rust components
echo Step 5: Building Rust components...
call :build_rust
if errorlevel 1 (
    call :print_error "Rust build failed"
    exit /b 1
)

REM Step 6: Run tests
echo Step 6: Running tests...
call :run_tests

echo.
echo %GREEN%🎉 Build completed successfully!%NC%
echo ==================================================
echo Next steps:
echo   - Check native-libs/ for copied DLLs
echo   - Run 'cargo test' to verify functionality
echo   - Use 'cargo run --example ^<name^>' to run examples

pause
goto :eof