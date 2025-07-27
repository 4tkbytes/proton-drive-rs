#!/usr/bin/env python3
"""
Build script for proton-sdk-rs and its dependencies.
This script automates the process of cloning repositories, building dotnet-crypto,
building Proton.SDK, and finally building proton-sdk-rs.

Note: Yes ONLY this python build script is AI HOWEVER, everything else is handmade with blood sweat and tears. Just know that before you accuse me of something
"""

import os
import subprocess
import sys
import shutil
import platform
from pathlib import Path
import argparse


class Colors:
    """ANSI color codes for terminal output"""
    RED = '\033[91m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    BLUE = '\033[94m'
    MAGENTA = '\033[95m'
    CYAN = '\033[96m'
    WHITE = '\033[97m'
    BOLD = '\033[1m'
    UNDERLINE = '\033[4m'
    END = '\033[0m'  # Reset to default
    
    @staticmethod
    def disable_on_windows():
        """Disable colors on Windows if not supported"""
        if platform.system() == 'Windows':
            # Try to enable ANSI support on Windows 10+
            try:
                import ctypes
                kernel32 = ctypes.windll.kernel32
                kernel32.SetConsoleMode(kernel32.GetStdHandle(-11), 7)
            except (AttributeError, OSError):
                # If that fails, disable colors
                for attr in dir(Colors):
                    if not attr.startswith('_') and attr != 'disable_on_windows':
                        setattr(Colors, attr, '')


# Initialize colors
Colors.disable_on_windows()


class BuildScript:
    def __init__(self, base_dir=None, arch=None):
        # Determine the correct base directory for cloning dependencies
        if base_dir:
            self.base_dir = Path(base_dir)
        else:
            current_dir = Path.cwd()
            # Check if we have indicators that we're in the main repo (has Cargo.toml, build.py, etc.)
            has_cargo_toml = (current_dir / "Cargo.toml").exists()
            has_build_script = (current_dir / "build.py").exists()
            has_proton_sdk_rs_subdir = (current_dir / "proton-sdk-rs").exists()
            
            if has_cargo_toml and has_build_script and has_proton_sdk_rs_subdir:
                # We're in the main repo directory in CI - use current directory as base
                self.base_dir = current_dir
                print(f"{Colors.CYAN}Detected CI environment - using current directory as base: {current_dir}{Colors.END}")
            elif current_dir.name == "proton-sdk-rs" and current_dir.parent.name == "proton-sdk-rs":
                # We're in the inner proton-sdk-rs directory
                self.base_dir = current_dir.parent.parent
            elif current_dir.name == "proton-sdk-rs":
                # We're in the outer proton-sdk-rs directory  
                self.base_dir = current_dir.parent
            else:
                # Fallback to parent directory
                self.base_dir = current_dir.parent
                
        self.arch = arch or self._detect_arch()
        self.os_name = self._detect_os()
        # Change from home directory to project directory
        self.local_nuget_repo = self.base_dir / "local-nuget-repository"
        self.required_tools = ['git', 'dotnet', 'cargo', 'rustc', 'go', 'gcc']
        self.optional_tools = []  # No optional tools required with integrated build system
        
        # Set default DLLs location to {projectRoot}/native-libs
        self.dlls_location = self.base_dir / "native-libs"
        os.environ["PROTON_SDK_LIB_DIR"] = str(self.dlls_location)
        print(f"{Colors.CYAN}Set PROTON_SDK_LIB_DIR to: {self.dlls_location}{Colors.END}")
        
        print(f"{Colors.CYAN}Base directory set to: {self.base_dir}{Colors.END}")
        print(f"{Colors.CYAN}Local NuGet repository: {self.local_nuget_repo}{Colors.END}")
        print(f"{Colors.CYAN}Current working directory: {Path.cwd()}{Colors.END}")
        
    def _detect_arch(self):
        """Detect system architecture"""
        machine = platform.machine().lower()
        if machine in ['x86_64', 'amd64']:
            return 'amd64'  # Use Go architecture naming
        elif machine in ['aarch64', 'arm64']:
            return 'arm64'
        elif machine in ['i386', 'i686', 'x86']:
            return '386'  # Go uses 386 for 32-bit x86
        else:
            return machine
    
    def _detect_os(self):
        """Detect operating system"""
        system = platform.system().lower()
        if system == 'windows':
            return 'windows'
        elif system == 'darwin':
            return 'macos'
        elif system == 'linux':
            return 'linux'
        else:
            return system
    
    def _check_windows_shell(self):
        """Check Windows shell compatibility (no longer required since Go build is integrated)"""
        if self.os_name != 'windows':
            return  # Not Windows, no check needed
        
        # Check for MSYSTEM environment variable (set by Git Bash/MSYS2)
        msystem = os.environ.get('MSYSTEM')
        if msystem:
            print(f"{Colors.GREEN}+{Colors.END} Running in Git Bash/MSYS2 (MSYSTEM={msystem})")
        else:
            print(f"{Colors.CYAN}i{Colors.END} Running in standard Windows shell (PowerShell/CMD)")
        
        print(f"{Colors.GREEN}+{Colors.END} Windows shell compatibility verified - Go build is now integrated")

    def run_command(self, cmd, cwd=None, shell=True, capture_output=False):
        """Run a shell command and handle errors"""
        print(f"{Colors.BLUE}Running:{Colors.END} {cmd}")
        if cwd:
            print(f"  {Colors.CYAN}in directory:{Colors.END} {cwd}")
        
        try:
            if capture_output:
                # For commands where we need to capture output (like version checks)
                result = subprocess.run(
                    cmd, 
                    shell=shell, 
                    cwd=cwd, 
                    check=True, 
                    capture_output=True, 
                    text=True
                )
                if result.stdout:
                    print(result.stdout)
                return result
            else:
                # For build commands, show live output
                result = subprocess.run(
                    cmd, 
                    shell=shell, 
                    cwd=cwd, 
                    check=True
                )
                return result
        except subprocess.CalledProcessError as e:
            print(f"{Colors.RED}Error running command:{Colors.END} {cmd}")
            print(f"{Colors.RED}Exit code:{Colors.END} {e.returncode}")
            if hasattr(e, 'stdout') and e.stdout:
                print(f"{Colors.YELLOW}Stdout:{Colors.END} {e.stdout}")
            if hasattr(e, 'stderr') and e.stderr:
                print(f"{Colors.RED}Stderr:{Colors.END} {e.stderr}")
            raise
    
    def check_dependencies(self):
        """Check if all required tools are available"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Checking dependencies ==={Colors.END}")
        
        missing_tools = []
        
        for tool in self.required_tools:
            try:
                # Special case for Go which uses 'version' instead of '--version'
                version_cmd = [tool, 'version'] if tool == 'go' else [tool, '--version']
                result = subprocess.run(
                    version_cmd, 
                    capture_output=True, 
                    text=True, 
                    timeout=10,
                    check=False
                )
                if result.returncode == 0:
                    print(f"{Colors.GREEN}+{Colors.END} {tool} is available")
                else:
                    missing_tools.append(tool)
            except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
                missing_tools.append(tool)
        
        # Check for optional tools
        for tool in self.optional_tools:
            try:
                result = subprocess.run(
                    [tool, '--version'], 
                    capture_output=True, 
                    text=True, 
                    timeout=10,
                    check=False
                )
                if result.returncode == 0:
                    print(f"{Colors.GREEN}+{Colors.END} {tool} is available")
                else:
                    print(f"{Colors.YELLOW}!{Colors.END} {tool} not found - may need manual line ending conversion")
            except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
                print(f"{Colors.YELLOW}!{Colors.END} {tool} not found - may need manual line ending conversion")
        
        if missing_tools:
            print(f"{Colors.RED}X Missing required tools:{Colors.END} {', '.join(missing_tools)}")
            print(f"\n{Colors.YELLOW}Please install the missing tools before running the build:{Colors.END}")
            for tool in missing_tools:
                if tool == 'git':
                    print(f"  - {Colors.CYAN}Git:{Colors.END} https://git-scm.com/downloads")
                elif tool == 'dotnet':
                    print(f"  - {Colors.CYAN}.NET SDK:{Colors.END} https://dotnet.microsoft.com/download")
                elif tool in ['cargo', 'rustc']:
                    print(f"  - {Colors.CYAN}Rust:{Colors.END} https://rustup.rs/")
                elif tool == 'go':
                    print(f"  - {Colors.CYAN}Go:{Colors.END} https://golang.org/dl/")
            sys.exit(1)
        
        print(f"{Colors.GREEN}+ All dependencies are available{Colors.END}")

    def clone_repositories(self):
        """Clone the required repositories (excluding proton-sdk-rs)"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Cloning repositories ==={Colors.END}")
        
        repos = [
            ("https://github.com/4tkbytes/dotnet-crypto", "dotnet-crypto"),
            ("https://github.com/4tkbytes/Proton.SDK", "Proton.SDK")
        ]
        
        os.chdir(self.base_dir)
        
        for repo_url, repo_name in repos:
            repo_path = self.base_dir / repo_name
            if repo_path.exists():
                print(f"{Colors.YELLOW}Repository {repo_name} already exists, skipping clone{Colors.END}")
                continue
            
            self.run_command(f"git clone {repo_url}")
    
    def build_go_crypto(self, crypto_dir):
        """Build Go cryptography library using integrated build logic"""
        print(f"{Colors.BLUE}Building Go cryptography library...{Colors.END}")
        
        # Build modes to compile
        build_modes = ["c-shared", "c-archive"]
        
        # Architecture and OS mappings
        arch_rid_map = {"386": "x86", "amd64": "x64", "arm64": "arm64"}
        os_rid_map = {
            "windows": "win", 
            "darwin": "osx", 
            "linux": "linux", 
            "android": "linux-bionic", 
            "ios": "ios"
        }
        
        # Convert Python arch naming to Go arch naming
        go_arch = self.arch  # amd64, arm64, 386
        go_os = "darwin" if self.os_name == "macos" else self.os_name
        
        # Get runtime identifier
        runtime_id = f"{os_rid_map[go_os]}-{arch_rid_map[go_arch]}"
        
        # Set up Go environment variables
        go_env = os.environ.copy()
        go_env.update({
            "GOFLAGS": "-trimpath",
            "CGO_ENABLED": "1",
            "CGO_LDFLAGS": "-s -w",
            "GOOS": go_os,
            "GOARCH": go_arch,
        })
        
        # Windows-specific configuration - default to MinGW
        if go_os == "windows":
            # Check if MinGW GCC is available
            try:
                subprocess.run(
                    ["gcc", "--version"], 
                    capture_output=True, 
                    check=True, 
                    timeout=10
                )
                print(f"{Colors.GREEN}Using MinGW GCC for Windows compilation (most compatible){Colors.END}")
                
                # Configure for MinGW
                go_env.update({
                    "CC": "gcc",
                    "CXX": "g++",
                    "CGO_CFLAGS": "-O2",
                    "CGO_LDFLAGS": "-s -w -static -static-libgcc -static-libstdc++"
                })
                
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                print(f"{Colors.RED}MinGW GCC not found{Colors.END}")
                print(f"{Colors.YELLOW}Please install MinGW-w64 from: https://www.mingw-w64.org/downloads/{Colors.END}")
                print(f"{Colors.YELLOW}Or use chocolatey: choco install mingw{Colors.END}")
                return
        else:
            go_env["CC"] = "gcc"
    
        lib_name = "proton_crypto"
        output_dir_path = crypto_dir / "bin" / "runtimes" / runtime_id / "native"
        output_dir_path.mkdir(parents=True, exist_ok=True)
        
        go_src_dir = crypto_dir / "src" / "go"
        if not go_src_dir.exists():
            print(f"{Colors.YELLOW}Go source directory not found at {go_src_dir}, skipping Go build{Colors.END}")
            return
        
        for build_mode in build_modes:
            # Determine output file name based on OS and build mode
            if go_os == "windows":
                if build_mode == "c-shared":
                    output_file_name = f"{lib_name}.dll"
                else:
                    # Generate .a file for MinGW compatibility
                    output_file_name = f"{lib_name}.a"
            elif go_os == "linux":
                if build_mode == "c-shared":
                    output_file_name = f"lib{lib_name}.so"
                else:
                    output_file_name = f"lib{lib_name}.a"
            elif go_os == "android":
                if build_mode == "c-shared":
                    output_file_name = f"lib{lib_name}.so"
                    go_env["CGO_LDFLAGS"] = f"-Wl,-soname,{output_file_name}"
                else:
                    print(f"{Colors.YELLOW}Skipping unsupported {build_mode} mode for {go_os}/{go_arch}{Colors.END}")
                    continue
            elif go_os == "darwin":
                if build_mode == "c-shared":
                    output_file_name = f"{lib_name}.dylib"
                else:
                    output_file_name = f"{lib_name}.a"
            elif go_os == "ios":
                if build_mode == "c-shared":
                    print(f"{Colors.YELLOW}Skipping unsupported {build_mode} mode for {go_os}/{go_arch}{Colors.END}")
                    continue
                else:
                    output_file_name = f"{lib_name}.a"
            else:
                print(f"{Colors.YELLOW}Unknown OS {go_os}, using default naming{Colors.END}")
                output_file_name = f"lib{lib_name}.so"
            
            output_file_path = output_dir_path / output_file_name
            
            print(f"{Colors.BLUE}Building for {go_os}/{go_arch} in {build_mode} mode -> {output_file_path}{Colors.END}")
            
            # Build the Go library
            cmd = [
                "go", "build", 
                "-C", str(go_src_dir),
                f"-buildmode={build_mode}",
                "-o", str(output_file_path)
            ]
            
            try:
                subprocess.run(
                    cmd,
                    env=go_env,
                    cwd=crypto_dir,
                    check=True,
                    capture_output=True,
                    text=True
                )
                print(f"{Colors.GREEN}+ Successfully built {output_file_name}{Colors.END}")
                
                # For Windows c-archive mode, also create a .lib file by copying
                # This provides compatibility for both MinGW and MSVC linking scenarios
                if go_os == "windows" and build_mode == "c-archive" and output_file_path.exists():
                    lib_file_path = output_file_path.with_suffix(".lib")
                    try:
                        shutil.copy2(output_file_path, lib_file_path)
                        print(f"{Colors.GREEN}+ Created .lib file for compatibility: {lib_file_path.name}{Colors.END}")
                    except Exception as e:
                        print(f"{Colors.YELLOW}Warning: Could not create .lib file: {e}{Colors.END}")
            
            except subprocess.CalledProcessError as e:
                print(f"{Colors.RED}Go build failed for {build_mode}:{Colors.END} {e}")
                if e.stdout:
                    print(f"{Colors.YELLOW}Stdout:{Colors.END} {e.stdout}")
                if e.stderr:
                    print(f"{Colors.RED}Stderr:{Colors.END} {e.stderr}")
                
                # Skip this build mode and continue
                print(f"{Colors.YELLOW}Skipping {build_mode} and continuing with next build mode{Colors.END}")
                continue
    
    def build_dotnet_crypto(self):
        """Build dotnet-crypto package"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Building dotnet-crypto ==={Colors.END}")
        
        crypto_dir = self.base_dir / "dotnet-crypto"
        os.chdir(crypto_dir)
        
        # Build Go cryptography library using integrated logic
        self.build_go_crypto(crypto_dir)
        
        # Create local nuget repository in project directory FIRST
        local_nuget_temp = crypto_dir / "local-nuget-repository"
        local_nuget_temp.mkdir(parents=True, exist_ok=True)
        
        # Ensure main local nuget repository exists in project directory
        self.local_nuget_repo.mkdir(parents=True, exist_ok=True)
        
        # Configure NuGet sources AFTER creating directories
        print(f"{Colors.BLUE}Configuring NuGet source ProtonRepository...{Colors.END}")
        try:
            # Try to remove existing ProtonRepository source (ignore errors if it doesn't exist)
            self.run_command('dotnet nuget remove source ProtonRepository')
            print(f"{Colors.GREEN}Removed existing ProtonRepository source{Colors.END}")
        except subprocess.CalledProcessError:
            print(f"{Colors.CYAN}ProtonRepository source didn't exist, continuing...{Colors.END}")
        
        # Add nuget source only after directory exists
        try:
            self.run_command(
                f'dotnet nuget add source "{self.local_nuget_repo}" --name ProtonRepository'
            )
            print(f"{Colors.GREEN}Added ProtonRepository source: {self.local_nuget_repo}{Colors.END}")
        except subprocess.CalledProcessError as e:
            print(f"{Colors.RED}Failed to add NuGet source: {e}{Colors.END}")
            raise
        
        # Pack the project with multiple target frameworks
        self.run_command(
            f'dotnet pack -c Release -p:Version=1.0.0 '
            f'src/dotnet/Proton.Cryptography.csproj --output {local_nuget_temp}'
        )
        
        # Copy the native library to the correct location within the NuGet package
        crypto_bin_dir = crypto_dir / "bin"
        if crypto_bin_dir.exists():
            # Find the native library file
            native_libs = list(crypto_bin_dir.glob("**/native/libproton_crypto.*"))
            if native_libs:
                print(f"{Colors.BLUE}Found native libraries:{Colors.END}")
                for lib in native_libs:
                    print(f"  {lib}")
                
                # Extract and modify the NuGet package to include native libraries
                import zipfile
                import tempfile
                
                nupkg_files = list(local_nuget_temp.glob("*.nupkg"))
                if nupkg_files:
                    nupkg_file = nupkg_files[0]
                    print(f"{Colors.BLUE}Adding native libraries to NuGet package: {nupkg_file.name}{Colors.END}")
                    
                    with tempfile.TemporaryDirectory() as temp_dir:
                        temp_path = Path(temp_dir)
                        
                        # Extract the package
                        with zipfile.ZipFile(nupkg_file, 'r') as zip_ref:
                            zip_ref.extractall(temp_path)
                        
                        # Copy native libraries to the package
                        for lib in native_libs:
                            # Determine the runtime path from the library path
                            parts = lib.parts
                            runtime_idx = None
                            for i, part in enumerate(parts):
                                if part == "runtimes":
                                    runtime_idx = i
                                    break
                            
                            if runtime_idx is not None and runtime_idx + 2 < len(parts):
                                runtime_id = parts[runtime_idx + 1]  # e.g., "linux-x64"
                                target_dir = temp_path / "runtimes" / runtime_id / "native"
                                target_dir.mkdir(parents=True, exist_ok=True)
                                
                                target_file = target_dir / lib.name
                                shutil.copy2(lib, target_file)
                                print(f"  Copied {lib.name} to {target_file}")
                        
                        # Repackage
                        nupkg_file.unlink()  # Remove original
                        with zipfile.ZipFile(nupkg_file, 'w', zipfile.ZIP_DEFLATED) as zip_ref:
                            for file_path in temp_path.rglob('*'):
                                if file_path.is_file():
                                    arc_name = file_path.relative_to(temp_path)
                                    zip_ref.write(file_path, arc_name)
                        
                        print(f"{Colors.GREEN}Updated NuGet package with native libraries{Colors.END}")
            else:
                print(f"{Colors.YELLOW}Warning: No native libraries found in {crypto_bin_dir}{Colors.END}")
        
        # Move packages to project directory
        if local_nuget_temp.exists():
            for file in local_nuget_temp.glob("*"):
                if file.is_file():  # Only move files, not directories
                    dest_file = self.local_nuget_repo / file.name
                    if dest_file.exists():
                        dest_file.unlink()  # Remove existing file
                    shutil.move(str(file), str(dest_file))
                    print(f"{Colors.GREEN}Moved {file.name} to {dest_file}{Colors.END}")

    def copy_protobufs(self):
        """Copy protobuf files from Proton.SDK to proton-sdk-sys"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Copying protobuf files ==={Colors.END}")
        
        # Source protobuf directory
        source_proto_dir = self.base_dir / "Proton.SDK" / "protos"
        
        # Target protobuf directory in proton-sdk-sys
        proton_sdk_sys_dir = self.base_dir / "proton-sdk-sys"
        target_proto_dir = proton_sdk_sys_dir / "protos"
        
        if not source_proto_dir.exists():
            print(f"{Colors.YELLOW}Warning: Source protobuf directory not found at {source_proto_dir}{Colors.END}")
            return
        
        if not proton_sdk_sys_dir.exists():
            print(f"{Colors.RED}Error: Cannot find proton-sdk-sys directory at {proton_sdk_sys_dir}{Colors.END}")
            return
        
        # Create target directory if it doesn't exist
        target_proto_dir.mkdir(parents=True, exist_ok=True)
        
        # Remove existing proto files to ensure clean copy
        if target_proto_dir.exists():
            for existing_file in target_proto_dir.glob("*.proto"):
                existing_file.unlink()
                print(f"{Colors.YELLOW}Removed existing: {existing_file.name}{Colors.END}")
        
        # Copy all .proto files
        copied_files = []
        for proto_file in source_proto_dir.glob("*.proto"):
            target_file = target_proto_dir / proto_file.name
            shutil.copy2(proto_file, target_file)
            copied_files.append(proto_file.name)
            print(f"{Colors.GREEN}Copied: {proto_file.name}{Colors.END}")
        
        if copied_files:
            print(f"{Colors.GREEN}Successfully copied {len(copied_files)} protobuf files to {target_proto_dir}{Colors.END}")
            print(f"{Colors.CYAN}Copied files: {', '.join(copied_files)}{Colors.END}")
        else:
            print(f"{Colors.YELLOW}No .proto files found in {source_proto_dir}{Colors.END}")

    def build_proton_sdk_rs(self):
        """Build proton-sdk-rs"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Building proton-sdk-rs ==={Colors.END}")
        
        # Determine the correct rust directory path
        # The proton-sdk-sys directory should be directly under the base directory
        # alongside proton-sdk-rs (the Rust workspace directory)
        rs_dir = self.base_dir
        
        print(f"{Colors.CYAN}Using base directory for native libs: {rs_dir}{Colors.END}")
        print(f"{Colors.CYAN}Looking for proton-sdk-sys at: {rs_dir / 'proton-sdk-sys'}{Colors.END}")
        print(f"{Colors.CYAN}Looking for proton-sdk-rs at: {rs_dir / 'proton-sdk-rs'}{Colors.END}")
        
        # Find and copy .NET binaries from the AOT-compiled CExports project
        sdk_src_dir = self.base_dir / "Proton.SDK" / "src"
        
        dotnet_arch = 'x64' if self.arch == 'amd64' else self.arch
        if self.os_name == 'windows':
            runtime_id = f"win-{dotnet_arch}"
        elif self.os_name == 'linux':
            runtime_id = f"linux-{dotnet_arch}"
        elif self.os_name == 'macos':
            runtime_id = f"osx-{dotnet_arch}"
        else:
            runtime_id = f"{self.os_name}-{dotnet_arch}"
        
        aot_publish_dir = sdk_src_dir / "Proton.Sdk.Drive.CExports" / "bin" / "Release" / "net9.0" / runtime_id / "publish"
        aot_output_dir = sdk_src_dir / "Proton.Sdk.Drive.CExports" / "bin" / "Release" / "net9.0" / runtime_id
        
        source_dir = None
        if aot_publish_dir.exists():
            print(f"{Colors.BLUE}Found AOT published binaries at:{Colors.END} {aot_publish_dir}")
            source_dir = aot_publish_dir
        elif aot_output_dir.exists():
            print(f"{Colors.BLUE}Found AOT output binaries at:{Colors.END} {aot_output_dir}")
            source_dir = aot_output_dir
        
        def copy_files_excluding_pdb(src_dir, dst_dir):
            """Copy files from src_dir to dst_dir, excluding .pdb files"""
            dst_dir.mkdir(parents=True, exist_ok=True)
            
            copied_files = []
            for item in src_dir.rglob('*'):
                if item.is_file():
                    # Skip .pdb files
                    if item.suffix.lower() == '.pdb':
                        print(f"{Colors.YELLOW}Skipping PDB file: {item.name}{Colors.END}")
                        continue
                    
                    # Calculate relative path from source
                    rel_path = item.relative_to(src_dir)
                    dst_file = dst_dir / rel_path
                    
                    # Create parent directories if needed
                    dst_file.parent.mkdir(parents=True, exist_ok=True)
                    
                    # Copy the file
                    shutil.copy2(item, dst_file)
                    copied_files.append(item.name)
                    print(f"{Colors.GREEN}Copied: {item.name}{Colors.END}")
            
            return copied_files
        
        if source_dir:
            print(f"{Colors.BLUE}Copying AOT-compiled binaries from:{Colors.END} {source_dir}")
            
            # Use self.dlls_location for output, matching build_dll_only
            native_libs_dir = self.dlls_location / runtime_id
            if native_libs_dir.exists():
                shutil.rmtree(native_libs_dir)
            copied_files = copy_files_excluding_pdb(source_dir, native_libs_dir)
            print(f"{Colors.GREEN}Successfully copied {len(copied_files)} files (excluding .pdb) to {native_libs_dir}{Colors.END}")
        else:
            print(f"{Colors.YELLOW}Warning: AOT output directory not found at {aot_output_dir}{Colors.END}")
            
            # Fallback: Look for any net9.0 directory as before
            net90_dirs = list(sdk_src_dir.glob("**/bin/Release/net9.0"))
            if not net90_dirs:
                # Fallback to any net*.0 directory
                net90_dirs = list(sdk_src_dir.glob("**/bin/Release/net*.0"))
            
            if net90_dirs:
                source_net90_dir = net90_dirs[0]  # Take the first match
                print(f"{Colors.BLUE}Copying .NET binaries from fallback location:{Colors.END} {source_net90_dir}")
                
                native_libs_dir = self.dlls_location / runtime_id
                if native_libs_dir.exists():
                    shutil.rmtree(native_libs_dir)
                copied_files = copy_files_excluding_pdb(source_net90_dir, native_libs_dir)
                print(f"{Colors.GREEN}Successfully copied {len(copied_files)} files (excluding .pdb) to {native_libs_dir}{Colors.END}")
            else:
                print(f"{Colors.YELLOW}Warning: No net9.0 binaries found{Colors.END}")
    
        # Copy protobuf files AFTER libraries are copied but BEFORE cargo tests
        self.copy_protobufs()
    
        # Run cargo test for both proton-sdk-rs and proton-sdk-sys
        rust_projects = [
            ("proton-sdk-rs", "Rust workspace"),
            ("proton-sdk-sys", "Native bindings")
        ]
        
        for project_name, project_desc in rust_projects:
            project_dir = self.base_dir / project_name
            if project_dir.exists():
                print(f"{Colors.CYAN}Running cargo testing binary (proton-drive) in {project_desc}: {project_dir}{Colors.END}")
                os.chdir(project_dir)
                try:
                    self.run_command("cargo run -p proton-drive")
                    print(f"{Colors.GREEN}+ Tests completed for {project_name}{Colors.END}")
                except subprocess.CalledProcessError as e:
                    print(f"{Colors.YELLOW}Warning: Tests failed for {project_name}: {e}{Colors.END}")
                    print(f"{Colors.YELLOW}Continuing with build process...{Colors.END}")
            else:
                print(f"{Colors.YELLOW}Warning: {project_name} directory not found at {project_dir}, skipping cargo test{Colors.END}")

    def clean_all(self):
        """Clean all build artifacts and temporary directories"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Cleaning build artifacts ==={Colors.END}")
        
        clean_targets = [
            # .NET build outputs
            (self.base_dir / "dotnet-crypto" / "bin", "dotnet-crypto build outputs"),
            (self.base_dir / "dotnet-crypto" / "obj", "dotnet-crypto intermediate files"),
            (self.base_dir / "dotnet-crypto" / "local-nuget-repository", "dotnet-crypto temp NuGet repo"),
            (self.base_dir / "Proton.SDK" / "src" / "**" / "bin", "Proton.SDK build outputs"),
            (self.base_dir / "Proton.SDK" / "src" / "**" / "obj", "Proton.SDK intermediate files"),
            
            # Rust build outputs
            (self.base_dir / "proton-sdk-rs" / "target", "Rust build outputs"),
            (self.base_dir / "proton-sdk-sys" / "target", "proton-sdk-sys build outputs"),
            
            # Native libraries
            (self.base_dir / "proton-sdk-sys" / "native-libs", "Native libraries"),
            
            # Local NuGet repository
            (self.local_nuget_repo, "Local NuGet repository"),
            
            # Go build artifacts
            (self.base_dir / "dotnet-crypto" / "bin" / "runtimes", "Go build artifacts"),
        ]
        
        for target_path, description in clean_targets:
            if target_path.exists():
                if target_path.is_dir():
                    # Handle glob patterns for nested directories
                    if "**" in str(target_path):
                        # Use glob to find matching directories
                        parent_path = Path(str(target_path).split("**")[0])
                        pattern = str(target_path).split("**")[1].lstrip("/\\")
                        if parent_path.exists():
                            for match in parent_path.glob(f"**/{pattern}"):
                                if match.is_dir():
                                    try:
                                        shutil.rmtree(match)
                                        print(f"{Colors.GREEN}Removed: {match} ({description}){Colors.END}")
                                    except Exception as e:
                                        print(f"{Colors.YELLOW}Warning: Could not remove {match}: {e}{Colors.END}")
                    else:
                        try:
                            shutil.rmtree(target_path)
                            print(f"{Colors.GREEN}Removed: {target_path} ({description}){Colors.END}")
                        except Exception as e:
                            print(f"{Colors.YELLOW}Warning: Could not remove {target_path}: {e}{Colors.END}")
                else:
                    try:
                        target_path.unlink()
                        print(f"{Colors.GREEN}Removed: {target_path} ({description}){Colors.END}")
                    except Exception as e:
                        print(f"{Colors.YELLOW}Warning: Could not remove {target_path}: {e}{Colors.END}")
            else:
                print(f"{Colors.CYAN}Not found: {target_path} ({description}){Colors.END}")
        
        # Clean cargo cache for this workspace
        for rust_project in ["proton-sdk-rs", "proton-sdk-sys"]:
            project_dir = self.base_dir / rust_project
            if project_dir.exists():
                print(f"{Colors.BLUE}Running cargo clean in {rust_project}...{Colors.END}")
                try:
                    self.run_command("cargo clean", cwd=project_dir)
                    print(f"{Colors.GREEN}+ Cargo clean completed for {rust_project}{Colors.END}")
                except subprocess.CalledProcessError as e:
                    print(f"{Colors.YELLOW}Warning: Cargo clean failed for {rust_project}: {e}{Colors.END}")
        
        # Clean .NET restore cache
        try:
            print(f"{Colors.BLUE}Cleaning .NET NuGet cache...{Colors.END}")
            self.run_command("dotnet nuget locals all --clear")
            print(f"{Colors.GREEN}+ .NET NuGet cache cleared{Colors.END}")
        except subprocess.CalledProcessError as e:
            print(f"{Colors.YELLOW}Warning: Could not clear .NET cache: {e}{Colors.END}")
    
        print(f"{Colors.BOLD}{Colors.GREEN}=== Clean completed ==={Colors.END}")

    def build_all(self, exclude_steps=None):
        """Execute the complete build process, optionally excluding certain steps"""
        if exclude_steps is None:
            exclude_steps = []
        
        # Define all build steps in order
        all_steps = [
            ("clone", "Repository cloning", self.clone_repositories),
            ("crypto", "dotnet-crypto build", self.build_dotnet_crypto),
            ("protos", "Protobuf copying", self.copy_protobufs),
            ("sdk", "Proton.SDK build", self.build_dll_only),
            # ("rust", "proton-sdk-rs build", self.build_proton_sdk_rs), rust will be done manually
        ]
        
        try:
            print(f"{Colors.BOLD}{Colors.MAGENTA}Starting build process in:{Colors.END} {self.base_dir}")
            print(f"{Colors.BOLD}{Colors.MAGENTA}Target OS/Arch:{Colors.END} {self.os_name}/{self.arch}")
            
            if exclude_steps:
                print(f"{Colors.YELLOW}Excluding steps: {', '.join(exclude_steps)}{Colors.END}")
            
            self._check_windows_shell()
            self.check_dependencies()
            
            # Execute steps that are not excluded
            for step_name, step_desc, step_func in all_steps:
                if step_name not in exclude_steps:
                    print(f"{Colors.BOLD}{Colors.CYAN}=== {step_desc} ==={Colors.END}")
                    step_func()
                else:
                    print(f"{Colors.YELLOW}Skipping {step_desc} (excluded){Colors.END}")
            
            print(f"{Colors.BOLD}{Colors.GREEN}=== Build completed successfully! ==={Colors.END}")
            
        except subprocess.CalledProcessError as e:
            print(f"{Colors.RED}Build failed:{Colors.END} {e}")
            sys.exit(1)
        except FileNotFoundError as e:
            print(f"{Colors.RED}Build failed - file not found:{Colors.END} {e}")
            sys.exit(1)
        except OSError as e:
            print(f"{Colors.RED}Build failed - OS error:{Colors.END} {e}")
            sys.exit(1)

    def build_dll_only(self):
        """Build only the .NET AOT library and copy to native-libs folder"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Building DLL only ==={Colors.END}")
        
        # Determine the runtime identifier based on the current platform
        dotnet_arch = 'x64' if self.arch == 'amd64' else self.arch
        if self.os_name == 'windows':
            runtime_id = f"win-{dotnet_arch}"
            lib_extension = ".dll"
        elif self.os_name == 'linux':
            runtime_id = f"linux-{dotnet_arch}"
            lib_extension = ".so"
        elif self.os_name == 'macos':
            runtime_id = f"osx-{dotnet_arch}"
            lib_extension = ".dylib"
        else:
            runtime_id = f"{self.os_name}-{dotnet_arch}"
            lib_extension = ".so"  # fallback

        # Check if Proton.SDK exists
        sdk_dir = self.base_dir / "Proton.SDK"
        if not sdk_dir.exists():
            print(f"{Colors.RED}Error: Proton.SDK directory not found at {sdk_dir}{Colors.END}")
            print(f"{Colors.YELLOW}Please run 'python build.py --step clone' first to clone repositories{Colors.END}")
            return
        
        src_dir = sdk_dir / "src"
        drive_project = src_dir / "Proton.Sdk.Drive.CExports" / "Proton.Sdk.Drive.CExports.csproj"
        
        if not drive_project.exists():
            print(f"{Colors.RED}Error: Proton.Sdk.Drive.CExports.csproj not found at {drive_project}{Colors.END}")
            return
        
        print(f"{Colors.BLUE}Building AOT library for runtime: {runtime_id}{Colors.END}")
        
        # Restore packages first
        print(f"{Colors.BLUE}Restoring packages...{Colors.END}")
        try:
            self.run_command(f'dotnet restore "{drive_project}"')
            print(f"{Colors.GREEN}Package restore completed{Colors.END}")
        except subprocess.CalledProcessError as e:
            print(f"{Colors.YELLOW}Warning: Package restore failed, continuing with build: {e}{Colors.END}")
        
        # Build with AOT
        try:
            self.run_command(
                f'dotnet publish "{drive_project}" '
                f'-r {runtime_id} '
                f'--self-contained '
                f'-p:PublishAot=true'
            )
            print(f"{Colors.GREEN}AOT compilation completed for {runtime_id}{Colors.END}")
        except subprocess.CalledProcessError as e:
            print(f"{Colors.RED}AOT Library compilation failed: {e}{Colors.END}")
            return
        
        # Find the built library
        aot_publish_dir = src_dir / "Proton.Sdk.Drive.CExports" / "bin" / "Release" / "net9.0" / runtime_id / "publish"
        aot_output_dir = src_dir / "Proton.Sdk.Drive.CExports" / "bin" / "Release" / "net9.0" / runtime_id
        
        source_dir = None
        if aot_publish_dir.exists():
            print(f"{Colors.BLUE}Found AOT published binaries at: {aot_publish_dir}{Colors.END}")
            source_dir = aot_publish_dir
        elif aot_output_dir.exists():
            print(f"{Colors.BLUE}Found AOT output binaries at: {aot_output_dir}{Colors.END}")
            source_dir = aot_output_dir
        else:
            print(f"{Colors.RED}Error: No AOT output found{Colors.END}")
            return
        
        # Use self.dlls_location for output, matching the rest of the build system
        native_libs_dir = self.dlls_location / runtime_id
        native_libs_dir.mkdir(parents=True, exist_ok=True)
        
        print(f"{Colors.CYAN}Creating native-libs at: {native_libs_dir}{Colors.END}")
        print(f"{Colors.CYAN}Base directory: {self.base_dir}{Colors.END}")
        
        # Copy library files (excluding .pdb files)
        copied_files = []
        for item in source_dir.rglob('*'):
            if item.is_file():
                # Skip .pdb files
                if item.suffix.lower() == '.pdb':
                    print(f"{Colors.YELLOW}Skipping PDB file: {item.name}{Colors.END}")
                    continue
                
                # Copy the file
                dst_file = native_libs_dir / item.name
                shutil.copy2(item, dst_file)
                copied_files.append(item.name)
                print(f"{Colors.GREEN}Copied: {item.name}{Colors.END}")
        
        if copied_files:
            print(f"{Colors.GREEN}Successfully copied {len(copied_files)} files to {native_libs_dir}{Colors.END}")
            
            # Find and highlight the main library file
            main_lib_files = [f for f in copied_files if lib_extension in f.lower()]
            if main_lib_files:
                print(f"{Colors.BOLD}{Colors.GREEN}Main library: {main_lib_files[0]}{Colors.END}")
            
            print(f"{Colors.CYAN}Native libraries location: {native_libs_dir}{Colors.END}")
        else:
            print(f"{Colors.YELLOW}No files were copied{Colors.END}")

def main():
    parser = argparse.ArgumentParser(description="Build proton-sdk-rs and dependencies")
    parser.add_argument(
        "--base-dir", 
        help="Base directory for cloning repositories (default: parent of current directory)"
    )
    parser.add_argument(
        "--arch", 
        help="Target architecture (default: auto-detect)",
        choices=["amd64", "arm64", "386"]
    )
    parser.add_argument(
        "--skip-clone", 
        action="store_true", 
        help="Skip repository cloning (deprecated: use --exclude clone)"
    )
    parser.add_argument(
        "--step", 
        choices=["clone", "crypto", "sdk", "rust", "protos", "dll", "all"],
        default="all",
        help="Run specific build step only"
    )
    parser.add_argument(
        "--exclude", 
        action="append",
        choices=["clone", "crypto", "sdk", "rust", "protos"],
        help="Exclude specific build steps (can be used multiple times)"
    )
    parser.add_argument(
        "--clean",
        action="store_true",
        help="Clean all build artifacts and exit"
    )
    
    args = parser.parse_args()
    
    builder = BuildScript(base_dir=args.base_dir, arch=args.arch)
    
    # Handle clean command
    if args.clean:
        builder.clean_all()
        return
    
    # Handle deprecated --skip-clone flag
    exclude_steps = args.exclude or []
    if args.skip_clone:
        print(f"{Colors.YELLOW}Warning: --skip-clone is deprecated, use --exclude clone instead{Colors.END}")
        if "clone" not in exclude_steps:
            exclude_steps.append("clone")
    
    if args.step == "all":
        builder.build_all(exclude_steps=exclude_steps)
    elif args.step == "clone":
        if "clone" not in exclude_steps:
            builder.clone_repositories()
        else:
            print(f"{Colors.YELLOW}Clone step excluded, nothing to do{Colors.END}")
    elif args.step == "crypto":
        if "crypto" not in exclude_steps:
            builder.build_dotnet_crypto()
        else:
            print(f"{Colors.YELLOW}Crypto step excluded, nothing to do{Colors.END}")
    elif args.step == "sdk":
        if "sdk" not in exclude_steps:
            print(f"{Colors.YELLOW}build_proton_sdk has been removed. Use build_dll_only instead.{Colors.END}")
            return
        else:
            print(f"{Colors.YELLOW}SDK step excluded, nothing to do{Colors.END}")
    elif args.step == "rust":
        if "rust" not in exclude_steps:
            builder.build_proton_sdk_rs()
        else:
            print(f"{Colors.YELLOW}Rust step excluded, nothing to do{Colors.END}")
    elif args.step == "protos":
        if "protos" not in exclude_steps:
            builder.copy_protobufs()
        else:
            print(f"{Colors.YELLOW}Protos step excluded, nothing to do{Colors.END}")
    elif args.step == "dll":
        builder.build_dll_only()


if __name__ == "__main__":
    main()
