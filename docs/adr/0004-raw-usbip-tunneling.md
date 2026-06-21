# Tunnel Raw USBIP Protocol over Iroh QUIC Streams

To minimize transport overhead and avoid custom serialization layers, we decided to tunnel the standard USBIP wire protocol directly over Iroh QUIC streams. Because Iroh's underlying QUIC transport already guarantees ordered, reliable, and encrypted delivery, we do not need custom framing or envelope serialization. The Client-side bridge will perform transparent byte-forwarding between the local kernel-facing TCP loopback socket and the remote Iroh stream, while the Host-side daemon will parse standard USBIP packets directly from the stream.
