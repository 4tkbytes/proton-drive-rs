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
        # If we're in proton-sdk-rs/proton-sdk-rs, go up two levels to the parent
        # If we're in proton-sdk-rs, go up one level to the parent
        if base_dir:
            self.base_dir = Path(base_dir)
        else:
            current_dir = Path.cwd()
            if current_dir.name == "proton-sdk-rs" and current_dir.parent.name == "proton-sdk-rs":
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
        self.optional_tools = ['dos2unix']  # Tools that are helpful but not required
        
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
        """Check if running in Git Bash on Windows"""
        if self.os_name != 'windows':
            return  # Not Windows, no check needed
        
        # Check for MSYSTEM environment variable (set by Git Bash/MSYS2)
        msystem = os.environ.get('MSYSTEM')
        if msystem:
            print(f"{Colors.GREEN}✓{Colors.END} Running in Git Bash/MSYS2 (MSYSTEM={msystem})")
            return
        
        # If we get here, we're not in Git Bash
        print(f"{Colors.RED}✗ Error: This script must be run in Git Bash on Windows{Colors.END}")
        print(f"{Colors.YELLOW}Please:{Colors.END}")
        print(f"  1. Install {Colors.CYAN}Git for Windows{Colors.END} from: https://git-scm.com/download/win")
        print(f"  2. Right-click in your project folder and select {Colors.CYAN}'Git Bash Here'{Colors.END}")
        print("  3. Run this script from the Git Bash terminal")
        print(f"\n{Colors.CYAN}Note:{Colors.END} If you're using WSL, run this script from within WSL instead.")
        sys.exit(1)

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
                    print(f"{Colors.GREEN}✓{Colors.END} {tool} is available")
                else:
                    missing_tools.append(tool)
            except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
                missing_tools.append(tool)
        
        # Special check for bash on Windows (needed for build-go.sh)
        if self.os_name == 'windows':
            try:
                result = subprocess.run(
                    ['bash', '--version'], 
                    capture_output=True, 
                    text=True, 
                    timeout=10,
                    check=False
                )
                if result.returncode == 0:
                    print(f"{Colors.GREEN}✓{Colors.END} bash is available")
                else:
                    print(f"{Colors.YELLOW}⚠{Colors.END} bash not found - Go build script may fail on Windows")
            except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
                print(f"{Colors.YELLOW}⚠{Colors.END} bash not found - Go build script may fail on Windows")
        
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
                    print(f"{Colors.GREEN}✓{Colors.END} {tool} is available")
                else:
                    print(f"{Colors.YELLOW}⚠{Colors.END} {tool} not found - may need manual line ending conversion")
            except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
                print(f"{Colors.YELLOW}⚠{Colors.END} {tool} not found - may need manual line ending conversion")
        
        if missing_tools:
            print(f"{Colors.RED}✗ Missing required tools:{Colors.END} {', '.join(missing_tools)}")
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
        
        print(f"{Colors.GREEN}✓ All dependencies are available{Colors.END}")

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
    
    def build_dotnet_crypto(self):
        """Build dotnet-crypto package"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Building dotnet-crypto ==={Colors.END}")
        
        crypto_dir = self.base_dir / "dotnet-crypto"
        os.chdir(crypto_dir)
        
        # Run build script
        build_script_path = crypto_dir / "build" / "build-go.sh"
        if build_script_path.exists():
            try:
                # Fix line endings for Unix systems (convert CRLF to LF)
                if self.os_name != 'windows':
                    print(f"{Colors.YELLOW}Converting line endings for Unix compatibility...{Colors.END}")
                    try:
                        self.run_command(f"dos2unix {build_script_path}", capture_output=True)
                    except subprocess.CalledProcessError:
                        # Fallback: manual line ending conversion
                        print(f"{Colors.YELLOW}dos2unix failed, trying manual conversion...{Colors.END}")
                        with open(build_script_path, 'rb') as f:
                            content = f.read()
                        content = content.replace(b'\r\n', b'\n')
                        with open(build_script_path, 'wb') as f:
                            f.write(content)
                
                # Make script executable
                self.run_command(f"chmod +x {build_script_path}", capture_output=True)
                
                # Run the build script with bash explicitly
                self.run_command(f"bash ./build/build-go.sh {self.os_name}/{self.arch}")
            except subprocess.CalledProcessError as e:
                print(f"{Colors.RED}Go build script failed:{Colors.END} {e}")
                sys.exit(1)
                # print(f"{Colors.YELLOW}Continuing without Go build - this may affect cryptography functionality{Colors.END}")
        else:
            print(f"{Colors.YELLOW}Build script build/build-go.sh not found, skipping go build{Colors.END}")
        
        # Create local nuget repository
        local_nuget_temp = crypto_dir / "local-nuget-repository"
        
        # Pack the project
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
        
        # Add Proton.Cryptography package to each project folder
        for folder in src_dir.iterdir():
            if folder.is_dir() and (folder / f"{folder.name}.csproj").exists():
                print(f"{Colors.BLUE}Adding package to {folder.name}{Colors.END}")
                os.chdir(folder)
                try:
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
            
            # Special handling for Windows - skip AOT if crypto library is incompatible
            if self.os_name == 'windows':
                print(f"{Colors.YELLOW}Warning: Windows AOT build may fail due to toolchain incompatibility{Colors.END}")
                print(f"{Colors.YELLOW}Attempting build without AOT first...{Colors.END}")
                
                try:
                    # Try without AOT first
                    self.run_command(
                        f'dotnet publish "{drive_project}" '
                        f'-r {runtime_id} '
                        f'--self-contained '
                        f'-p:PublishAot=false'
                    )
                    print(f"{Colors.GREEN}Non-AOT compilation completed for {runtime_id}{Colors.END}")
                except subprocess.CalledProcessError:
                    print(f"{Colors.YELLOW}Non-AOT build also failed, trying regular build...{Colors.END}")
                    try:
                        # Fallback to regular build
                        self.run_command(
                            f'dotnet build "{drive_project}" -c Release'
                        )
                        print(f"{Colors.GREEN}Regular build completed for {runtime_id}{Colors.END}")
                    except subprocess.CalledProcessError as e:
                        print(f"{Colors.RED}All Windows build attempts failed:{Colors.END} {e}")
                        print(f"{Colors.YELLOW}Continuing with build - you may need to build manually{Colors.END}")
            else:
                # For non-Windows, try AOT compilation
                try:
                    self.run_command(
                        f'dotnet publish "{drive_project}" '
                        f'-r {runtime_id} '
                        f'--self-contained '
                        f'-p:PublishAot=true '
                        f'-p:EnableCompressionInSingleFile=false '
                        f'-p:OptimizationPreference=Speed'
                    )
                    print(f"{Colors.GREEN}AOT compilation completed for {runtime_id}{Colors.END}")
                except subprocess.CalledProcessError:
                    print(f"{Colors.YELLOW}AOT build failed, trying without AOT...{Colors.END}")
                    try:
                        # Fallback to non-AOT
                        self.run_command(
                            f'dotnet publish "{drive_project}" '
                            f'-r {runtime_id} '
                            f'--self-contained '
                            f'-p:PublishAot=false'
                        )
                        print(f"{Colors.GREEN}Non-AOT compilation completed for {runtime_id}{Colors.END}")
                    except subprocess.CalledProcessError as e:
                        print(f"{Colors.RED}Both AOT and non-AOT builds failed:{Colors.END} {e}")
                        raise
        else:
            print(f"{Colors.YELLOW}Warning: Proton.Sdk.Drive.CExports.csproj not found{Colors.END}")
    
    def build_proton_sdk_rs(self):
        """Build proton-sdk-rs"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Building proton-sdk-rs ==={Colors.END}")
        
        rs_dir = self.base_dir / "proton-sdk-rs"
        
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
        aot_output_dir = sdk_src_dir / "Proton.Sdk.Drive.CExports" / "bin" / "Release" / "net9.0" / runtime_id
        
        if aot_output_dir.exists():
            print(f"{Colors.BLUE}Copying AOT-compiled binaries from:{Colors.END} {aot_output_dir}")
            
            # Create native-libs directory if it doesn't exist
            native_libs_dir = rs_dir / "proton-sdk-sys" / "native-libs"
            native_libs_dir.mkdir(parents=True, exist_ok=True)
            
            # Copy the runtime folder into native-libs (don't replace, just add/update this runtime)
            runtime_target_dir = native_libs_dir / runtime_id
            if runtime_target_dir.exists():
                shutil.rmtree(runtime_target_dir)  # Remove only this specific runtime folder
            shutil.copytree(aot_output_dir, runtime_target_dir)
            print(f"{Colors.GREEN}Successfully copied {runtime_id} AOT binaries to proton-sdk-sys/native-libs/{runtime_id}{Colors.END}")
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
                native_libs_dir = rs_dir / "proton-sdk-sys" / "native-libs"
                native_libs_dir.mkdir(parents=True, exist_ok=True)
                
                # Copy as a runtime-specific subdirectory
                runtime_target_dir = native_libs_dir / runtime_id
                if runtime_target_dir.exists():
                    shutil.rmtree(runtime_target_dir)  # Remove only this specific runtime folder
                shutil.copytree(source_net90_dir, runtime_target_dir)
                print(f"{Colors.GREEN}Successfully copied net9.0 directory to proton-sdk-sys/native-libs/{runtime_id}{Colors.END}")
            else:
                print(f"{Colors.YELLOW}Warning: No net9.0 binaries found{Colors.END}")
        
        # Run cargo test
        os.chdir(rs_dir)
        self.run_command("cargo test")
    
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
