# Use Single-Use Capability Tickets for Security

To secure the "ticket-only" authorization model, we decided that connections are single-use by default. When a Host shares a physical USB device, it generates a connection ticket. Once a Client mounts the device, the Host stops accepting new connections. If the Client disconnects, the sharing session on the Host terminates immediately. This minimizes the window of vulnerability and ensures a compromised or leaked ticket cannot be reused to access the hardware device later.
