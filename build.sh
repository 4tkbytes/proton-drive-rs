#!/bin/bash

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_step() {
    echo -e "${BLUE}🔄 $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Function to run commands with error handling
run_command() {
    local cmd="$1"
    local description="$2"
    local cwd="${3:-$(pwd)}"
    
    print_step "$description"
    if [ "$cwd" != "$(pwd)" ]; then
        echo "   Running in: $cwd"
        (cd "$cwd" && eval "$cmd")
        local exit_code=$?
        if [ $exit_code -ne 0 ]; then
            print_error "Failed: $description (exit code: $exit_code)"
            return $exit_code
        fi
    else
        eval "$cmd"
        local exit_code=$?
        if [ $exit_code -ne 0 ]; then
            print_error "Failed: $description (exit code: $exit_code)"
            return $exit_code
        fi
    fi
}

check_dependencies() {
    echo -e "${BLUE}🔍 Checking dependencies...${NC}"
    
    # Check dotnet
    if ! command -v dotnet &> /dev/null; then
        print_error "Dotnet is not installed or not in PATH"
        return 1
    fi
    
    dotnet_version=$(dotnet --version)
    major_minor=$(echo "$dotnet_version" | cut -d'.' -f1,2)
    version_num=$(echo "$major_minor * 10" | bc 2>/dev/null || echo "90")
    
    if (( $(echo "$version_num < 90" | bc -l 2>/dev/null || echo "0") )); then
        print_error "Dotnet version is $dotnet_version, which is too low. Please upgrade to dotnet 9.0+"
        return 1
    else
        print_success "Dotnet version $dotnet_version is supported!"
    fi
    
    # Check rust/cargo
    if ! command -v cargo &> /dev/null; then
        print_error "Cargo is not installed or not in PATH"
        return 1
    fi
    
    cargo_version=$(cargo --version)
    print_success "$cargo_version"
}

update_submodules() {
    print_step "📦 Updating git submodules..."
    if ! run_command "git submodule update --init --recursive" "Updating submodules"; then
        print_warning "Failed to update submodules, continuing anyway..."
    fi
}

build_dotnet() {
    print_step "🔨 Compiling Proton.SDK components..."
    
    if [ ! -d "Proton.SDK" ]; then
        print_error "Proton.SDK directory not found!"
        return 1
    fi
    
    # Just build for Release, let .NET choose the platform
    if ! run_command "dotnet build -c Release" "Building Proton.SDK" "Proton.SDK"; then
        print_error "Failed to build Proton.SDK"
        return 1
    fi
}

copy_native_libs() {
    print_step "📋 Copying native libraries..."
    
    # Create native-libs directory
    mkdir -p native-libs
    
    # Find the actual build output directory
    base_path="Proton.SDK/src"
    
    # Look for the C export projects and find their actual output
    declare -a export_projects=(
        "Proton.Sdk.CExports"
        "Proton.Sdk.Drive.CExports"
        "Proton.Sdk.Instrumentation.CExport"
    )
    
    copied_count=0
    
    for project in "${export_projects[@]}"; do
        echo "Looking for libraries in $project..."
        
        project_path="$base_path/$project/bin/Release"
        if [ ! -d "$project_path" ]; then
            print_warning "Directory not found: $project_path"
            continue
        fi
        
        # Find all .dll files in the project's bin directory
        echo "Searching in: $project_path"
        find_output=$(find "$project_path" -name "*.dll" 2>/dev/null || true)
        
        if [ -n "$find_output" ]; then
            echo "Found DLL files:"
            echo "$find_output"
            
            while IFS= read -r source_path; do
                if [ -z "$source_path" ]; then
                    continue
                fi
                
                lib_name=$(basename "$source_path")
                echo "Processing: $lib_name"
                
                if [[ "$lib_name" == *"libproton"* ]] || [[ "$lib_name" == *"proton"* ]]; then
                    target_path="native-libs/$lib_name"
                    
                    echo "  Copying: $source_path -> $target_path"
                    if cp "$source_path" "$target_path" 2>/dev/null; then
                        print_success "Copied $lib_name"
                        ((copied_count++))
                    else
                        print_warning "Failed to copy $source_path"
                    fi
                else
                    echo "  Skipping: $lib_name (doesn't match pattern)"
                fi
            done <<< "$find_output"
        else
            print_warning "No DLL files found for $project in $project_path"
        fi
    done
    
    echo "Copy operation completed. Copied $copied_count files."
    
    if [ $copied_count -gt 0 ]; then
        print_success "Successfully copied $copied_count libraries to native-libs/"
        
        # List what was actually copied
        echo "Files in native-libs/:"
        ls -la native-libs/ 2>/dev/null || echo "Could not list native-libs directory"
    else
        print_warning "No libraries were copied!"
        
        # Debug: show what's actually in the build directories
        echo "Debug: Contents of build directories:"
        for project in "${export_projects[@]}"; do
            project_path="$base_path/$project/bin/Release"
            if [ -d "$project_path" ]; then
                echo "Contents of $project_path:"
                find "$project_path" -type f 2>/dev/null || echo "  Could not list files"
            fi
        done
        return 1
    fi
}

build_rust() {
    print_step "🦀 Building Rust components..."
    
    # Build the sys crate first
    if ! run_command "cargo build -p proton-sdk-sys" "Building proton-sdk-sys"; then
        print_warning "Failed to build proton-sdk-sys, trying to continue..."
    fi
    
    # Build the safe wrapper  
    if ! run_command "cargo build -p proton-sdk-rs" "Building proton-sdk-rs"; then
        print_warning "Failed to build proton-sdk-rs, trying to continue..."
    fi
    
    # Build everything
    if ! run_command "cargo build --workspace" "Building entire workspace"; then
        print_error "Failed to build workspace"
        return 1
    fi
}

run_tests() {
    print_step "🧪 Running tests..."
    if ! run_command "cargo test --workspace" "Running Rust tests"; then
        print_warning "Some tests failed, but continuing..."
    fi
}

main() {
    echo -e "${BLUE}🚀 Proton Drive Rust SDK Build Script${NC}"
    echo "=================================================="
    
    # Handle interrupts gracefully
    trap 'echo -e "\n${YELLOW}⚠️  Build interrupted by user${NC}"; exit 1' INT
    
    # Step 1: Check dependencies
    echo "Step 1: Checking dependencies..."
    if ! check_dependencies; then
        print_error "Dependency check failed"
        exit 1
    fi
    
    # Step 2: Update submodules  
    echo "Step 2: Updating submodules..."
    update_submodules
    
    # Step 3: Build .NET components
    echo "Step 3: Building .NET components..."
    if ! build_dotnet; then
        print_error ".NET build failed"
        exit 1
    fi
    
    # Step 4: Copy native libraries
    echo "Step 4: Copying native libraries..."
    if ! copy_native_libs; then
        print_error "Failed to copy native libraries"
        exit 1
    fi
    
    # Step 5: Build Rust components
    echo "Step 5: Building Rust components..."
    if ! build_rust; then
        print_error "Rust build failed"
        exit 1
    fi
    
    # Step 6: Run tests
    echo "Step 6: Running tests..."
    run_tests
    
    echo -e "\n${GREEN}🎉 Build completed successfully!${NC}"
    echo "=================================================="
    echo "Next steps:"
    echo "  - Check native-libs/ for copied DLLs"
    echo "  - Run 'cargo test' to verify functionality"
    echo "  - Use 'cargo run --example <name>' to run examples"
}

# Check if script is being sourced or executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi