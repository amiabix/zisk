# Docker Deployment

> **Operators:** For configuration and deployment details, see [Distributed README](../../distributed/README.md)

---

This guide provides comprehensive instructions for deploying ZisK's distributed proof generation system using Docker containers. Docker deployment offers several advantages including consistent environments, simplified scaling, simplified dependency management, and easy integration with orchestration platforms like Kubernetes and Docker Swarm.

## Docker Image Architecture

The ZisK distributed system is packaged into Docker images that can be deployed across different environments. The system supports two main image variants to accommodate different hardware configurations and performance requirements.

The **CPU-Only variant** is the default configuration, optimized for CPU-based proof generation. This variant offers smaller image sizes and faster startup times, making it suitable for most deployment scenarios where GPU acceleration is not available or necessary. It's ideal for development environments, testing, and production deployments on standard cloud instances.

The **GPU-Enabled variant** includes CUDA support for GPU acceleration, providing significant performance gains for computationally intensive proof generation tasks. While this variant has a larger image size, the performance benefits often justify the additional overhead, especially for production workloads that need to maximize throughput. This variant requires the NVIDIA Docker runtime to be installed on the host system.

### Dockerfile Structure

The Docker image is built using a multi-stage build process that optimizes both build time and final image size. The Dockerfile is located at `distributed/Dockerfile` and follows best practices for Rust applications.

```dockerfile
# Multi-stage build for optimal image size
FROM rust:1.70 as builder

# Build arguments for feature selection
ARG GPU=false

# Build ZisK binaries with appropriate features
WORKDIR /app
COPY . .
RUN if [ "$GPU" = "true" ]; then \
      cargo build --release --features gpu --bin zisk-coordinator --bin zisk-worker; \
    else \
      cargo build --release --bin zisk-coordinator --bin zisk-worker; \
    fi

# Runtime image with minimal dependencies
FROM ubuntu:22.04

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binaries and configuration
COPY --from=builder /app/target/release/zisk-coordinator /app/bin/
COPY --from=builder /app/target/release/zisk-worker /app/bin/
COPY --from=builder /app/distributed/config /app/config/

WORKDIR /app
```

## Building Docker Images

Building Docker images for the ZisK distributed system is straightforward and supports various configurations depending on your deployment needs. The build process uses Docker's multi-stage build feature to create optimized images with minimal runtime dependencies.

### CPU-Only Build

For most deployments, the CPU-only variant provides the best balance of performance and resource usage. Building this variant is simple and doesn't require any special hardware or drivers.

```bash
# Build standard CPU image
docker build -t zisk-distributed:latest -f distributed/Dockerfile .

# Build with specific tag
docker build -t zisk-distributed:v1.0.0 -f distributed/Dockerfile .

# Build with build arguments
docker build --build-arg RUST_VERSION=1.70 -t zisk-distributed:latest -f distributed/Dockerfile .
```

### GPU-Enabled Build

When you need maximum performance for proof generation, the GPU-enabled variant can provide significant speedups. This variant requires CUDA support and should be built on systems with NVIDIA GPUs or in environments that support GPU acceleration.

```bash
# Build GPU-accelerated image
docker build --build-arg GPU=true -t zisk-distributed:gpu -f distributed/Dockerfile .

# Tag for registry
docker build --build-arg GPU=true -t myregistry.com/zisk-distributed:gpu -f distributed/Dockerfile .
```

### Multi-Architecture Builds

For deployments across different architectures, you can build multi-architecture images that support both AMD64 and ARM64 platforms. This is particularly useful for cloud deployments where you might have mixed hardware configurations.

```bash
# Build for multiple architectures
docker buildx create --use
docker buildx build --platform linux/amd64,linux/arm64 \
  -t zisk-distributed:latest \
  -f distributed/Dockerfile \
  --push .
```

## Container Orchestration

The ZisK distributed system can be deployed using various container orchestration platforms, each offering different benefits for different use cases. Docker Compose is ideal for local development and testing, while Kubernetes and Docker Swarm provide more robust solutions for production deployments.

### Docker Compose Setup

Docker Compose provides an excellent way to deploy the distributed system for local development, testing, and small-scale production deployments. It simplifies the management of multiple containers and their networking, making it easy to spin up the entire system with a single command.

```yaml
version: '3.8'

services:
  coordinator:
    image: zisk-distributed:latest
    container_name: zisk-coordinator
    networks:
      - zisk-net
    ports:
      - "50051:50051"
    volumes:
      - "./logs:/var/log/distributed"
    environment:
      - RUST_LOG=info
    command: >
      zisk-coordinator --config /app/config/coordinator/dev.toml
    healthcheck:
      test: ["CMD", "grpcurl", "-plaintext", "localhost:50051", "list"]
      interval: 30s
      timeout: 10s
      retries: 3

  worker-1:
    image: zisk-distributed:latest
    container_name: zisk-worker-1
    networks:
      - zisk-net
    depends_on:
      - coordinator
    volumes:
      - "./logs:/var/log/distributed"
      - "${HOME}/.zisk/cache:/app/.zisk/cache:ro"
      - "./proving-keys:/app/proving-keys:ro"
      - "./elf:/app/elf:ro"
      - "./inputs:/app/inputs:ro"
    environment:
      - RUST_LOG=info
    shm_size: "20g"
    command: >
      zisk-worker
      --coordinator-url http://coordinator:50051
      --elf /app/elf/program.elf
      --proving-key /app/proving-keys
      --inputs-folder /app/inputs
      --compute-capacity 8

  worker-2:
    image: zisk-distributed:latest
    container_name: zisk-worker-2
    networks:
      - zisk-net
    depends_on:
      - coordinator
    volumes:
      - "./logs:/var/log/distributed"
      - "${HOME}/.zisk/cache:/app/.zisk/cache:ro"
      - "./proving-keys:/app/proving-keys:ro"
      - "./elf:/app/elf:ro"
      - "./inputs:/app/inputs:ro"
    environment:
      - RUST_LOG=info
    shm_size: "20g"
    command: >
      zisk-worker
      --coordinator-url http://coordinator:50051
      --elf /app/elf/program.elf
      --proving-key /app/proving-keys
      --inputs-folder /app/inputs
      --compute-capacity 8

networks:
  zisk-net:
    driver: bridge
```

### Deployment Commands

Once you have your Docker Compose configuration ready, deploying the system is straightforward. The following commands will help you manage the deployment lifecycle.

```bash
# Start the entire stack
docker-compose up -d

# View logs
docker-compose logs -f coordinator
docker-compose logs -f worker-1

# Scale workers
docker-compose up -d --scale worker=4

# Stop and cleanup
docker-compose down
docker-compose down -v  # Also remove volumes
```

The `-d` flag runs containers in detached mode, allowing them to run in the background. The `--scale` option lets you dynamically adjust the number of worker instances, which is useful for testing different scaling scenarios or adjusting capacity based on demand.

## Production Deployment

For production environments, you'll want to use more robust orchestration platforms that provide features like automatic scaling, health monitoring, service discovery, and high availability. Kubernetes and Docker Swarm are both excellent choices, each with their own strengths.

### Kubernetes Deployment

Kubernetes provides the most comprehensive solution for production deployments, offering advanced features like automatic scaling, rolling updates, health checks, and service mesh integration. The following configurations show how to deploy the ZisK distributed system on Kubernetes.

#### Coordinator Deployment

The coordinator deployment includes resource limits, health checks, and proper service configuration to ensure reliable operation in a production environment.

```yaml
# coordinator-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: zisk-coordinator
  labels:
    app: zisk-coordinator
spec:
  replicas: 1
  selector:
    matchLabels:
      app: zisk-coordinator
  template:
    metadata:
      labels:
        app: zisk-coordinator
    spec:
      containers:
      - name: coordinator
        image: zisk-distributed:latest
        ports:
        - containerPort: 50051
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "1000m"
        env:
        - name: RUST_LOG
          value: "info"
        volumeMounts:
        - name: config-volume
          mountPath: /app/config
        - name: logs-volume
          mountPath: /var/log/distributed
        command: ["zisk-coordinator"]
        args: ["--config", "/app/config/coordinator/prod.toml"]
        livenessProbe:
          exec:
            command:
            - grpcurl
            - -plaintext
            - localhost:50051
            - list
          initialDelaySeconds: 30
          periodSeconds: 30
      volumes:
      - name: config-volume
        configMap:
          name: zisk-config
      - name: logs-volume
        persistentVolumeClaim:
          claimName: zisk-logs-pvc

---
apiVersion: v1
kind: Service
metadata:
  name: zisk-coordinator-service
spec:
  selector:
    app: zisk-coordinator
  ports:
  - protocol: TCP
    port: 50051
    targetPort: 50051
  type: LoadBalancer
```

#### Worker Deployment

The worker deployment is configured to scale horizontally and includes proper resource allocation for proof generation tasks. Each worker pod is configured with sufficient memory and CPU resources to handle the computational requirements of proof generation.

```yaml
# worker-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: zisk-worker
  labels:
    app: zisk-worker
spec:
  replicas: 3
  selector:
    matchLabels:
      app: zisk-worker
  template:
    metadata:
      labels:
        app: zisk-worker
    spec:
      containers:
      - name: worker
        image: zisk-distributed:latest
        resources:
          requests:
            memory: "4Gi"
            cpu: "2000m"
          limits:
            memory: "8Gi"
            cpu: "4000m"
        env:
        - name: RUST_LOG
          value: "info"
        - name: WORKER_ID
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        volumeMounts:
        - name: proving-keys
          mountPath: /app/proving-keys
          readOnly: true
        - name: elf-files
          mountPath: /app/elf
          readOnly: true
        - name: inputs
          mountPath: /app/inputs
          readOnly: true
        - name: cache
          mountPath: /app/.zisk/cache
        - name: logs-volume
          mountPath: /var/log/distributed
        command: ["zisk-worker"]
        args:
        - "--coordinator-url"
        - "http://zisk-coordinator-service:50051"
        - "--elf"
        - "/app/elf/program.elf"
        - "--proving-key"
        - "/app/proving-keys"
        - "--inputs-folder"
        - "/app/inputs"
        - "--worker-id"
        - "$(WORKER_ID)"
        - "--compute-capacity"
        - "8"
      volumes:
      - name: proving-keys
        persistentVolumeClaim:
          claimName: proving-keys-pvc
      - name: elf-files
        persistentVolumeClaim:
          claimName: elf-files-pvc
      - name: inputs
        persistentVolumeClaim:
          claimName: inputs-pvc
      - name: cache
        persistentVolumeClaim:
          claimName: cache-pvc
      - name: logs-volume
        persistentVolumeClaim:
          claimName: zisk-logs-pvc
```

### Docker Swarm Deployment

Docker Swarm provides a simpler alternative to Kubernetes for organizations that want container orchestration without the complexity of Kubernetes. It's particularly well-suited for smaller deployments or teams that are already familiar with Docker.

```yaml
# docker-stack.yml
version: '3.8'

services:
  coordinator:
    image: zisk-distributed:latest
    networks:
      - zisk-overlay
    ports:
      - "50051:50051"
    volumes:
      - logs:/var/log/distributed
    environment:
      - RUST_LOG=info
    command: zisk-coordinator --config /app/config/coordinator/prod.toml
    deploy:
      replicas: 1
      placement:
        constraints:
          - node.role == manager
      resources:
        limits:
          memory: 2G
          cpus: '1.0'
        reservations:
          memory: 1G
          cpus: '0.5'

  worker:
    image: zisk-distributed:latest
    networks:
      - zisk-overlay
    volumes:
      - logs:/var/log/distributed
      - proving-keys:/app/proving-keys:ro
      - elf-files:/app/elf:ro
      - inputs:/app/inputs:ro
      - cache:/app/.zisk/cache
    environment:
      - RUST_LOG=info
    command: >
      zisk-worker
      --coordinator-url http://coordinator:50051
      --elf /app/elf/program.elf
      --proving-key /app/proving-keys
      --inputs-folder /app/inputs
      --compute-capacity 8
    deploy:
      replicas: 4
      placement:
        constraints:
          - node.role == worker
      resources:
        limits:
          memory: 8G
          cpus: '4.0'
        reservations:
          memory: 4G
          cpus: '2.0'

networks:
  zisk-overlay:
    driver: overlay
    attachable: true

volumes:
  logs:
  proving-keys:
  elf-files:
  inputs:
  cache:
```

## Volume Management

Proper volume management is crucial for the distributed proving system to function correctly. The system requires several types of volumes with different access patterns and persistence requirements.

### Required Volumes

The distributed system requires several types of volumes to store different kinds of data. **Proving Keys** are stored in read-only volumes that are shared across all workers, containing the cryptographic keys needed for proof generation. **ELF Files** are also read-only and contain the compiled programs that workers will execute. **Inputs** are read-only volumes containing the input data for proof generation, while **Cache** volumes are read-write and store optimization data that can improve performance over time. **Logs** are write-only volumes where the system stores application logs for monitoring and debugging.

**Shared Memory** is particularly important for large proof computations, with a recommended size of 20GB or more. This shared memory is used for inter-process communication and temporary data storage during proof generation, and insufficient shared memory can cause proof generation to fail.

### Volume Configuration Examples

#### Docker Volume Mounts

```bash
# Create named volumes
docker volume create zisk-proving-keys
docker volume create zisk-cache
docker volume create zisk-logs

# Run with volume mounts
docker run -d --name zisk-coordinator \
  --network zisk-net \
  -v zisk-logs:/var/log/distributed \
  -p 50051:50051 \
  zisk-distributed:latest \
  zisk-coordinator

docker run -d --name zisk-worker-1 \
  --network zisk-net \
  --shm-size=20g \
  -v zisk-logs:/var/log/distributed \
  -v zisk-proving-keys:/app/proving-keys:ro \
  -v ./elf:/app/elf:ro \
  -v ./inputs:/app/inputs:ro \
  -v zisk-cache:/app/.zisk/cache \
  zisk-distributed:latest \
  zisk-worker --coordinator-url http://zisk-coordinator:50051 \
    --elf /app/elf/program.elf \
    --proving-key /app/proving-keys \
    --inputs-folder /app/inputs
```

#### Kubernetes Persistent Volume Claims

```yaml
# persistent-volumes.yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: proving-keys-pvc
spec:
  accessModes:
    - ReadOnlyMany
  resources:
    requests:
      storage: 50Gi
  storageClassName: fast-ssd

---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: cache-pvc
spec:
  accessModes:
    - ReadWriteMany
  resources:
    requests:
      storage: 100Gi
  storageClassName: fast-ssd

---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: zisk-logs-pvc
spec:
  accessModes:
    - ReadWriteMany
  resources:
    requests:
      storage: 20Gi
  storageClassName: standard
```

## Network Configuration

### Container Networking

#### Docker Bridge Network

```bash
# Create custom bridge network
docker network create --driver bridge \
  --subnet=172.20.0.0/16 \
  --gateway=172.20.0.1 \
  zisk-net

# Run containers on custom network
docker run -d --name zisk-coordinator \
  --network zisk-net \
  --ip 172.20.0.10 \
  -p 50051:50051 \
  zisk-distributed:latest

docker run -d --name zisk-worker-1 \
  --network zisk-net \
  --ip 172.20.0.11 \
  zisk-distributed:latest \
  zisk-worker --coordinator-url http://172.20.0.10:50051
```

#### Docker Overlay Network (Swarm)

```bash
# Create overlay network for multi-host communication
docker network create --driver overlay \
  --subnet=10.0.0.0/16 \
  --attachable \
  zisk-overlay

# Deploy stack with overlay network
docker stack deploy -c docker-stack.yml zisk-stack
```

### Load Balancing

#### NGINX Reverse Proxy

```nginx
# nginx.conf
upstream zisk-coordinators {
    server coordinator-1:50051;
    server coordinator-2:50051;
    # Add more coordinators for high availability
}

server {
    listen 50051 http2;
    
    location / {
        grpc_pass grpc://zisk-coordinators;
        grpc_set_header Host $host;
        grpc_set_header X-Real-IP $remote_addr;
        grpc_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

## Monitoring and Observability

### Health Checks

#### Container Health Checks

```dockerfile
# Add to Dockerfile
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD grpcurl -plaintext localhost:50051 zisk.distributed.api.v1.ZiskDistributedApi/HealthCheck || exit 1
```

#### Docker Compose Health Checks

```yaml
services:
  coordinator:
    # ... other configuration
    healthcheck:
      test: ["CMD", "grpcurl", "-plaintext", "localhost:50051", "zisk.distributed.api.v1.ZiskDistributedApi/HealthCheck"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
```

### Logging

#### Centralized Logging with ELK Stack

```yaml
# Add to docker-compose.yml
services:
  elasticsearch:
    image: elasticsearch:8.0.0
    environment:
      - discovery.type=single-node
      - "ES_JAVA_OPTS=-Xms512m -Xmx512m"
    volumes:
      - elasticsearch-data:/usr/share/elasticsearch/data

  logstash:
    image: logstash:8.0.0
    volumes:
      - ./logstash.conf:/usr/share/logstash/pipeline/logstash.conf
    depends_on:
      - elasticsearch

  kibana:
    image: kibana:8.0.0
    ports:
      - "5601:5601"
    depends_on:
      - elasticsearch

  coordinator:
    # ... other configuration
    logging:
      driver: "gelf"
      options:
        gelf-address: "udp://localhost:12201"
        tag: "zisk-coordinator"
```

#### Structured Logging Configuration

```toml
# Production logging configuration
[logging]
level = "info"
format = "json"
file_path = "/var/log/distributed/coordinator.log"

# Additional structured fields
[logging.fields]
service = "zisk-coordinator"
environment = "production"
version = "1.0.0"
```

### Metrics and Monitoring

#### Prometheus Integration

```yaml
# Add metrics endpoint to coordinator
services:
  coordinator:
    # ... other configuration
    ports:
      - "50051:50051"
      - "9090:9090"  # Metrics endpoint
    environment:
      - METRICS_ENABLED=true
      - METRICS_PORT=9090

  prometheus:
    image: prom/prometheus
    ports:
      - "9091:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
```

## Scaling Strategies

### Horizontal Scaling

#### Dynamic Worker Scaling

```bash
# Scale workers based on demand
docker-compose up -d --scale worker=8

# Kubernetes horizontal pod autoscaler
kubectl autoscale deployment zisk-worker \
  --cpu-percent=70 \
  --min=2 \
  --max=20
```

#### Multi-Region Deployment

```yaml
# Deploy across multiple regions
services:
  coordinator-us-east:
    image: zisk-distributed:latest
    deploy:
      placement:
        constraints:
          - node.labels.region == us-east

  coordinator-us-west:
    image: zisk-distributed:latest
    deploy:
      placement:
        constraints:
          - node.labels.region == us-west
```

### Resource Optimization

#### GPU Worker Deployment

```yaml
# GPU-enabled worker configuration
services:
  gpu-worker:
    image: zisk-distributed:gpu
    runtime: nvidia
    environment:
      - NVIDIA_VISIBLE_DEVICES=all
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
```

## Security Considerations

### Container Security

#### Security Hardening

```dockerfile
# Use non-root user
RUN groupadd -r zisk && useradd -r -g zisk zisk
USER zisk

# Read-only root filesystem
docker run --read-only --tmpfs /tmp --tmpfs /var/run zisk-distributed:latest
```

#### Network Security

```bash
# Restrict network access
docker run --network none zisk-worker  # Isolated worker
docker run --cap-drop ALL --cap-add NET_BIND_SERVICE zisk-coordinator
```

### Secrets Management

#### Docker Secrets

```bash
# Create secrets
echo "proving_key_password" | docker secret create proving_key_pass -

# Use in stack
services:
  worker:
    secrets:
      - proving_key_pass
secrets:
  proving_key_pass:
    external: true
```

#### Kubernetes Secrets

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: zisk-secrets
type: Opaque
data:
  proving-key-password: <base64-encoded-password>
```

This comprehensive Docker deployment guide provides the foundation for running ZisK's distributed proving system at scale in production environments with proper monitoring, security, and operational practices.

## See Also

- [Overview & Architecture](./introduction.md) for system concepts and architecture
- [Local Setup](./local_development.md) for development and testing
- [Distributed README](../../distributed/README.md) for operator configuration and troubleshooting
