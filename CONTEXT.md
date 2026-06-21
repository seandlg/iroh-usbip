# iroh-usbip

Securely share and access USB devices over the internet with zero configuration using peer-to-peer networking.

## Language

**Host**:
A node that shares one or more of its local physical USB devices with the network.
_Avoid_: Provider, server, exporter

**Client**:
A node that attaches and mounts a shared remote USB device, exposing it locally as a virtual USB device.
_Avoid_: Consumer, guest, importer

**Physical Device**:
A hardware USB device physically plugged into a Host.
_Avoid_: Real device, local device

**Virtual Device**:
A software-simulated USB device created on a Client that mirrors a Physical Device.
_Avoid_: Emulated device, remote device

**Bridge**:
The system component that forwards USB packets between a local interface and a remote peer.
_Avoid_: Tunnel, proxy, redirector
