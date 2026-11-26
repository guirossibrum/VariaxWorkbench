# USB dependencies

- rusb 0.9.4, crates.io archive `rusb-0.9.4.crate`, SHA-256 `ab9f9ff05b63a786553a4c02943b74b34a988448671001e9a27e2f0565cc05a4`. Its manifest points to the sibling sys crate. Six elided return lifetimes are spelled out for the current Rust compiler.
- libusb1-sys 0.7.0, crates.io archive `libusb1-sys-0.7.0.crate`, SHA-256 `da050ade7ac4ff1ba5379af847a10a10a8e284181e060105bf8d86960ce9ce0f`. The archive carries libusb 1.0.27 under `libusb/`.

Darwin descriptor discovery targets the PODxt Live without opening it or changing device state. Session open uses ordinary `USBDeviceOpen` and reports exclusive access as busy. Claims and alternate changes return their original errors without reconfiguration, reclaim, or halt-clear fallbacks. Linux and Windows use the
original platform backends. Cargo enables the vendored static build on Darwin.
Upstream license texts are included in the corresponding directories.
