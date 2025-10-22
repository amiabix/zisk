# Distributed System Configuration

This guide explains how to configure the ZisK distributed proving system for different deployment scenarios.

## Table of Contents

- [Configuration Methods](#configuration-methods)
- [Coordinator Configuration](#coordinator-configuration)
- [Worker Configuration](#worker-configuration)
- [Launching Proofs](#launching-proofs)
- [Monitoring and Health Checks](#monitoring-and-health-checks)
- [Troubleshooting](#troubleshooting)

---

## Configuration Methods

The distributed system supports multiple configuration approaches:

- **Configuration Files** - TOML files for production deployments
- **Command-Line Arguments** - Quick overrides and testing
- **Environment Variables** - Container and cloud deployments

**Priority Order**: Command-line arguments → Configuration files → Environment variables → Built-in defaults

---

## Coordinator Configuration

The coordinator manages the distributed proof generation process and assigns work to available workers.

### Using Configuration Files

Create a TOML configuration file and specify its location:

```bash
# Using command-line argument
cargo run --release --bin zisk-coordinator -- --config /path/to/config.toml

# Using environment variable
export ZISK_COORDINATOR_CONFIG_PATH="/path/to/config.toml"
cargo run --release --bin zisk-coordinator
```

If no configuration file is specified, the system uses built-in defaults.

### Basic Configuration Options

| TOML Key | CLI Argument | Environment Variable | Type | Default | Description |
|----------|--------------|---------------------|------|---------|-------------|
| `service.name` | - | - | String | "ZisK Distributed Coordinator" | Service name for identification |
| `service.environment` | - | - | String | development | Environment setting (development, staging, production) |
| `server.host` | - | - | String | "0.0.0.0" | Server host address |
| `server.port` | `--port` | - | Number | 50051 | gRPC server port |
| `server.shutdown_timeout_seconds` | - | - | Number | 30 | Graceful shutdown timeout in seconds |
| `logging.level` | - | RUST_LOG | String | debug | Log level (error, warn, info, debug, trace) |
| `logging.format` | - | - | String | pretty | Log format (pretty, json, compact) |
| `logging.file_path` | - | - | String | - | Optional log file path for file logging |

### Advanced Configuration Options

| TOML Key | CLI Argument | Environment Variable | Type | Default | Description |
|----------|--------------|---------------------|------|---------|-------------|
| `coordinator.max_workers_per_job` | - | - | Number | 10 | Maximum workers per proof job |
| `coordinator.max_total_workers` | - | - | Number | 1000 | Maximum total registered workers |
| `coordinator.phase1_timeout_seconds` | - | - | Number | 300 | Phase 1 timeout in seconds |
| `coordinator.phase2_timeout_seconds` | - | - | Number | 600 | Phase 2 timeout in seconds |
| `coordinator.webhook_url` | `--webhook-url` | - | String | - | Optional webhook URL for job completion notifications |

### Configuration Examples

#### Development Configuration

```toml
[service]
name = "ZisK Distributed Coordinator"
environment = "development"

[logging]
level = "debug"
format = "pretty"
```

#### Production Configuration

```toml
[service]
name = "ZisK Distributed Coordinator"
environment = "production"

[server]
host = "0.0.0.0"
port = 50051

[logging]
level = "info"
format = "json"
file_path = "/var/log/distributed/coordinator.log"

[coordinator]
max_workers_per_job = 20
max_total_workers = 5000
phase1_timeout_seconds = 600  # 10 minutes
phase2_timeout_seconds = 1200 # 20 minutes
webhook_url = "http://webhook.example.com/notify?job_id={$job_id}"
```

### Webhook Notifications

The coordinator can send notifications when jobs complete. Configure webhook URLs with job ID placeholders:

```bash
# With explicit placeholder
zisk-coordinator --webhook-url 'http://example.com/notify?job_id={$job_id}'
# Sends: http://example.com/notify?job_id=12345

# Without placeholder (ID appended automatically)
zisk-coordinator --webhook-url 'http://example.com/notify'
# Sends: http://example.com/notify/12345
```

---

## Worker Configuration

The worker executes proof generation tasks assigned by the coordinator.

### Using Configuration Files

Create a TOML configuration file and specify its location:

```bash
# Using command-line argument
cargo run --release --bin zisk-worker -- --config /path/to/config.toml

# Using environment variable
export ZISK_WORKER_CONFIG_PATH="/path/to/config.toml"
cargo run --release --bin zisk-worker
```

If no configuration file is specified, the system uses built-in defaults.

### Key Configuration Options

#### Basic Settings

**Worker Identity:**
- `worker.worker_id` - Unique worker identifier (auto-generated if not specified)
- `worker.compute_capacity.compute_units` - Worker compute capacity (default: 10)
- `worker.environment` - Environment setting (development, staging, production)

**Connection Settings:**
- `coordinator.url` - Coordinator server URL (default: "http://127.0.0.1:50051")
- `worker.inputs_folder` - Path to input files directory (default: current directory)

**Logging Configuration:**
- `logging.level` - Log level (error, warn, info, debug, trace, default: debug)
- `logging.format` - Log format (pretty, json, compact, default: pretty)
- `logging.file_path` - Optional log file path for file logging

#### Advanced Settings

**Proof Generation:**
- `--elf` - Path to ELF file (required)
- `--proving-key` - Path to setup folder (default: ~/.zisk/provingKey)
- `--witness-lib` - Path to witness computation library
- `--emulator` - Use prebuilt emulator (boolean, default: false)

**Performance Tuning:**
- `--compute-capacity` - Worker compute capacity (default: 10)
- `--number-threads-witness` - Threads for witness computation
- `--max-witness-stored` - Maximum witnesses in memory
- `--preallocate` - GPU preallocation flag
- `--max-streams` - Maximum GPU streams

**Debugging:**
- `-v`, `-vv`, `-vvv` - Verbosity level (0=error, 1=warn, 2=info, 3=debug, 4=trace)
- `--debug` - Enable debug mode with component filter
- `--verify-constraints` - Verify constraints (boolean, default: false)

### Input Files

Workers need to know where to find input files for proof generation. The `--inputs-folder` parameter specifies the base directory where input files are stored.

**How it works:**
- When the coordinator sends a prove command with a filename, the worker combines `--inputs-folder` + `filename` to locate the file
- Default location is the current working directory (`.`) if not specified
- This allows input files to be organized in a dedicated directory, separate from the worker executable

**Example:**
```bash
# Worker with inputs in specific folder
cargo run --release --bin zisk-worker -- --elf program.elf --inputs-folder /data/inputs/

# Coordinator requests proof for "input.bin" -> Worker looks for "/data/inputs/input.bin"
cargo run --release --bin zisk-coordinator -- prove --input input.bin --compute-capacity 10
```

### Configuration Examples

#### Development Configuration

```toml
[worker]
compute_capacity.compute_units = 10
environment = "development"

[logging]
level = "debug"
format = "pretty"
```

#### Production Configuration

```toml
[worker]
worker_id = "worker-001"
compute_capacity.compute_units = 10
environment = "production"
inputs_folder = "/app/inputs"

[coordinator]
url = "http://coordinator:50051"

[connection]
reconnect_interval_seconds = 5
heartbeat_timeout_seconds = 30

[logging]
level = "info"
format = "json"
file_path = "/var/log/distributed/worker-001.log"
```

---

## Launching Proofs

To launch a proof generation request, use the `prove` command:

```bash
cargo run --release --bin zisk-coordinator -- prove --input <input_filename> --compute-capacity 10
```

**How it works:**
- The `--compute-capacity` flag indicates the total compute units required for the proof
- The coordinator assigns one or more workers to meet this capacity
- If multiple workers are needed, the workload is distributed across them
- Requests that exceed the combined capacity of available workers will fail

---

## Monitoring and Health Checks

The coordinator exposes gRPC endpoints for monitoring the system:

```bash
# Basic health check
grpcurl -plaintext 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/HealthCheck

# System status
grpcurl -plaintext 127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/SystemStatus

# List active jobs
grpcurl -plaintext -d '{"active_only": true}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/JobsList

# List connected workers
grpcurl -plaintext -d '{"available_only": true}' \
  127.0.0.1:50051 zisk.distributed.api.v1.ZiskDistributedApi/WorkersList
```

---

## Troubleshooting

### Common Issues

**Worker can't connect to coordinator:**
- Verify coordinator is running and accessible on the specified port
- Check firewall settings if coordinator and worker are on different machines
- Ensure correct URL format: `http://host:port` (not `https://` for default setup)

**Configuration not loading:**
- Verify TOML syntax with a TOML validator
- Check file permissions on configuration files
- Use CLI overrides to test specific values

**Worker not receiving tasks:**
- Check worker registration in coordinator logs
- Verify compute capacity is appropriate for available tasks
- Ensure worker ID is unique if running multiple workers
- Confirm coordinator has active jobs to distribute

**Input file not found errors:**
- Verify the input file exists in the worker's `--inputs-folder` directory
- Check file permissions - worker needs read access to input files
- Ensure you're using the filename only (not full path) when launching proofs
- Confirm `--inputs-folder` path is correct and accessible

**Port conflicts:**
- Use `--port` flag or update configuration file to change ports
- Check for other services using the same ports

### Debug Mode

Enable detailed logging for troubleshooting by modifying configuration files or using CLI arguments:

```bash
# Coordinator with debug logging (via config file)
cargo run --release --bin zisk-coordinator -- --config debug-coordinator.toml

# Worker with debug logging (via config file)
cargo run --release --bin zisk-worker -- --config debug-worker.toml
```

Where `debug-coordinator.toml` or `debug-worker.toml` contains:
```toml
[logging]
level = "debug"
format = "pretty"
```

### Log Files

When file logging is enabled, logs are written into specified paths in the configuration files. Ensure the application has write permissions to these paths.

```toml
[logging]
file_path = "/var/log/distributed/coordinator.log"
```
