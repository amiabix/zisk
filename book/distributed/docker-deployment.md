# Docker Deployment

## Overview

This guide covers deploying ZisK's distributed proof generation system using Docker containers. Docker deployment provides consistent environments, easier scaling, and integration with orchestration platforms like Kubernetes and Docker Swarm.

## Table of Contents

- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Advanced Deployment](#advanced-deployment)
- [See Also](#see-also)

---

## Quick Start

The easiest way to get started with Docker deployment. For detailed configuration options, command-line arguments, and troubleshooting, see the [Configuration Guide](./configuration-guide.md).

### Step 1: Build the Docker Image

First, build the Docker image from the ZisK repository:

```bash
docker build -t zisk-distributed:latest -f distributed/Dockerfile .
```

**What's happening:** This builds the Docker image containing both the coordinator and worker binaries. The build process compiles the Rust code and creates an optimized container image.

### Step 2: Create a Network

Create a Docker network for container communication:

```bash
docker network create zisk-net
```

**What's happening:** This creates a custom Docker network that allows containers to communicate with each other using container names instead of IP addresses.

### Step 3: Start the Coordinator

Start the coordinator container:

```bash
docker run -d --rm --name zisk-coordinator \
  --network zisk-net \
  -p 50051:50051 \
  zisk-distributed:latest \
  zisk-coordinator
```

**What's happening:** 
- `-d`: Runs the container in detached mode (background)
- `--rm`: Automatically removes the container when it stops
- `--network zisk-net`: Connects to our custom network
- `-p 50051:50051`: Exposes port 50051 for gRPC communication
- The coordinator starts and waits for worker connections

### Step 4: Start a Worker

Start a worker container (replace paths with your actual directories):

```bash
docker run -d --rm --name zisk-worker-1 \
  --network zisk-net --shm-size=20g \
  -v /path/to/elf:/app/elf:ro \
  -v /path/to/inputs:/app/inputs:ro \
  zisk-distributed:latest \
  zisk-worker --coordinator-url http://zisk-coordinator:50051 \
    --elf /app/elf/program.elf \
    --inputs-folder /app/inputs
```

**What's happening:**
- `--shm-size=20g`: Allocates 20GB shared memory for proof generation
- `-v /path/to/elf:/app/elf:ro`: Mounts your ELF binary (read-only)
- `-v /path/to/inputs:/app/inputs:ro`: Mounts your input files (read-only)
- The worker connects to the coordinator and registers its availability

### Step 5: Generate a Proof

Submit a proof generation request:

```bash
docker exec -it zisk-coordinator \
  zisk-coordinator prove --input input.bin --compute-capacity 10
```

**What's happening:** This submits a proof request to the coordinator. The system will automatically execute the three-phase proof generation process and save the result to the `./proofs/` directory.

## Configuration

### Using Configuration Files

You can use TOML configuration files with Docker by mounting them as volumes:

```bash
# Mount configuration file
docker run -v /path/to/config:/app/config zisk-distributed:latest \
  zisk-coordinator --config /app/config/coordinator.toml

# Use environment variable for config path
docker run -e ZISK_COORDINATOR_CONFIG_PATH="/app/config/coordinator.toml" \
  -v /path/to/config:/app/config zisk-distributed:latest zisk-coordinator
```

### Common Configuration Options

**Coordinator:**
- `server.port`: gRPC server port (default: 50051)
- `logging.level`: Log level (debug, info, warn, error)
- `coordinator.max_workers_per_job`: Maximum workers per job
- `coordinator.webhook_url`: Webhook for job completion notifications

**Worker:**
- `worker.compute_capacity.compute_units`: Worker compute capacity
- `coordinator.url`: Coordinator server URL
- `worker.inputs_folder`: Path to input files directory


## Advanced Deployment

### Building Custom Images

```bash
# CPU-only build
docker build -t zisk-distributed:latest -f distributed/Dockerfile .

# GPU-enabled build
docker build --build-arg GPU=true -t zisk-distributed:gpu -f distributed/Dockerfile .
```

### Production Considerations

- **Resource Limits**: Set appropriate memory and CPU limits
- **Health Checks**: Implement container health checks
- **Logging**: Use structured logging and log aggregation
- **Monitoring**: Set up metrics collection and alerting
- **Security**: Use non-root users and read-only filesystems
- **Secrets**: Use Docker secrets or Kubernetes secrets for sensitive data

For detailed configuration options, command-line arguments, and troubleshooting, see the [Configuration Guide](./configuration-guide.md).

## See Also

- [Manual Deployment](./manual-deployment.md) for development and testing
- [Configuration Guide](./configuration-guide.md) for detailed configuration options, command-line arguments, and troubleshooting
