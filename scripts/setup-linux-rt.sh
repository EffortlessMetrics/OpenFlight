#!/bin/bash
# Flight Hub Linux Real-Time Setup Script
# 
# This script configures Linux for real-time audio/input processing
# required by Flight Hub's 250Hz axis processing loop.
#
# Requirements:
# - Must be run as root (sudo)
# - Configures /etc/security/limits.conf for rtprio and memlock
# - Provides instructions for group membership
#
# Usage:
#   sudo ./setup-linux-rt.sh
#
# The script is idempotent - safe to run multiple times.

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Flight Hub marker for limits.conf entries
MARKER="# Flight Hub RT configuration"
MARKER_END="# End Flight Hub RT configuration"

print_header() {
    echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║${NC}        ${GREEN}Flight Hub Linux Real-Time Setup${NC}                    ${BLUE}║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
    echo
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

# Check if running as root
check_root() {
    if [[ $EUID -ne 0 ]]; then
        print_error "This script must be run as root (use sudo)"
        echo
        echo "Usage: sudo $0"
        exit 1
    fi
}

# Check if limits.conf already has Flight Hub configuration
has_flighthub_config() {
    grep -q "$MARKER" /etc/security/limits.conf 2>/dev/null
}

# Remove existing Flight Hub configuration from limits.conf
remove_existing_config() {
    if has_flighthub_config; then
        print_info "Removing existing Flight Hub configuration..."
        # Use sed to remove the block between markers (inclusive)
        sed -i "/$MARKER/,/$MARKER_END/d" /etc/security/limits.conf
    fi
}

# Configure /etc/security/limits.conf
configure_limits() {
    local limits_file="/etc/security/limits.conf"
    
    print_info "Configuring $limits_file..."
    
    # Backup the file if no backup exists
    if [[ ! -f "${limits_file}.flighthub_backup" ]]; then
        cp "$limits_file" "${limits_file}.flighthub_backup"
        print_success "Created backup at ${limits_file}.flighthub_backup"
    fi
    
    # Remove any existing Flight Hub config (for idempotency)
    remove_existing_config
    
    # Append new configuration
    cat >> "$limits_file" << EOF

$MARKER
# Allow audio group members to use real-time scheduling
@audio   -  rtprio     99
@audio   -  memlock    unlimited

# Alternative: Allow input group (for HID device access)
@input   -  rtprio     99
@input   -  memlock    unlimited
$MARKER_END
EOF
    
    print_success "Added RT limits for @audio and @input groups"
}

# Check and optionally install rtkit
check_rtkit() {
    echo
    print_info "Checking for rtkit (real-time policy kit)..."
    
    if command -v rtkit-daemon &> /dev/null || systemctl is-active --quiet rtkit-daemon 2>/dev/null; then
        print_success "rtkit is installed and available"
        return 0
    fi
    
    print_warning "rtkit is not installed"
    echo
    echo "rtkit allows unprivileged processes to acquire real-time scheduling."
    echo "Flight Hub will work without it, but rtkit provides better security."
    echo
    
    # Detect package manager and suggest install command
    if command -v apt-get &> /dev/null; then
        echo "To install rtkit on Debian/Ubuntu:"
        echo -e "  ${GREEN}sudo apt-get install rtkit${NC}"
    elif command -v dnf &> /dev/null; then
        echo "To install rtkit on Fedora:"
        echo -e "  ${GREEN}sudo dnf install rtkit${NC}"
    elif command -v pacman &> /dev/null; then
        echo "To install rtkit on Arch Linux:"
        echo -e "  ${GREEN}sudo pacman -S rtkit${NC}"
    elif command -v zypper &> /dev/null; then
        echo "To install rtkit on openSUSE:"
        echo -e "  ${GREEN}sudo zypper install rtkit${NC}"
    else
        echo "Please install rtkit using your distribution's package manager."
    fi
    echo
    
    # Ask if user wants to install rtkit
    read -p "Would you like to install rtkit now? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        install_rtkit
    fi
}

# Install rtkit based on detected package manager
install_rtkit() {
    print_info "Installing rtkit..."
    
    if command -v apt-get &> /dev/null; then
        apt-get update && apt-get install -y rtkit
    elif command -v dnf &> /dev/null; then
        dnf install -y rtkit
    elif command -v pacman &> /dev/null; then
        pacman -S --noconfirm rtkit
    elif command -v zypper &> /dev/null; then
        zypper install -y rtkit
    else
        print_error "Could not detect package manager. Please install rtkit manually."
        return 1
    fi
    
    # Enable and start rtkit service
    if systemctl is-enabled rtkit-daemon &> /dev/null || systemctl enable rtkit-daemon 2>/dev/null; then
        systemctl start rtkit-daemon 2>/dev/null || true
        print_success "rtkit installed and started"
    else
        print_success "rtkit installed"
    fi
}

# Print group membership instructions
print_group_instructions() {
    local current_user="${SUDO_USER:-$USER}"
    
    echo
    echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║${NC}              ${GREEN}Next Steps Required${NC}                           ${BLUE}║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
    echo
    echo "To enable real-time scheduling for Flight Hub, add your user to the"
    echo "'audio' group (recommended) or 'input' group:"
    echo
    echo -e "  ${GREEN}sudo usermod -a -G audio $current_user${NC}"
    echo
    echo "Or for the input group (also provides HID device access):"
    echo
    echo -e "  ${GREEN}sudo usermod -a -G input $current_user${NC}"
    echo
    echo -e "${YELLOW}IMPORTANT:${NC} You must ${YELLOW}log out and log back in${NC} (or reboot) for"
    echo "the group membership changes to take effect."
    echo
    echo "To verify your group membership after logging back in:"
    echo
    echo -e "  ${GREEN}groups${NC}"
    echo
    echo "You should see 'audio' or 'input' in the output."
    echo
}

# Print current status
print_status() {
    local current_user="${SUDO_USER:-$USER}"
    
    echo
    echo -e "${BLUE}Current Status:${NC}"
    echo
    
    # Check group membership
    if id -nG "$current_user" 2>/dev/null | grep -qw "audio"; then
        print_success "User '$current_user' is in the 'audio' group"
    else
        print_warning "User '$current_user' is NOT in the 'audio' group"
    fi
    
    if id -nG "$current_user" 2>/dev/null | grep -qw "input"; then
        print_success "User '$current_user' is in the 'input' group"
    else
        print_warning "User '$current_user' is NOT in the 'input' group"
    fi
    
    # Check current limits
    echo
    echo "Current RT limits for user '$current_user':"
    echo -n "  rtprio:  "
    su - "$current_user" -c "ulimit -r" 2>/dev/null || echo "unknown"
    echo -n "  memlock: "
    su - "$current_user" -c "ulimit -l" 2>/dev/null || echo "unknown"
}

# Verify the configuration
verify_config() {
    echo
    print_info "Verifying configuration..."
    
    if grep -q "@audio.*rtprio.*99" /etc/security/limits.conf; then
        print_success "rtprio limit configured for @audio group"
    else
        print_error "rtprio limit NOT configured correctly"
    fi
    
    if grep -q "@audio.*memlock.*unlimited" /etc/security/limits.conf; then
        print_success "memlock limit configured for @audio group"
    else
        print_error "memlock limit NOT configured correctly"
    fi
}

# Main function
main() {
    print_header
    
    check_root
    
    print_info "This script will configure your system for Flight Hub's"
    print_info "real-time 250Hz axis processing requirements."
    echo
    
    # Configure limits.conf
    configure_limits
    
    # Verify configuration
    verify_config
    
    # Check/install rtkit
    check_rtkit
    
    # Print current status
    print_status
    
    # Print instructions
    print_group_instructions
    
    echo -e "${GREEN}Setup complete!${NC}"
    echo
    echo "For more information, see the Flight Hub documentation:"
    echo "  https://github.com/openflight/flight-hub/docs/how-to/linux-rt-setup.md"
    echo
}

# Run main function
main "$@"
