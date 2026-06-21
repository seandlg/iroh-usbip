# Use Iroh Connection Tickets for Peer Discovery

To achieve zero-configuration peer-to-peer networking over the internet, we decided to use Iroh Connection Tickets as the primary connection establishment mechanism. When a Host shares a device, it will generate a ticket containing its Node ID and current network coordinates (relays and direct addresses). The Client will consume this ticket to locate and connect to the Host, avoiding the need for manual port forwarding, static IP configuration, or a custom DHT.
