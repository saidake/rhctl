```bash
#!/bin/bash

# ****************************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Tests the Nginx configuration and restarts the Nginx systemd service.
#
# Safe to re-run.
#
# Usage:
#   ./restart.sh
#
# Since : 0.1.0
# Date  : Oct 3, 2026
# ****************************************************************************************************

set -e

# ========================================================================= Check

if ! command -v nginx >/dev/null 2>&1; then
    echo "Error: Nginx is not installed." >&2
    exit 1
fi

if ! command -v systemctl >/dev/null 2>&1; then
    echo "Error: systemctl is required." >&2
    exit 1
fi

# ========================================================================= Test configuration

echo "Testing Nginx configuration..."

nginx -t

# ========================================================================= Restart

echo "Restarting Nginx..."

systemctl restart nginx

# ========================================================================= Final output

echo
echo "Nginx restart completed."
echo "Version: $(nginx -v 2>&1)"
echo "Service: $(systemctl is-active nginx 2>/dev/null || echo "unknown")"
```
