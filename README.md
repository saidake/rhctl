# Table of Contents
- [Table of Contents](#table-of-contents)
- [Introduction](#introduction)
- [Core Commands](#core-commands)
  - [sbxctl upload](#sbxctl-upload)
  - [sbxctl execute](#sbxctl-execute)
  - [sbxctl patch](#sbxctl-patch)
- [Environment Configuration Helper](#environment-configuration-helper)
  - [Docker and Docker Compose](#docker-and-docker-compose)
    - [Installing on a Remote Linux Host](#installing-on-a-remote-linux-host)
  - [Docker Desktop](#docker-desktop)
    - [Installing on Local Windows](#installing-on-local-windows)
  - [AWS LocalStack](#aws-localstack)
    - [Installing on a Remote Linux Host](#installing-on-a-remote-linux-host-1)
  - [MailHog](#mailhog)
    - [Installing on Local Windows](#installing-on-local-windows-1)
  - [RocksDB](#rocksdb)
    - [Installing on a Remote Linux Host](#installing-on-a-remote-linux-host-2)
  - [Typesense](#typesense)
    - [Installing on a Remote Linux Host](#installing-on-a-remote-linux-host-3)

# Introduction
**Sandbox Control (sbxctl)** is a high-performance Rust CLI tool for remote server management, enabling file transfers, script execution, and file patching via SSH. 

# Core Commands
## sbxctl upload
[Back to Top](#table-of-contents)  
Automates uploading files from a local `assets-root` directory to mapped remote paths, according to rules defined in a `properties` file. This command replicates the functionality of the previous `cpfiles.sh` script.

**Prerequisites**:
1. Ensure `sbxctl` is built and executable:
   ```bash
   cargo build --release
   ```
2. Configure server variables in a YAML configuration file (e.g., `config.yml`):
   ```yaml
   remote:
     host: "192.168.75.128"
     user: "test99"
     ssh_port: 22
     password: "testpwd"
   upload:
     properties_file: "AAA/config/path-mapping.properties"
     assets_root: "AAA/assets"
     use_rsync: false
     use_sudo: false
     silent: false
   ```
3. Alternatively, provide configuration via CLI arguments (`--host`, `--user`, `--port`, `--password`).

**Examples**:
- Using default settings from `config.yml`:
  ```bash
  sbxctl --config config.yml upload
  ```

  Copies `example1.txt` to `/home/test99/examples` and `example2.txt`, `example3.txt` from `AAA/assets/exampledir` to `/home/test99/examples/targetdir` on the remote server.

- Specifying properties and assets root via CLI:
  ```bash
  sbxctl --host 192.168.75.128 --user test99 --port 22 upload --properties AAA/config/path-mapping.properties --assets-root AAA/assets
  ```
  Same as above, but overrides config file settings.

**Usage**:
```bash
sbxctl [--config <config.yml>] [--host <host>] [--user <user>] [--port <port>] [--password <password>] [--use-sudo] [--use-rsync] [--silent] [--log-level <level>] upload [--properties <properties>] [--assets-root <assets-root>]
```
- `<properties>`: Path to the properties file defining file mappings (e.g., `AAA/config/path-mapping.properties`).
- `<assets-root>`: Base directory for local files to upload (e.g., `AAA/assets`).

**Options**:
- `--use-rsync`: Use `rsync` for uploading instead of `scp` if available (default: `false`).
- `--use-sudo`: Execute commands with `sudo` privileges on the remote server (default: `false`). Note: With `sudo`, `~` resolves to `/root`.
- `--silent`: Suppress confirmation prompts (auto-approve overwrites) (default: `false`).
- `--log-level`: Set logging level (`debug`, `info`, `warn`, `error`) (default: `info`).  
<br/>
- `--properties`: Path to the properties file (overrides config file).
- `--assets-root`: Base directory for local files (overrides config file).

**Properties File Format** (e.g., `AAA/config/path-mapping.properties`):
```
example1.txt=~/examples
exampledir=~/examples/targetdir
```

## sbxctl execute
[Back to Top](#table-of-contents)  
Executes a local Bash script on a remote server in a specified working directory. This command replicates the functionality of the previous `execr.sh` script, with real-time output streaming.

**Prerequisites**:
1. Ensure `sbxctl` is built:
   ```bash
   cargo build --release
   ```
2. Configure server variables in `config.yml` (as shown above) or via CLI arguments.
3. Ensure the local Bash script exists (e.g., `AAA/assets/example-bash.sh`).

**Examples**:
- Using config file:
  ```bash
  sbxctl --config config.yml execute AAA/assets/example-bash.sh ~/examples
  ```
  Executes `AAA/assets/example-bash.sh` in `/home/test99/examples` on the remote server.

- Using CLI arguments:
  ```bash
  sbxctl --host 192.168.75.128 --user test99 --port 22 execute AAA/assets/example-bash.sh ~/examples
  ```

**Usage**:
```bash
sbxctl [--config <config.yml>] [--host <host>] [--user <user>] [--port <port>] [--password <password>] [--use-sudo] [--use-rsync] [--silent] [--log-level <level>] execute <script> [--remote-path <remote-path>]
```
- `<script>`: Path to the local Bash script to execute.
- `--remote-path`: Remote working directory (default: `~`).

**Options**:
- `--use-rsync`: Use `rsync` for uploading the script in `sudo` mode (default: `false`).
- `--use-sudo`: Execute the script with `sudo` privileges (uploads script to a temporary path) (default: `false`).
- `--silent`: Suppress confirmation prompts (default: `false`).
- `--log-level`: Set logging level (`debug`, `info`, `warn`, `error`) (default: `info`).

**Example Script** (e.g., `AAA/assets/example-bash.sh`):
```bash
#!/bin/bash
pwd
echo "Remote Execution"
```

## sbxctl patch
[Back to Top](#table-of-contents)  
Safely patches a remote file by uploading a local patch file, backing up the target file, and applying the patch, or recovering from a backup. This command replicates the functionality of the previous `patchr.sh` script.

**Prerequisites**:
1. Ensure `sbxctl` is built:
   ```bash
   cargo build --release
   ```
2. Configure server variables in `config.yml` (as shown above) or via CLI arguments.
3. Ensure the local patch file exists (e.g., `AAA/assets/example-patch.txt`).
4. [Optional] Create a test file on the remote server (e.g., `~/examples/example-patch-remote.txt`).

**Examples**:
- Patch mode (using config file):
  ```bash
  sbxctl --config config.yml patch
  ```
  Patches `/home/test99/examples/example-patch-remote.txt` with `AAA/assets/example-patch.txt`, backing up to `/home/test99/tmp/example-patch-remote.txt.bak`.

- Patch mode (CLI arguments):
  ```bash
  sbxctl --host 192.168.75.128 --user test99 --port 22 patch --local-patch AAA/assets/example-patch.txt --remote-upload ~/tmp/example-patch.txt.upload --remote-file ~/examples/example-patch-remote.txt --remote-backup ~/tmp/example-patch-remote.txt.bak
  ```

- Recover mode:
  ```bash
  sbxctl --config config.yml patch --recover
  ```
  Restores `/home/test99/examples/example-patch-remote.txt` from `/home/test99/tmp/example-patch-remote.txt.bak`.

**Usage**:
```bash
sbxctl [--config <config.yml>] [--host <host>] [--user <user>] [--port <port>] [--password <password>] [--use-sudo] [--use-rsync] [--silent] [--log-level <level>] patch [--local-patch <local-patch>] [--remote-upload <remote-upload>] [--remote-file <remote-file>] [--remote-backup <remote-backup>] [--recover]
```
- `--local-patch`: Path to the local patch file.
- `--remote-upload`: Temporary remote path for the uploaded patch file.
- `--remote-file`: Remote file to patch.
- `--remote-backup`: Path for the backup file.
- `--recover`: Restore from backup (default: `false`).
- `--use-rsync`, `--use-sudo`, `--silent`, `--log-level`: Same as above.

**Steps (Patch Mode)**:
1. Upload `local-patch` to `remote-upload`.
2. Backup `remote-file` to `remote-backup`.
3. Overwrite `remote-file` with `remote-upload`.

**Steps (Recover Mode)**:
1. Restore `remote-file` from `remote-backup`.

# Environment Configuration Helper
## Docker and Docker Compose
Docker is a platform that enables you to package, distribute, and run applications in lightweight, portable containers. Docker Compose is a tool for defining and managing multi-container Docker applications using YAML files.

### Installing on a Remote Linux Host
[Back to Top](#table-of-contents)  
**Prerequisites**:
1. Configure `config.yml` or CLI arguments for SSH access (see [sbxctl upload](#sbxctl-upload)).
2. Ensure the remote server has internet access.

**Commands**:
```bash
sbxctl --config config.yml execute ./scripts/docker/install.sh ~/examples
```
Installs Docker and Docker Compose on the remote server.

**Example Success Output**:
```
[INFO] Connecting to test99@192.168.75.128:22
[INFO] Executing script in '/home/test99/examples'
...
[INFO] Docker installed successfully: Docker version 28.3.2, build 578ccf6
...
[INFO] Docker Compose installed successfully: Docker Compose version v2.38.2
[INFO] Execution complete.
```

## Docker Desktop
Docker Desktop is an easy-to-install application for building, sharing, and running containerized applications on Windows and Mac.

### Installing on Local Windows
[Back to Top](#table-of-contents)  
**Prerequisites**:
1. Open Command Prompt with administrator privileges and navigate to the project root directory.

**Commands**:
```bash
call scripts\docker\install.bat
```
Installs Docker Desktop locally on Windows.

## AWS LocalStack
LocalStack is a local AWS cloud stack emulator for testing AWS services.

### Installing on a Remote Linux Host
[Back to Top](#table-of-contents)  
**Prerequisites**:
1. Configure `config.yml` or CLI arguments for SSH access.
2. Docker and Docker Compose are installed on the remote server (see [Docker and Docker Compose](#docker-and-docker-compose)).

**Commands**:
```bash
sbxctl --config config.yml upload --properties ./scripts/aws/cpfiles-env.sh --assets-root ./scripts/aws/assets
sbxctl --config config.yml execute ./scripts/aws/localstack-start.sh ~/examples
```
- Uploads `scripts/aws/assets/docker-compose.yml` to `/opt/sandbox/aws` on the remote server.
- Starts LocalStack service.

```bash
sbxctl --config config.yml execute ./scripts/aws/localstack-stop.sh ~/examples
```
Stops LocalStack service.

## MailHog
MailHog is a lightweight email testing tool that acts as a local SMTP server.

### Installing on Local Windows
[Back to Top](#table-of-contents)  
**Prerequisites**:
1. Docker Desktop is installed and running (see [Docker Desktop](#docker-desktop)).

**Commands**:
```bash
call scripts\mailhog\start.bat
```
Installs and runs the MailHog Docker image.

```bash
call scripts\mailhog\stop.bat
```
Stops the MailHog Docker image.

**Access**:
- SMTP server: http://localhost:1025
- Web UI: http://localhost:8025

## RocksDB
RocksDB is a high-performance embedded key-value store optimized for low-latency data access.

### Installing on a Remote Linux Host
[Back to Top](#table-of-contents)  
**Prerequisites**:
1. Configure `config.yml` or CLI arguments for SSH access.

**Commands**:
```bash
sbxctl --config config.yml execute ./scripts/rocksdb/install.sh ~/examples
```
Installs RocksDB.

```bash
sbxctl --config config.yml execute ./scripts/rocksdb/uninstall.sh ~/examples
```
Uninstalls RocksDB.

## Typesense
Typesense is an open-source, fast, typo-tolerant search engine for building instant search experiences.

### Installing on a Remote Linux Host
[Back to Top](#table-of-contents)  
**Prerequisites**:
1. Configure `config.yml` or CLI arguments for SSH access.

**Commands**:
```bash
sbxctl --config config.yml execute ./scripts/typesense/install.sh ~/examples
```
Installs Typesense.