# XCP Daemon Example

Simple example of how the daemonizer can be used to run XCP as a daemon.

## Usage

```bash
cargo build; # Build the example
sudo chmod +x ./install.sh; # Make the install script executable
sudo ./install.sh; # Install the example (binaries and configuration files)
sudo systemctl start xcp_daemon; # Start the daemon
sudo systemctl status xcp_daemon; # Check the status of the daemon
sudo systemctl stop xcp_daemon; # Stop the daemon
sudo systemctl reload xcp_daemon; # Reload the daemon (XCP server will NOT be reloaded)
sudo systemctl restart xcp_daemon; # Restart the daemon (XCP server will be reloaded)
sudo systemctl enable xcp_daemon; # Enable the daemon to start at boot time
sudo systemctl disable xcp_daemon; # Disable the daemon from starting at boot time
```
