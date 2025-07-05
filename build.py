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
        self.required_tools = ['git', 'dotnet', 'cargo', 'rustc', 'go']
        self.optional_tools = ['dos2unix']  # Tools that are helpful but not required
        
    def _detect_arch(self):
        """Detect system architecture"""
        machine = platform.machine().lower()
        if machine in ['x86_64', 'amd64']:
            return 'x64'
        elif machine in ['aarch64', 'arm64']:
            return 'arm64'
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
    
    def run_command(self, cmd, cwd=None, shell=True):
        """Run a shell command and handle errors"""
        print(f"{Colors.BLUE}Running:{Colors.END} {cmd}")
        if cwd:
            print(f"  {Colors.CYAN}in directory:{Colors.END} {cwd}")
        
        try:
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
        except subprocess.CalledProcessError as e:
            print(f"{Colors.RED}Error running command:{Colors.END} {cmd}")
            print(f"{Colors.RED}Exit code:{Colors.END} {e.returncode}")
            if e.stdout:
                print(f"{Colors.YELLOW}Stdout:{Colors.END} {e.stdout}")
            if e.stderr:
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
                        self.run_command(f"dos2unix {build_script_path}")
                    except subprocess.CalledProcessError:
                        # Fallback: manual line ending conversion
                        print(f"{Colors.YELLOW}dos2unix failed, trying manual conversion...{Colors.END}")
                        with open(build_script_path, 'rb') as f:
                            content = f.read()
                        content = content.replace(b'\r\n', b'\n')
                        with open(build_script_path, 'wb') as f:
                            f.write(content)
                
                # Make script executable
                self.run_command(f"chmod +x {build_script_path}")
                
                # Run the build script
                self.run_command(f"bash build/build-go.sh {self.os_name}/{self.arch}")
            except subprocess.CalledProcessError as e:
                print(f"{Colors.RED}Go build script failed:{Colors.END} {e}")
                print(f"{Colors.YELLOW}Continuing without Go build - this may affect cryptography functionality{Colors.END}")
        else:
            print(f"{Colors.YELLOW}Build script build/build-go.sh not found, skipping go build{Colors.END}")
        
        # Create local nuget repository
        local_nuget_temp = crypto_dir / "local-nuget-repository"
        
        # Pack the project
        self.run_command(
            f'dotnet pack -c Release -p:Version=1.0.0 '
            f'src/dotnet/Proton.Cryptography.csproj --output {local_nuget_temp}'
        )
        
        # Ensure local nuget repository exists
        self.local_nuget_repo.mkdir(parents=True, exist_ok=True)
        
        # Move packages to home directory
        if local_nuget_temp.exists():
            for file in local_nuget_temp.glob("*"):
                shutil.move(str(file), str(self.local_nuget_repo / file.name))
        
        # Add nuget source
        self.run_command(
            f'dotnet nuget add source "{self.local_nuget_repo}" --name ProtonRepository'
        )
    
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
        drive_project = src_dir / "Proton.Sdk.Drive" / "Proton.Sdk.Drive.csproj"
        if drive_project.exists():
            self.run_command(f'dotnet publish "{drive_project}"')
        else:
            print(f"{Colors.YELLOW}Warning: Proton.Sdk.Drive.csproj not found{Colors.END}")
    
    def build_proton_sdk_rs(self):
        """Build proton-sdk-rs"""
        print(f"{Colors.BOLD}{Colors.CYAN}=== Building proton-sdk-rs ==={Colors.END}")
        
        rs_dir = self.base_dir / "proton-sdk-rs"
        native_libs_dir = rs_dir / "native-libs"
        
        # Create native-libs directory
        native_libs_dir.mkdir(parents=True, exist_ok=True)
        
        # Find and copy .NET binaries
        sdk_src_dir = self.base_dir / "Proton.SDK" / "src"
        
        # Look for published binaries
        publish_dirs = list(sdk_src_dir.glob("**/bin/Release/net*/publish"))
        if not publish_dirs:
            # Fallback to any Release binaries
            publish_dirs = list(sdk_src_dir.glob("**/bin/Release/net*"))
        
        if publish_dirs:
            source_dir = publish_dirs[0]  # Take the first match
            print(f"{Colors.BLUE}Copying binaries from:{Colors.END} {source_dir}")
            
            for file in source_dir.rglob("*"):
                if file.is_file():
                    relative_path = file.relative_to(source_dir)
                    dest_path = native_libs_dir / relative_path
                    dest_path.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(file, dest_path)
        else:
            print(f"{Colors.YELLOW}Warning: No published binaries found{Colors.END}")
        
        # Run cargo test
        os.chdir(rs_dir)
        self.run_command("cargo test")
    
    def build_all(self):
        """Execute the complete build process"""
        try:
            print(f"{Colors.BOLD}{Colors.MAGENTA}Starting build process in:{Colors.END} {self.base_dir}")
            print(f"{Colors.BOLD}{Colors.MAGENTA}Target OS/Arch:{Colors.END} {self.os_name}/{self.arch}")
            
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
        choices=["x64", "arm64", "x86"]
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
