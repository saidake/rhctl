#!/bin/bash

# ****************************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Creates NATS JetStream streams with the specified subjects.
#
# If a stream already exists, its configuration is not modified.
#
# Usage:
#   ./create-streams.sh \
#     --stream STREAM_1 \
#       --subject "subject.test1.>" \
#       --subject "subject.test2.>" \
#     --stream STREAM_2 \
#       --subject "subject.test3.>" \
#       --subject "subject.test4.>"
#
#   ./create-streams.sh 
#     --port 4222 \
#     --stream STREAM_1 \
#       --subject "subject.test1.>" \
#       --subject "subject.test2.>" \
#     --stream STREAM_2 \
#       --subject "subject.test3.>" \
#       --subject "subject.test4.>"
# Required Parameters:
#   --stream <name>
#       JetStream stream name.
#       Can be specified multiple times.
#
#   --subject <subject>
#       Subject pattern for the most recently specified stream.
#       Can be specified multiple times.
#
#       A --subject must follow a --stream.
#
# Optional Parameters:
#   --port <port>
#       Listen port written to nats.conf (default: `4222`).
#         Example port values: `4222`, `4223`, `14222`
#
# Override Parameters:
#   RHCTL_NATS_PORT=<port>
#       Same as `--port`.
#   RHCTL_STREAMS=<streams>
#       Comma-separated stream definitions.
#
#       Each stream definition uses:
#
#         <name>:<subject>|<subject>|...
#
#       Example:
#         RHCTL_STREAMS="STREAM_1:subject.test1.>|subject.test2.>,STREAM_2:subject.test3.>"
#
# Since : 1.0.0
# Date  : Oct 3, 2026
# ****************************************************************************************************

set -euo pipefail

# ========================================================================= Parameter

STREAM_NAMES=()
STREAM_SUBJECTS=()

CURRENT_STREAM_INDEX=-1
RHCTL_NATS_PORT="${RHCTL_NATS_PORT:-4222}"  

if [[ -n "${RHCTL_STREAMS:-}" ]]; then
    IFS=',' read -ra ENV_STREAMS <<< "${RHCTL_STREAMS}"

    for STREAM in "${ENV_STREAMS[@]}"; do
        if [[ "${STREAM}" != *:* ]]; then
            echo "Error: Invalid stream definition: ${STREAM}" >&2
            exit 1
        fi

        STREAM_NAME="${STREAM%%:*}"
        SUBJECT_LIST="${STREAM#*:}"

        STREAM_NAMES+=("${STREAM_NAME}")
        STREAM_SUBJECTS+=("${SUBJECT_LIST}")
    done
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --port)
            [[ $# -ge 2 ]] || {
                echo "Error: --port requires a value." >&2
                exit 1
            }
            RHCTL_NATS_PORT="$2"
            shift 2
            ;;
        --stream)
            [[ $# -ge 2 ]] || {
                echo "Error: --stream requires a value." >&2
                exit 1
            }

            STREAM_NAMES+=("$2")
            STREAM_SUBJECTS+=("")
            CURRENT_STREAM_INDEX=$((${#STREAM_NAMES[@]} - 1))

            shift 2
            ;;

        --subject)
            [[ $# -ge 2 ]] || {
                echo "Error: --subject requires a value." >&2
                exit 1
            }

            if (( CURRENT_STREAM_INDEX < 0 )); then
                echo "Error: --subject must follow --stream." >&2
                exit 1
            fi

            if [[ -n "${STREAM_SUBJECTS[CURRENT_STREAM_INDEX]}" ]]; then
                STREAM_SUBJECTS[CURRENT_STREAM_INDEX]+="|$2"
            else
                STREAM_SUBJECTS[CURRENT_STREAM_INDEX]="$2"
            fi

            shift 2
            ;;

        *)
            echo "Error: Unknown option $1" >&2
            exit 1
            ;;
    esac
done

if [[ ${#STREAM_NAMES[@]} -eq 0 ]]; then
    echo "Error: At least one --stream is required." >&2
    exit 1
fi

# ========================================================================= Constants

# ========================================================================= Methods

die() {
    echo "Error: $*" >&2
    exit 1
}

validate_stream_name() {
    local NAME="$1"

    if [[ -z "${NAME}" ]]; then
        die "Stream name cannot be empty."
    fi

    if ! [[ "${NAME}" =~ ^[A-Za-z0-9_-]+$ ]]; then
        die "Invalid stream name: ${NAME}"
    fi
}

# ========================================================================= Check NATS CLI

if ! command -v nats >/dev/null 2>&1; then
    die "nats CLI is not installed."
fi

export NATS_URL="nats://127.0.0.1:${RHCTL_NATS_PORT}"   # NATS will automatically read this
# ========================================================================= Validate parameters

for ((INDEX = 0; INDEX < ${#STREAM_NAMES[@]}; INDEX++)); do
    STREAM_NAME="${STREAM_NAMES[INDEX]}"
    SUBJECT_LIST="${STREAM_SUBJECTS[INDEX]}"

    validate_stream_name "${STREAM_NAME}"

    if [[ -z "${SUBJECT_LIST}" ]]; then
        die "At least one --subject is required for stream: ${STREAM_NAME}"
    fi
done

# ========================================================================= Create streams

for ((INDEX = 0; INDEX < ${#STREAM_NAMES[@]}; INDEX++)); do
    STREAM_NAME="${STREAM_NAMES[INDEX]}"
    SUBJECT_LIST="${STREAM_SUBJECTS[INDEX]}"

    IFS='|' read -ra SUBJECTS <<< "${SUBJECT_LIST}"

    echo "Configuring stream: ${STREAM_NAME}"
    echo "  Subjects:"

    for SUBJECT in "${SUBJECTS[@]}"; do
        echo "    - ${SUBJECT}"
    done

    if nats stream info "${STREAM_NAME}" >/dev/null 2>&1; then
        echo ""
        echo "  Stream already exists. Skipping."
        echo ""
        continue
    fi

    echo ""
    echo "  Creating stream..."

    SUBJECT_ARGS=()

    for SUBJECT in "${SUBJECTS[@]}"; do
        SUBJECT_ARGS+=(--subjects "${SUBJECT}")
    done

    nats stream add "${STREAM_NAME}" \
        "${SUBJECT_ARGS[@]}" \
        --storage file \
        --retention limits \
        --discard old \
        --max-msgs=-1 \
        --max-bytes=-1 \
        --max-age=0 \
        --max-msg-size=-1 \
        --dupe-window=2m \
        --replicas=1 \
        --no-deny-delete \
        --no-deny-purge \
        --defaults

    echo ""
done

# ========================================================================= Final output

echo ""
echo " JetStream Configuration Complete"
echo ""
echo " Streams:"

for ((INDEX = 0; INDEX < ${#STREAM_NAMES[@]}; INDEX++)); do
    STREAM_NAME="${STREAM_NAMES[INDEX]}"
    SUBJECT_LIST="${STREAM_SUBJECTS[INDEX]}"

    echo "   - ${STREAM_NAME}"

    IFS='|' read -ra SUBJECTS <<< "${SUBJECT_LIST}"

    for SUBJECT in "${SUBJECTS[@]}"; do
        echo "       - ${SUBJECT}"
    done
done

echo ""
echo " JetStream Streams:"
echo ""

nats stream ls