## Docker Commands

### Build the Docker Image

```bash
# Build standard CPU image
docker build -t zisk-distributed:latest -f distributed/Dockerfile .

# Build GPU-enabled image
docker build --build-arg GPU=true -t zisk-distributed:gpu -f distributed/Dockerfile .
```

### Run Coordinator Container

```bash
# Start coordinator container
docker run -d --rm --name zisk-coordinator \
  --network zisk-net \
  -v "$LOGS_DIR:/var/log/distributed" \
  zisk-distributed:latest \
  zisk-coordinator --config /app/config/coordinator/dev.toml
```

### Run Worker Container

```bash
# Start worker container
docker run -d --rm --name zisk-worker-1 \
  --network zisk-net \
  --shm-size=20g \
  -v "$LOGS_DIR:/var/log/distributed" \
  -v "$PROVING_KEYS:/app/proving-keys:ro" \
  -v "$ELF_FILES:/app/elf:ro" \
  -v "$INPUTS:/app/inputs:ro" \
  zisk-distributed:latest \
  zisk-worker --coordinator-url http://zisk-coordinator:50051 \
    --elf /app/elf/program.elf \
    --proving-key /app/proving-keys \
    --inputs-folder /app/inputs
```
