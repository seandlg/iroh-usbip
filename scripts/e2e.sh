#!/bin/bash
set -euo pipefail

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --mock)
            export IROH_USBIP_MOCK=1
            shift
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

# Check if running as root
if [ "${IROH_USBIP_MOCK:-0}" != "1" ]; then
    if [ "$EUID" -ne 0 ]; then
        echo "Error: This script must be run as root (sudo)." >&2
        exit 1
    fi
fi

echo "=== Host-Level E2E Integration Test Script ==="

# Save the original directory
PROJECT_ROOT=$(pwd)
GADGET_DIR="/sys/kernel/config/usb_gadget/g1"

# Initialize process tracking variables
HOST_PID=""
CLIENT_PID=""

# Cleanup function to ensure clean state on exit
cleanup() {
    echo "=== Performing E2E Cleanup ==="
    
    # 1. Kill client daemon (send SIGINT to allow graceful VHCI detach)
    if [ -n "$CLIENT_PID" ]; then
        echo "Terminating client (PID: $CLIENT_PID)..."
        kill -INT "$CLIENT_PID" 2>/dev/null || true
        # Wait up to 5 seconds for client to exit
        for i in {1..50}; do
            if ! kill -0 "$CLIENT_PID" 2>/dev/null; then
                break
            fi
            sleep 0.1
        done
        # Force kill if still running
        kill -KILL "$CLIENT_PID" 2>/dev/null || true
    fi

    # 2. Kill host daemon
    if [ -n "$HOST_PID" ]; then
        echo "Terminating host (PID: $HOST_PID)..."
        kill -INT "$HOST_PID" 2>/dev/null || true
        # Wait up to 5 seconds for host to exit
        for i in {1..50}; do
            if ! kill -0 "$HOST_PID" 2>/dev/null; then
                break
            fi
            sleep 0.1
        done
        kill -KILL "$HOST_PID" 2>/dev/null || true
    fi

    # 3. Dismantle ConfigFS USB gadget (only in non-mock mode)
    if [ "${IROH_USBIP_MOCK:-0}" != "1" ]; then
        if [ -d "$GADGET_DIR" ]; then
            echo "Dismantling ConfigFS USB gadget at $GADGET_DIR..."
            # Unbind from UDC
            echo "" > "$GADGET_DIR/UDC" 2>/dev/null || true
            # Remove configuration symlinks
            rm -f "$GADGET_DIR"/configs/c.1/*.usb0 2>/dev/null || true
            # Remove configuration strings and directory
            if [ -d "$GADGET_DIR/configs/c.1/strings/0x409" ]; then
                rmdir "$GADGET_DIR/configs/c.1/strings/0x409" 2>/dev/null || true
            fi
            if [ -d "$GADGET_DIR/configs/c.1" ]; then
                rmdir "$GADGET_DIR/configs/c.1" 2>/dev/null || true
            fi
            # Remove functions
            if [ -d "$GADGET_DIR/functions/acm.usb0" ]; then
                rmdir "$GADGET_DIR/functions/acm.usb0" 2>/dev/null || true
            fi
            # Remove gadget strings and main directory
            if [ -d "$GADGET_DIR/strings/0x409" ]; then
                rmdir "$GADGET_DIR/strings/0x409" 2>/dev/null || true
            fi
            rmdir "$GADGET_DIR" 2>/dev/null || true
        fi
    else
        echo "Cleaning up mock sysfs directory..."
        rm -rf target/mock_sysfs
    fi

    echo "=== E2E Cleanup Completed ==="
}

# Set the EXIT trap
trap cleanup EXIT

if [ "${IROH_USBIP_MOCK:-0}" != "1" ]; then
    # 1. Load kernel modules
    echo "Loading required kernel modules..."
    modprobe vhci-hcd || echo "Warning: vhci-hcd failed to load, assuming built-in or already active."
    modprobe dummy-hcd || echo "Warning: dummy-hcd failed to load, assuming built-in or already active."
    modprobe libcomposite || echo "Warning: libcomposite failed to load, assuming built-in or already active."

    # 2. Mount configfs if not mounted
    if [ ! -d /sys/kernel/config/usb_gadget ]; then
        echo "Mounting ConfigFS..."
        mount -t configfs none /sys/kernel/config || true
    fi

    # 3. Configure virtual USB gadget g1
    echo "Configuring virtual USB gadget..."
    if [ -d "$GADGET_DIR" ]; then
        echo "Stale gadget found, cleaning up before configuring..."
        echo "" > "$GADGET_DIR/UDC" 2>/dev/null || true
        rm -f "$GADGET_DIR"/configs/c.1/*.usb0 2>/dev/null || true
        rmdir "$GADGET_DIR"/configs/c.1/strings/0x409 2>/dev/null || true
        rmdir "$GADGET_DIR"/configs/c.1 2>/dev/null || true
        rmdir "$GADGET_DIR"/functions/acm.usb0 2>/dev/null || true
        rmdir "$GADGET_DIR"/strings/0x409 2>/dev/null || true
        rmdir "$GADGET_DIR" 2>/dev/null || true
    fi

    mkdir -p "$GADGET_DIR"
    cd "$GADGET_DIR"

    # USB ID properties (using standard Linux Foundation / Multifunction Composite Gadget IDs)
    echo "0x1d6b" > idVendor
    echo "0x0104" > idProduct
    mkdir -p strings/0x409
    echo "0123456789" > strings/0x409/serialnumber
    echo "Antigravity" > strings/0x409/manufacturer
    echo "E2E_Virtual_Gadget" > strings/0x409/product

    # Configuration definition
    mkdir -p configs/c.1/strings/0x409
    echo "Conf 1" > configs/c.1/strings/0x409/configuration
    echo 250 > configs/c.1/MaxPower

    # Function definition (ACM serial port)
    mkdir -p functions/acm.usb0
    ln -s functions/acm.usb0 configs/c.1/acm.usb0

    # Bind to UDC
    UDC_NAME=$(ls /sys/class/udc | head -n 1)
    if [ -z "$UDC_NAME" ]; then
        echo "Error: No virtual UDC found in /sys/class/udc. Make sure dummy-hcd is loaded." >&2
        exit 1
    fi
    echo "Binding gadget to UDC: $UDC_NAME"
    echo "$UDC_NAME" > UDC

    # Return to project root
    cd "$PROJECT_ROOT"
fi

# 4. Build iroh-usbip binary
echo "Ensuring iroh-usbip is compiled..."
if [ -n "${SUDO_USER:-}" ] && [ "$EUID" -eq 0 ]; then
    # Run cargo build as the original user to avoid polluting the target directory with root-owned files
    sudo -u "$SUDO_USER" env "PATH=$PATH" "HOME=$HOME" cargo build
else
    cargo build
fi

# 5. Share the gadget device
echo "Sharing physical/mock USB device (VID: 1d6b, PID: 0104)..."
HOST_LOG=$(mktemp)
./target/debug/iroh-usbip share --vid "1d6b" --pid "0104" > "$HOST_LOG" 2>&1 &
HOST_PID=$!

# Wait and capture connection ticket
echo "Waiting for connection ticket to be generated..."
TICKET=""
for i in {1..30}; do
    if grep -q "Connection ticket:" "$HOST_LOG"; then
        TICKET=$(grep -A 1 "Connection ticket:" "$HOST_LOG" | tail -n 1 | xargs)
        if [ -n "$TICKET" ]; then
            break
        fi
    fi
    sleep 1
done

if [ -z "$TICKET" ]; then
    echo "Error: Connection ticket not generated." >&2
    echo "=== Host Daemon Output ===" >&2
    cat "$HOST_LOG" >&2
    exit 1
fi

echo "Ticket successfully captured!"

# 6. Connect client daemon to the host and attach the device
echo "Starting client daemon to attach remote device..."
CLIENT_LOG=$(mktemp)
./target/debug/iroh-usbip attach "$TICKET" > "$CLIENT_LOG" 2>&1 &
CLIENT_PID=$!

# 7. Find active VHCI sysfs path
if [ "${IROH_USBIP_MOCK:-0}" = "1" ]; then
    VHCI_DIR="target/mock_sysfs"
else
    VHCI_DIR=""
    for candidate in /sys/devices/platform/vhci_hcd.0 /sys/devices/platform/vhci-hcd.0 /sys/devices/platform/vhci_hcd /sys/devices/platform/vhci-hcd; do
        if [ -d "$candidate" ]; then
            VHCI_DIR="$candidate"
            break
        fi
    done

    if [ -z "$VHCI_DIR" ]; then
        echo "Error: VHCI sysfs directory not found." >&2
        exit 1
    fi
fi
echo "VHCI directory found at: $VHCI_DIR"

# 8. Assert device is correctly attached using lsusb and sysfs status
echo "Verifying device attachment..."
VERIFIED=false

if [ "${IROH_USBIP_MOCK:-0}" = "1" ]; then
    # In mock mode, we bypass lsusb and check the mock status file directly
    for i in {1..30}; do
        if [ -f "$VHCI_DIR/status" ]; then
            echo "--- Current VHCI Status ---"
            cat "$VHCI_DIR/status"
            echo "---------------------------"

            if awk 'NR>1 { if ($6 != "000000" && $6 != "0" && $6 != "") found=1; } END { if (found) exit 0; else exit 1; }' "$VHCI_DIR/status" 2>/dev/null; then
                echo "Device confirmed attached in VHCI status!"
                VERIFIED=true
                break
            fi
        fi
        sleep 1
    done
else
    # Standard physical verification using lsusb and real status file
    for i in {1..30}; do
        # Check lsusb matches the vendor/product descriptors
        if lsusb | grep -q "1d6b:0104"; then
            echo "Device found in lsusb!"

            # Check vhci status file for active connection (non-zero socket fd)
            if [ -f "$VHCI_DIR/status" ]; then
                echo "--- Current VHCI Status ---"
                cat "$VHCI_DIR/status"
                echo "---------------------------"

                if awk 'NR>1 { if ($6 != "000000" && $6 != "0" && $6 != "") found=1; } END { if (found) exit 0; else exit 1; }' "$VHCI_DIR/status" 2>/dev/null; then
                    echo "Device confirmed attached in VHCI status!"
                    VERIFIED=true
                    break
                fi
            fi
        fi
        sleep 1
    done
fi

if [ "$VERIFIED" = "false" ]; then
    echo "Error: Verification failed. Device was not successfully attached." >&2
    echo "=== Host Daemon Output ===" >&2
    cat "$HOST_LOG" >&2
    echo "=== Client Daemon Output ===" >&2
    cat "$CLIENT_LOG" >&2
    exit 1
fi

echo "E2E Integration Test PASSED successfully!"
exit 0
