import json
import random
import os

def generate_cast():
    width = 100
    height = 30
    
    header = {
        "version": 2,
        "width": width,
        "height": height,
        "timestamp": 1718992000,
        "title": "iroh-usbip real-world sharing demo",
        "env": {"SHELL": "/bin/bash", "TERM": "xterm-256color"}
    }
    
    events = []
    current_time = 0.0
    
    def add_event(time_delta, data_type, data):
        nonlocal current_time
        current_time += time_delta
        events.append([round(current_time, 4), data_type, data])
        
    def type_string(s, speed_range=(0.04, 0.12)):
        for char in s:
            delay = random.uniform(*speed_range)
            add_event(delay, "o", char)
            
    def type_comment(s):
        # Type comments slightly faster but still natural
        type_string("# " + s, speed_range=(0.03, 0.07))
        add_event(0.3, "o", "\r\n")

    def print_lines(lines, delay_per_line=0.05):
        for line in lines:
            add_event(delay_per_line, "o", line + "\r\n")

    # Prompt variables for Host and Client
    host_prompt = "\r\n\u001b[1;32mroot\u001b[0m in \u001b[1;34m🌐 iroh-usbip-host\u001b[0m in \u001b[1;35m~\u001b[0m ❯ "
    client_prompt = "\r\n\u001b[1;32mroot\u001b[0m in \u001b[1;34m🌐 iroh-usbip-client\u001b[0m in \u001b[1;35m~\u001b[0m ❯ "

    # --- PHASE 1: Host setup and waiting ---
    add_event(0.5, "o", "\u001b[1;33m=== HOST TERMINAL ===\u001b[0m\r\n")
    add_event(0.3, "o", host_prompt)
    
    # Comment 1
    type_comment("1. List available USB devices on the Host")
    add_event(0.2, "o", host_prompt)
    
    # Type list command
    type_string("iroh-usbip list")
    add_event(0.3, "o", "\r\n")
    
    # Print list output
    list_output = [
        "Bus 001 Device 001: ID 1d6b:0001 Linux Foundation 1.1 root hub",
        "Bus 001 Device 002: ID 0627:0001 Adomax Technology Co., Ltd QEMU Tablet"
    ]
    print_lines(list_output)
    
    add_event(1.0, "o", host_prompt)
    
    # Comment 2
    type_comment("2. Share the QEMU Tablet (0627:0001) to get a connection ticket")
    add_event(0.2, "o", host_prompt)
    
    # Type share command
    type_string("iroh-usbip share --vid 0627")
    add_event(0.3, "o", "\r\n")
    
    share_initial = [
        "Sharing USB device Bus 001 Device 002: ID 0627:0001 QEMU QEMU USB Tablet...",
        "Connection ticket:",
        "endpointaaoj5z7ylpwmizgikhk352mave64budppioi36mxbc2vdze4yztwsbabaafhd4abv6oqeaiamruagmvptubacafzca6jnl45aiaqckqdiaaaacqcus4mk57772y547oexibq",
        "Waiting for client to connect..."
    ]
    print_lines(share_initial)
    
    # Pause to simulate waiting for client
    add_event(2.5, "o", "")

    # --- PHASE 2: Client Terminal (Connecting) ---
    add_event(0.5, "o", "\u001b[2J\u001b[H") # clear screen
    add_event(0.5, "o", "\u001b[1;36m=== CLIENT TERMINAL ===\u001b[0m\r\n")
    add_event(0.3, "o", client_prompt)
    
    # Comment 3
    type_comment("3. Attach the shared remote device using the connection ticket")
    add_event(0.2, "o", client_prompt)
    
    # Type attach command, then paste ticket
    type_string("iroh-usbip attach ")
    add_event(0.4, "o", "") # pause
    
    ticket = "endpointaaoj5z7ylpwmizgikhk352mave64budppioi36mxbc2vdze4yztwsbabaafhd4abv6oqeaiamruagmvptubacafzca6jnl45aiaqckqdiaaaacqcus4mk57772y547oexibq"
    add_event(0.1, "o", ticket)
    
    add_event(0.6, "o", "") # pause
    add_event(0.2, "o", "\r\n")
    
    attach_output = [
        "Connecting to remote shared device via Iroh P2P...",
        "Connected to Host! Querying shared devices...",
        "Found remote device: 1-2 (speed: Full)",
        "Attaching to local virtual port: 0",
        "Successfully attached virtual device to VHCI port 0!",
        "Device is now connected. Press Ctrl+C to disconnect."
    ]
    print_lines(attach_output)
    
    # Pause to simulate active connection
    add_event(2.5, "o", "")

    # --- PHASE 3: Host Terminal Updates (Success) ---
    add_event(0.5, "o", "\u001b[2J\u001b[H") # clear screen
    add_event(0.1, "o", "\u001b[1;33m=== HOST TERMINAL (UPDATED) ===\u001b[0m\r\n")
    
    # Re-draw the entire host terminal state up to waiting IMMEDIATELY in one block
    redraw_block = host_prompt + "# 1. List available USB devices on the Host\r\n"
    redraw_block += host_prompt + "iroh-usbip list\r\n"
    for line in list_output:
        redraw_block += line + "\r\n"
    redraw_block += host_prompt + "# 2. Share the QEMU Tablet (0627:0001) to get a connection ticket\r\n"
    redraw_block += host_prompt + "iroh-usbip share --vid 0627\r\n"
    for line in share_initial:
        redraw_block += line + "\r\n"
        
    add_event(0.1, "o", redraw_block)
        
    # Pause 1.2 seconds before showing client connection to simulate network time/delay
    add_event(1.2, "o", "Client connected! Establishing stream...\r\n")
    add_event(0.5, "o", "Session started. Redirecting USB traffic.\r\n")
    
    add_event(2.5, "o", "")

    # --- PHASE 4: Client Verification (lsusb) ---
    add_event(0.5, "o", "\u001b[2J\u001b[H") # clear screen
    add_event(0.5, "o", "\u001b[1;32m=== CLIENT VERIFICATION ===\u001b[0m\r\n")
    add_event(0.3, "o", client_prompt)
    
    # Comment 4
    type_comment("4. Verify that the virtual QEMU Tablet is mounted on Bus 002")
    add_event(0.2, "o", client_prompt)
    
    type_string("lsusb")
    add_event(0.3, "o", "\r\n")
    
    lsusb_output = [
        "Bus 001 Device 001: ID 1d6b:0001 Linux Foundation 1.1 root hub",
        "Bus 001 Device 002: ID 0627:0001 Adomax Technology Co., Ltd QEMU Tablet",
        "Bus 002 Device 001: ID 1d6b:0002 Linux Foundation 2.0 root hub",
        "Bus 002 Device 002: ID 0627:0001 Adomax Technology Co., Ltd QEMU Tablet",
        "Bus 003 Device 001: ID 1d6b:0003 Linux Foundation 3.0 root hub"
    ]
    print_lines(lsusb_output)
    add_event(2.0, "o", client_prompt)
    
    # Write to file at workspace root
    script_dir = os.path.dirname(os.path.abspath(__file__))
    workspace_root = os.path.dirname(script_dir)
    output_path = os.path.join(workspace_root, "demo.cast")
    
    with open(output_path, "w") as f:
        f.write(json.dumps(header) + "\n")
        for event in events:
            f.write(json.dumps(event) + "\n")
    print(f"Successfully generated asciicast at {output_path}")

if __name__ == "__main__":
    generate_cast()
