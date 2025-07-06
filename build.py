#!/usr/bin/env python3
"""
Build script for proton-sdk-rs and its dependencies.
This script automates the process of cloning repositories, building dotnet-crypto,
building Proton.SDK, and finally building proton-sdk-rs.
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
        self.local_nuget_repo = Path.home() / "local-nuget-repository"
        self.required_tools = ['git', 'dotnet', 'cargo', 'rustc', 'go', 'gcc']
        self.optional_tools = []  # No optional tools required with integrated build system
        
        print(f"{Colors.CYAN}Base directory set to: {self.base_dir}{Colors.END}")
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
        
        # Windows-specific configuration
        if go_os == "windows":
            # Check for different compiler options in order of preference
            compiler_configs = [
                # Option 1: Try MSVC directly (if vcvars is set up)
                {
                    "name": "MSVC",
                    "cc": "cl",
                    "cxx": "cl",
                    "check_cmd": ["cl"],
                    "cgo_cflags": "",
                    "cgo_ldflags": "-s -w"
                },
                # Option 2: Try clang-cl (MSVC-compatible clang)
                {
                    "name": "clang-cl",
                    "cc": "clang-cl",
                    "cxx": "clang-cl",
                    "check_cmd": ["clang-cl", "--version"],
                    "cgo_cflags": "",
                    "cgo_ldflags": "-s -w"
                },
                # Option 3: MinGW GCC (most reliable fallback)
                {
                    "name": "MinGW GCC",
                    "cc": "gcc",
                    "cxx": "g++",
                    "check_cmd": ["gcc", "--version"],
                    "cgo_cflags": "-O2",
                    "cgo_ldflags": "-s -w -static -static-libgcc -static-libstdc++"
                }
            ]
            
            selected_compiler = None
            for config in compiler_configs:
                try:
                    result = subprocess.run(
                        config["check_cmd"], 
                        capture_output=True, 
                        check=True, 
                        timeout=10
                    )
                    selected_compiler = config
                    print(f"{Colors.GREEN}Using {config['name']} for Windows compilation{Colors.END}")
                    break
                except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                    print(f"{Colors.YELLOW}{config['name']} not available{Colors.END}")
                    continue
            
            if not selected_compiler:
                print(f"{Colors.RED}No suitable C compiler found for Windows{Colors.END}")
                print(f"{Colors.YELLOW}Please install one of: Visual Studio Build Tools, LLVM/Clang, or MinGW-w64{Colors.END}")
                return
            
            # Apply the selected compiler configuration - ensure all values are strings
            go_env.update({
                "CC": str(selected_compiler["cc"]),
                "CXX": str(selected_compiler["cxx"]),
                "CGO_CFLAGS": str(selected_compiler["cgo_cflags"]),
                "CGO_LDFLAGS": str(selected_compiler["cgo_ldflags"])
            })
            
            # For MSVC/clang-cl, ensure proper environment is set up
            if selected_compiler["name"] in ["MSVC", "clang-cl"]:
                # Try to detect Visual Studio installation and set up environment
                try:
                    # Try to find vcvars64.bat and run it to set up MSVC environment
                    import winreg
                    
                    # Look for Visual Studio installation
                    vs_keys = [
                        r"SOFTWARE\Microsoft\VisualStudio\SxS\VS7",
                        r"SOFTWARE\WOW6432Node\Microsoft\VisualStudio\SxS\VS7"
                    ]
                    
                    vs_path = None
                    for key_path in vs_keys:
                        try:
                            with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, key_path) as key:
                                # Try different Visual Studio versions
                                for version in ["17.0", "16.0", "15.0"]:  # 2022, 2019, 2017
                                    try:
                                        vs_path, _ = winreg.QueryValueEx(key, version)
                                        if Path(vs_path).exists():
                                            break
                                    except FileNotFoundError:
                                        continue
                            if vs_path:
                                break
                        except FileNotFoundError:
                            continue
                    
                    if vs_path:
                        vcvars_path = Path(vs_path) / "VC" / "Auxiliary" / "Build" / "vcvars64.bat"
                        if vcvars_path.exists():
                            print(f"{Colors.BLUE}Found Visual Studio at: {vs_path}{Colors.END}")
                            
                            # Run vcvars64.bat to get environment variables
                            cmd = f'"{vcvars_path}" && set'
                            result = subprocess.run(
                                cmd, 
                                shell=True, 
                                capture_output=True, 
                                text=True, 
                                timeout=30
                            )
                            
                            if result.returncode == 0:
                                # Parse environment variables from vcvars output
                                for line in result.stdout.splitlines():
                                    if '=' in line:
                                        key, value = line.split('=', 1)
                                        if key.upper() in ['INCLUDE', 'LIB', 'LIBPATH', 'PATH', 'WINDOWSSDKDIR', 'WINDOWSSDKVERSION']:
                                            go_env[key] = value
                                print(f"{Colors.GREEN}MSVC environment configured{Colors.END}")
                            else:
                                print(f"{Colors.YELLOW}Could not configure MSVC environment, trying without{Colors.END}")
                        else:
                            print(f"{Colors.YELLOW}vcvars64.bat not found, trying without MSVC setup{Colors.END}")
                    else:
                        print(f"{Colors.YELLOW}Visual Studio not found in registry, trying without MSVC setup{Colors.END}")
                        
                except Exception as e:
                    print(f"{Colors.YELLOW}Could not set up MSVC environment: {e}{Colors.END}")
                    print(f"{Colors.YELLOW}Continuing without MSVC environment setup{Colors.END}")
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
                    # Generate both .a and .lib for Windows compatibility
                    output_file_name = f"{lib_name}.a"  # Go generates .a file
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
                
                # For Windows c-archive mode, create MSVC-compatible .lib file
                if go_os == "windows" and build_mode == "c-archive" and output_file_path.exists():
                    # Try to convert .a to .lib using lib.exe (MSVC library manager)
                    lib_file_path = output_file_path.with_suffix(".lib")
                    
                    # First try using lib.exe if available (part of MSVC)
                    try:
                        lib_cmd = [
                            "lib.exe",
                            f"/OUT:{lib_file_path}",
                            str(output_file_path)
                        ]
                        subprocess.run(lib_cmd, check=True, capture_output=True, text=True)
                        print(f"{Colors.GREEN}+ Created MSVC-compatible .lib file using lib.exe: {lib_file_path.name}{Colors.END}")
                    except (subprocess.CalledProcessError, FileNotFoundError):
                        # Fallback: Just copy and rename the .a file to .lib
                        # This works in many cases as the formats are similar
                        try:
                            shutil.copy2(output_file_path, lib_file_path)
                            print(f"{Colors.YELLOW}+ Created .lib file by copying .a file: {lib_file_path.name}{Colors.END}")
                            print(f"{Colors.YELLOW}  Note: This may not be fully MSVC-compatible{Colors.END}")
                        except Exception as e:
                            print(f"{Colors.YELLOW}Warning: Could not create .lib file: {e}{Colors.END}")
                
            except subprocess.CalledProcessError as e:
                print(f"{Colors.RED}Go build failed for {build_mode}:{Colors.END} {e}")
                if e.stdout:
                    print(f"{Colors.YELLOW}Stdout:{Colors.END} {e.stdout}")
                if e.stderr:
                    print(f"{Colors.RED}Stderr:{Colors.END} {e.stderr}")
                
                # For Windows, if clang failed, try fallback to MinGW
                if go_os == "windows" and "clang" in go_env.get("CC", ""):
                    print(f"{Colors.YELLOW}Clang build failed, trying MinGW GCC fallback...{Colors.END}")
                    
                    # Update environment for MinGW
                    go_env.update({
                        "CC": "gcc",
                        "CXX": "g++", 
                        "CGO_CFLAGS": "-O2",
                        "CGO_LDFLAGS": "-s -w -static -static-libgcc -static-libstdc++"
                    })
                    
                    try:
                        subprocess.run(
                            cmd,
                            env=go_env,
                            cwd=crypto_dir,
                            check=True,
                            capture_output=True,
                            text=True
                        )
                        print(f"{Colors.GREEN}+ MinGW fallback successful for {output_file_name}{Colors.END}")
                        
                        # Create .lib file for MinGW output too
                        if build_mode == "c-archive" and output_file_path.exists():
                            lib_file_path = output_file_path.with_suffix(".lib")
                            shutil.copy2(output_file_path, lib_file_path)
                            print(f"{Colors.GREEN}+ Created .lib file from MinGW output: {lib_file_path.name}{Colors.END}")
                            
                    except subprocess.CalledProcessError as fallback_e:
                        print(f"{Colors.RED}MinGW fallback also failed:{Colors.END} {fallback_e}")
                        if fallback_e.stderr:
                            print(f"{Colors.RED}Stderr:{Colors.END} {fallback_e.stderr}")
                        # Don't exit, try other build modes
                        continue
                else:
                    # Don't exit, try other build modes
                    continue
    
    def build_dotnet_crypto(self):
        """Build dotnet-crypto package"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Building dotnet-crypto ==={Colors.END}")
        
        crypto_dir = self.base_dir / "dotnet-crypto"
        os.chdir(crypto_dir)
        
        # Build Go cryptography library using integrated logic
        self.build_go_crypto(crypto_dir)
        
        # Create local nuget repository
        local_nuget_temp = crypto_dir / "local-nuget-repository"
        
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
        
        # Ensure local nuget repository exists
        self.local_nuget_repo.mkdir(parents=True, exist_ok=True)
        
        # Move packages to home directory
        if local_nuget_temp.exists():
            for file in local_nuget_temp.glob("*"):
                shutil.move(str(file), str(self.local_nuget_repo / file.name))
        
        # Add nuget source
        try:
            self.run_command(
                f'dotnet nuget add source "{self.local_nuget_repo}" --name ProtonRepository'
            )
        except subprocess.CalledProcessError as e:
            print(f"{Colors.YELLOW}NuGet source 'ProtonRepository' already exists or error exists (check logs), skipping...{Colors.END}")
            print(f"{Colors.UNDERLINE}Exception Caught:{Colors.END} {e}")
    
    def build_proton_sdk(self):
        """Build Proton.SDK"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Building Proton.SDK ==={Colors.END}")
        
        sdk_dir = self.base_dir / "Proton.SDK"
        src_dir = sdk_dir / "src"
        
        os.chdir(src_dir)
        
        # Configure NuGet sources to include both nuget.org and local repository
        print(f"{Colors.BLUE}Configuring NuGet sources...{Colors.END}")
        try:
            # Clear existing sources first and list them
            self.run_command('dotnet nuget list source', capture_output=True)
            
            # Add nuget.org if not already present
            try:
                self.run_command('dotnet nuget add source https://api.nuget.org/v3/index.json -n nuget.org')
            except subprocess.CalledProcessError:
                print(f"{Colors.YELLOW}nuget.org source may already exist{Colors.END}")
            
            # Add local repository
            try:
                self.run_command(f'dotnet nuget add source "{self.local_nuget_repo}" -n ProtonRepository')
            except subprocess.CalledProcessError:
                print(f"{Colors.YELLOW}ProtonRepository source may already exist{Colors.END}")
            
            print(f"{Colors.GREEN}NuGet sources configured{Colors.END}")
        except subprocess.CalledProcessError:
            print(f"{Colors.YELLOW}Warning: Could not configure NuGet sources, continuing...{Colors.END}")
        
        # Add Proton.Cryptography package to each project folder
        for folder in src_dir.iterdir():
            if folder.is_dir() and (folder / f"{folder.name}.csproj").exists():
                print(f"{Colors.BLUE}Adding package to {folder.name}{Colors.END}")
                os.chdir(folder)
                try:
                    # First restore packages to ensure dependencies are available
                    self.run_command('dotnet restore')
                    
                    # Then add the package
                    self.run_command(
                        f'dotnet add package Proton.Cryptography -s "{self.local_nuget_repo}"'
                    )
                except subprocess.CalledProcessError:
                    print(f"{Colors.YELLOW}Warning: Failed to add package to {folder.name}, continuing...{Colors.END}")
                os.chdir(src_dir)
        
        # Publish Proton.Sdk.Drive
        drive_project = src_dir / "Proton.Sdk.Drive.CExports" / "Proton.Sdk.Drive.CExports.csproj"
        if drive_project.exists():
            # Determine the runtime identifier based on the current platform
            # Use .NET runtime identifier format (x64 instead of amd64)
            dotnet_arch = 'x64' if self.arch == 'amd64' else self.arch
            if self.os_name == 'windows':
                runtime_id = f"win-{dotnet_arch}"
                lib_suffix = ".dll"
            elif self.os_name == 'linux':
                runtime_id = f"linux-{dotnet_arch}"
                lib_suffix = ".so"
            elif self.os_name == 'macos':
                runtime_id = f"osx-{dotnet_arch}"
                lib_suffix = ".dylib"
            else:
                runtime_id = f"{self.os_name}-{dotnet_arch}"
                lib_suffix = ".so"  # fallback
            
            print(f"{Colors.BLUE}Publishing with AOT for runtime: {runtime_id}{Colors.END}")
            
            # Restore packages first to ensure all dependencies are available
            print(f"{Colors.BLUE}Restoring packages for {drive_project.name}...{Colors.END}")
            try:
                self.run_command(f'dotnet restore "{drive_project}"')
                print(f"{Colors.GREEN}Package restore completed{Colors.END}")
            except subprocess.CalledProcessError as e:
                print(f"{Colors.YELLOW}Warning: Package restore failed, continuing with build: {e}{Colors.END}")
            
            try:
                # Try without AOT first
                self.run_command(
                    f'dotnet publish "{drive_project}" '
                    f'-r {runtime_id} '
                    f'--self-contained '
                    f'-p:PublishAot=true'
                )
                print(f"{Colors.GREEN}Non-AOT compilation completed for {runtime_id}{Colors.END}")
            except subprocess.CalledProcessError:
                print(f"{Colors.YELLOW}Proton SDK AOT Library compilation failed :({Colors.END}")
                print("Gracefully exiting now")
                sys.exit()
        else:
            print(f"{Colors.YELLOW}Warning: Proton.Sdk.Drive.CExports.csproj not found{Colors.END}")
    
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
        
        # Determine the runtime identifier based on the current platform
        dotnet_arch = 'x64' if self.arch == 'amd64' else self.arch
        if self.os_name == 'windows':
            runtime_id = f"win-{dotnet_arch}"
        elif self.os_name == 'linux':
            runtime_id = f"linux-{dotnet_arch}"
        elif self.os_name == 'macos':
            runtime_id = f"osx-{dotnet_arch}"
        else:
            runtime_id = f"{self.os_name}-{dotnet_arch}"
        
        # Look for the specific AOT-compiled output directory
        # First try the publish directory (AOT output)
        aot_publish_dir = sdk_src_dir / "Proton.Sdk.Drive.CExports" / "bin" / "Release" / "net9.0" / runtime_id / "publish"
        aot_output_dir = sdk_src_dir / "Proton.Sdk.Drive.CExports" / "bin" / "Release" / "net9.0" / runtime_id
        
        source_dir = None
        if aot_publish_dir.exists():
            print(f"{Colors.BLUE}Found AOT published binaries at:{Colors.END} {aot_publish_dir}")
            source_dir = aot_publish_dir
        elif aot_output_dir.exists():
            print(f"{Colors.BLUE}Found AOT output binaries at:{Colors.END} {aot_output_dir}")
            source_dir = aot_output_dir
        
        if source_dir:
            print(f"{Colors.BLUE}Copying AOT-compiled binaries from:{Colors.END} {source_dir}")
            
            # Create native-libs directory if it doesn't exist
            # proton-sdk-sys should be directly under base directory, not under proton-sdk-rs
            proton_sdk_sys_dir = self.base_dir / "proton-sdk-sys"
            if not proton_sdk_sys_dir.exists():
                print(f"{Colors.RED}Error: Cannot find proton-sdk-sys directory at {proton_sdk_sys_dir}{Colors.END}")
                print(f"{Colors.YELLOW}Expected structure: {self.base_dir} should contain both 'proton-sdk-rs' and 'proton-sdk-sys' directories{Colors.END}")
                return
            
            native_libs_dir = proton_sdk_sys_dir / "native-libs"
            native_libs_dir.mkdir(parents=True, exist_ok=True)
            
            print(f"{Colors.CYAN}Creating native-libs at: {native_libs_dir}{Colors.END}")
            
            # Copy the runtime folder into native-libs with publish subdirectory
            runtime_target_dir = native_libs_dir / runtime_id / "publish"
            runtime_target_dir.mkdir(parents=True, exist_ok=True)
            
            # Remove existing contents and copy new ones
            if runtime_target_dir.exists():
                shutil.rmtree(runtime_target_dir)
            shutil.copytree(source_dir, runtime_target_dir)
            print(f"{Colors.GREEN}Successfully copied {runtime_id} AOT binaries to {runtime_target_dir}{Colors.END}")
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
                
                # Create native-libs directory if it doesn't exist
                # proton-sdk-sys should be directly under base directory, not under proton-sdk-rs
                proton_sdk_sys_dir = self.base_dir / "proton-sdk-sys"
                if not proton_sdk_sys_dir.exists():
                    print(f"{Colors.RED}Error: Cannot find proton-sdk-sys directory at {proton_sdk_sys_dir}{Colors.END}")
                    print(f"{Colors.YELLOW}Expected structure: {self.base_dir} should contain both 'proton-sdk-rs' and 'proton-sdk-sys' directories{Colors.END}")
                    return
                
                native_libs_dir = proton_sdk_sys_dir / "native-libs"
                native_libs_dir.mkdir(parents=True, exist_ok=True)
                
                print(f"{Colors.CYAN}Creating native-libs at: {native_libs_dir}{Colors.END}")
                
                # Copy as a runtime-specific subdirectory with publish structure
                runtime_target_dir = native_libs_dir / runtime_id / "publish"
                if runtime_target_dir.exists():
                    shutil.rmtree(runtime_target_dir)  # Remove only this specific runtime folder
                shutil.copytree(source_net90_dir, runtime_target_dir)
                print(f"{Colors.GREEN}Successfully copied net9.0 directory to {runtime_target_dir}{Colors.END}")
            else:
                print(f"{Colors.YELLOW}Warning: No net9.0 binaries found{Colors.END}")
        
        # Run cargo test for both proton-sdk-rs and proton-sdk-sys
        rust_projects = [
            ("proton-sdk-rs", "Rust workspace"),
            ("proton-sdk-sys", "Native bindings")
        ]
        
        for project_name, project_desc in rust_projects:
            project_dir = self.base_dir / project_name
            if project_dir.exists():
                print(f"{Colors.CYAN}Running cargo test in {project_desc}: {project_dir}{Colors.END}")
                os.chdir(project_dir)
                try:
                    self.run_command("cargo test")
                    print(f"{Colors.GREEN}+ Tests completed for {project_name}{Colors.END}")
                except subprocess.CalledProcessError as e:
                    print(f"{Colors.YELLOW}Warning: Tests failed for {project_name}: {e}{Colors.END}")
                    print(f"{Colors.YELLOW}Continuing with build process...{Colors.END}")
            else:
                print(f"{Colors.YELLOW}Warning: {project_name} directory not found at {project_dir}, skipping cargo test{Colors.END}")
    
    def build_all(self):
        """Execute the complete build process"""
        try:
            print(f"{Colors.BOLD}{Colors.MAGENTA}Starting build process in:{Colors.END} {self.base_dir}")
            print(f"{Colors.BOLD}{Colors.MAGENTA}Target OS/Arch:{Colors.END} {self.os_name}/{self.arch}")
            
            self._check_windows_shell()
            self.check_dependencies()
            self.clone_repositories()
            self.build_dotnet_crypto()
            self.build_proton_sdk()
            self.build_proton_sdk_rs()
            
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
        help="Skip repository cloning"
    )
    parser.add_argument(
        "--step", 
        choices=["clone", "crypto", "sdk", "rust", "all"],
        default="all",
        help="Run specific build step only"
    )
    
    args = parser.parse_args()
    
    builder = BuildScript(base_dir=args.base_dir, arch=args.arch)
    
    if args.step == "all":
        builder.build_all()
    elif args.step == "clone":
        builder.clone_repositories()
    elif args.step == "crypto":
        builder.build_dotnet_crypto()
    elif args.step == "sdk":
        builder.build_proton_sdk()
    elif args.step == "rust":
        builder.build_proton_sdk_rs()


if __name__ == "__main__":
    main()
