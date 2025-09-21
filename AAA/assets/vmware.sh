#!/bin/bash

# Define the mount point
MOUNT_POINT="/mnt/hgfs"

# Unmount the directory if it's mounted
if mountpoint -q "$MOUNT_POINT"; then
    umount "$MOUNT_POINT" > /dev/null 2>&1
fi

# Mount the VMware shared folder
vmhgfs-fuse .host:vmshare "$MOUNT_POINT" > /dev/null 2>&1


