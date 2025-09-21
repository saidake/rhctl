#!/bin/bash

source "$(dirname "${BASH_SOURCE[0]}")/AAA/config/global.sh"

# ================================================================== Required Configurations
# Import global environment variables
echo "------------------------------------------------- cpfiles.sh - Test Case 1"
bash ./scripts/cpfiles.sh
echo "------------------------------------------------- cpfiles.sh - Test Case 2"
bash ./scripts/cpfiles.sh ./scripts/AAA/assets/example-env.sh
echo "------------------------------------------------- cpfiles.sh - Test Case 3"
bash ./scripts/cpfiles.sh ./scripts/AAA/assets/example-env-sudo.sh
echo "------------------------------------------------- execr.sh - Test Case 1"
bash ./scripts/execr.sh ./scripts/AAA/assets/example-bash.sh
echo "------------------------------------------------- patchr.sh - Test Case 1"
bash ./scripts/patchr.sh
echo "------------------------------------------------- patchr.sh - Test Case 2"
bash ./scripts/patchr.sh recover