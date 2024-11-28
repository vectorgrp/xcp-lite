# XCP Daemon Example

Simple example of how the daemonizer can be used to run XCP as a daemon.

## Usage

```bash
cargo build; # Build the example
sudo chmod +x ./install.sh; # Make the install script executable
sudo ./install.sh; # Install the example (binaries and configuration files)
sudo systemctl start xcpd; # Start the daemon
sudo systemctl status xcpd; # Check the status of the daemon
sudo systemctl stop xcpd; # Stop the daemon
sudo systemctl reload xcpd; # Reload the daemon (XCP server will NOT be reloaded)
sudo systemctl restart xcpd; # Restart the daemon (XCP server will be reloaded)
sudo systemctl enable xcpd; # Enable the daemon to start at boot time
sudo systemctl disable xcpd; # Disable the daemon from starting at boot time
```
