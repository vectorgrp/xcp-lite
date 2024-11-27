#!/bin/bash

if systemctl is-active --quiet xcpd; then
    sudo systemctl stop xcpd || { echo "Failed to stop xcpd"; exit 1; }
else
    echo "xcp daemon not running"
fi

cp ./cfg/xcpd.service /etc/systemd/system && echo "Copied service file to /etc/systemd/system" || { echo "Failed to copy service file"; exit 1; }

mkdir -p /etc/xcpd || { echo "Failed to create directory /etc/xcpd"; exit 1; }

cp ./cfg/xcpd.conf /etc/xcpd && echo "Copied config file to /etc/xcpd" || { echo "Failed to copy config file"; exit 1; }

cp ../../target/debug/xcp_daemon /usr/bin && echo "Copied binary to /usr/bin" || { echo "Failed to copy binary"; exit 1; }

systemctl daemon-reload && echo "Reloaded daemons" || { echo "Failed to reload daemons"; exit 1; }

systemctl start xcpd && echo "Started xcpd" || { echo "Failed to start xcpd"; exit 1; }