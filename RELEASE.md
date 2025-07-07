# Release Notes

First ever release, and probably the only release until the sdk gets updated. 

It is just a library compiled. Either cargo will download the release script or you can just tag this on to
the end of your own executable (when it gets bundled). 

---

## Available Platforms

- **Windows**: `win-x64`
- **Linux**: `linux-x64`
- **macOS**: `osx-x64`, `osx-arm64`
- No arm builds (can't figure out how to get my build to work sorry <(＿　＿)>)

## Archive Contents

- Individual platform archives: `proton-sdk-native-{platform}.{zip|tar.gz}`
- Combined archive: `proton-sdk-native-all-platforms.tar.gz`

---
Precautions: Ensure you and your app consumers are aware that the API is not stable. It is subject to change and is not production ready (cannot make any proper apps). 

Other than that, enjoy!