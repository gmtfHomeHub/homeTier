#!/usr/bin/env python3
"""
Inject NetworkExtension target into Tauri-generated Xcode project.

Usage:
    python3 inject_ne_target.py <xcodeproj_path> <ne_sources_dir>

This script modifies the project.pbxproj to add:
1. A new NetworkExtension target (PacketTunnelProvider)
2. Build phases for the Swift sources
3. Linking against the easytier-ios-staticlib
4. Entitlements and Info.plist configuration
"""

import sys
import os
import re
import uuid
import shutil
from pathlib import Path
from typing import List, Dict, Optional

try:
    from mod_pbxproj import XcodeProject
except ImportError:
    print("Error: mod_pbxproj not installed. Install with: pip install mod-pbxproj")
    sys.exit(1)


def generate_uuid() -> str:
    """Generate a 24-character hex UUID for pbxproj (like Xcode does)."""
    return uuid.uuid4().hex[:24].upper()


class NETargetInjector:
    def __init__(self, project_path: Path, ne_sources_dir: Path):
        self.project_path = project_path
        self.ne_sources_dir = ne_sources_dir
        self.project = XcodeProject.load(str(project_path / "project.pbxproj"))
        self.main_target = self._find_main_target()
        self.ne_target_name = "HomeTierTunnel"
        self.bundle_id = "com.hometier.app"
        self.group_id = "group.com.hometier.app"

    def _find_main_target(self) -> Optional[Dict]:
        """Find the main app target in the project."""
        for target in self.project.get_targets():
            if target.get('productType') == 'com.apple.product-type.application':
                return target
        return None

    def inject(self) -> bool:
        """Perform the full injection."""
        print(f"Injecting NetworkExtension target into {self.project_path}")

        # 1. Add NE target
        ne_target = self._add_ne_target()
        if not ne_target:
            return False

        # 2. Add Swift source files to NE target
        if not self._add_swift_sources(ne_target):
            return False

        # 3. Add headers
        if not self._add_headers(ne_target):
            return False

        # 4. Configure build settings
        if not self._configure_build_settings(ne_target):
            return False

        # 5. Add entitlements
        if not self._add_entitlements(ne_target):
            return False

        # 6. Add Info.plist
        if not self._add_info_plist(ne_target):
            return False

        # 7. Add link against staticlib
        if not self._add_staticlib_linkage(ne_target):
            return False

        # 8. Save project
        self.project.save()
        print("Successfully injected NetworkExtension target")
        return True

    def _add_ne_target(self) -> Optional[Dict]:
        """Add the NetworkExtension target."""
        print(f"Adding target: {self.ne_target_name}")

        # Create target
        target = self.project.add_target(
            self.ne_target_name,
            'com.apple.product-type.app-extension',
            self.bundle_id + '.tunnel',
            'swift',
            deployment_target='15.0'
        )

        if not target:
            print("Failed to create NE target")
            return None

        # Set product name
        self.project.add_build_setting(
            'PRODUCT_NAME',
            'HomeTierTunnel',
            target=target.get('name')
        )

        # Set extension point identifier
        self.project.add_build_setting(
            'NSExtensionPointIdentifier',
            'com.apple.networkextension.packet-tunnel',
            target=target.get('name')
        )

        return target

    def _add_swift_sources(self, target: Dict) -> bool:
        """Add Swift source files to the NE target."""
        print("Adding Swift source files...")

        swift_files = [
            'PacketTunnelProvider.swift',
            'TunnelHelper.swift',
            'AddressHelper.swift',
            'BuilderHelper.swift',
        ]

        for swift_file in swift_files:
            src_path = self.ne_sources_dir / swift_file
            if not src_path.exists():
                print(f"Warning: {src_path} not found, skipping")
                continue

            # Add file to project
            file_ref = self.project.add_file(
                str(src_path),
                target=target.get('name')
            )
            if not file_ref:
                print(f"Failed to add {swift_file}")
                return False

        return True

    def _add_headers(self, target: Dict) -> bool:
        """Add C headers to the NE target."""
        print("Adding C headers...")

        headers = [
            'kern_control.h',
            'easytier_ios.h',
        ]

        for header in headers:
            src_path = self.ne_sources_dir / header
            if not src_path.exists():
                print(f"Warning: {src_path} not found, skipping")
                continue

            self.project.add_file(
                str(src_path),
                target=target.get('name')
            )

        return True

    def _configure_build_settings(self, target: Dict) -> bool:
        """Configure build settings for the NE target."""
        print("Configuring build settings...")

        target_name = target.get('name')

        # Swift settings
        self.project.add_build_setting('SWIFT_VERSION', '5.0', target=target_name)
        self.project.add_build_setting('SWIFT_OPTIMIZATION_LEVEL', '-O', target=target_name)

        # NetworkExtension settings
        self.project.add_build_setting(
            'CODE_SIGN_ENTITLEMENTS',
            'HomeTierTunnel/entitlements.entitlements',
            target=target_name
        )
        self.project.add_build_setting(
            'INFOPLIST_FILE',
            'HomeTierTunnel/Info.plist',
            target=target_name
        )

        # Link staticlib
        self.project.add_build_setting(
            'LIBRARY_SEARCH_PATHS',
            '$(BUILD_DIR)/$(CONFIGURATION)-$(PLATFORM_NAME)',
            target=target_name
        )
        self.project.add_build_setting(
            'OTHER_LDFLAGS',
            '-leasytier_ios_staticlib',
            target=target_name
        )

        # Header search paths
        self.project.add_build_setting(
            'HEADER_SEARCH_PATHS',
            '$(SRCROOT)/../gen-scripts/ios',
            target=target_name
        )

        # Enable modules
        self.project.add_build_setting('DEFINES_MODULE', 'YES', target=target_name)
        self.project.add_build_setting('SWIFT_OBJC_BRIDGING_HEADER', '', target=target_name)

        # iOS deployment target
        self.project.add_build_setting('IPHONEOS_DEPLOYMENT_TARGET', '15.0', target=target_name)

        return True

    def _add_entitlements(self, target: Dict) -> bool:
        """Add entitlements file to the NE target."""
        print("Adding entitlements...")

        entitlements_src = self.ne_sources_dir / 'entitlements.entitlements'
        if not entitlements_src.exists():
            print("Warning: entitlements.entitlements not found")
            return False

        # Copy to target directory
        target_dir = self.project_path.parent / self.ne_target_name
        target_dir.mkdir(exist_ok=True)
        entitlements_dst = target_dir / 'entitlements.entitlements'
        shutil.copy2(entitlements_src, entitlements_dst)

        # Add to project
        self.project.add_file(
            str(entitlements_dst.relative_to(self.project_path.parent)),
            target=target.get('name')
        )

        return True

    def _add_info_plist(self, target: Dict) -> bool:
        """Add Info.plist to the NE target."""
        print("Adding Info.plist...")

        plist_src = self.ne_sources_dir / 'Info.plist'
        if not plist_src.exists():
            print("Warning: Info.plist not found")
            return False

        target_dir = self.project_path.parent / self.ne_target_name
        target_dir.mkdir(exist_ok=True)
        plist_dst = target_dir / 'Info.plist'
        shutil.copy2(plist_src, plist_dst)

        self.project.add_file(
            str(plist_dst.relative_to(self.project_path.parent)),
            target=target.get('name')
        )

        return True

    def _add_staticlib_linkage(self, target: Dict) -> bool:
        """Add linkage against easytier-ios-staticlib."""
        print("Adding staticlib linkage...")

        # The staticlib should be built by cargo and placed in the build directory
        # We add a Run Script build phase to ensure it's built
        script = '''
# Build easytier-ios-staticlib
cd "${SRCROOT}/../.."
cargo build --target aarch64-apple-ios --manifest-path src-tauri/easytier-ios-staticlib/Cargo.toml --release 2>&1 | xcpretty
'''

        self.project.add_build_phase(
            target.get('name'),
            'Run Script',
            'Build easytier-ios-staticlib',
            script
        )

        return True


def main():
    if len(sys.argv) != 3:
        print("Usage: python3 inject_ne_target.py <xcodeproj_path> <ne_sources_dir>")
        sys.exit(1)

    xcodeproj_path = Path(sys.argv[1])
    ne_sources_dir = Path(sys.argv[2])

    if not xcodeproj_path.exists():
        print(f"Error: Xcode project not found at {xcodeproj_path}")
        sys.exit(1)

    if not ne_sources_dir.exists():
        print(f"Error: NE sources directory not found at {ne_sources_dir}")
        sys.exit(1)

    injector = NETargetInjector(xcodeproj_path, ne_sources_dir)
    if not injector.inject():
        print("Injection failed")
        sys.exit(1)

    print("Injection completed successfully")


if __name__ == '__main__':
    main()