#!/bin/bash

sudo systemctl stop xcp_demo_daemon || { echo "Failed to stop xcp_demo_daemon"; exit 1; }

cp ./cfg/xcp_demo_daemon.service /etc/systemd/system && echo "Copied service file to /etc/systemd/system" || { echo "Failed to copy service file"; exit 1; }

mkdir -p /etc/xcp_demo_daemon || { echo "Failed to create directory /etc/xcp_demo_daemon"; exit 1; }

cp ./cfg/xcp_demo_daemon.conf /etc/xcp_demo_daemon && echo "Copied config file to /etc/xcp_demo_daemon" || { echo "Failed to copy config file"; exit 1; }

cp ../../target/debug/xcp_daemon /usr/bin && echo "Copied binary to /usr/bin" || { echo "Failed to copy binary"; exit 1; }

systemctl daemon-reload && echo "Reloaded daemons" || { echo "Failed to reload daemons"; exit 1; }