# proton-sdk-rs

Taken from [https://github.com/ProtonDriveApps/sdk](https://github.com/ProtonDriveApps/sdk), this project aims to create safe bindings to create Proton Drive apps with its sdk with a great language.

For now, it is a work in progress, so contributions are as always welcome and appreciated.

There are crates that exist, however they are merely name shells until I can fully implement both sides.

Specifically, there are 2 crates:

- proton-sdk-sys: Unsafe binding to the library
- proton-sdk-rs: Safe implementation of proton-sdk-sys

## Build

To build the project, there is a handy build script that I made. It will compile, clone and check dependencies.

> [!NOTE]
> The build script is only available for local development, however you can grab the libraries produced and create your own bindings or use the
> library for yourself.

```bash
python3 build.py
```

Grab yourself a cup of coffee while it generates the libraries and tests the cargo project.