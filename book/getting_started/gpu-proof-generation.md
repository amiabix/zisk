# GPU Proof Generation

ZisK supports GPU-accelerated proof generation for significantly improved performance and scalability on NVIDIA hardware.

## Prerequisites

**Hardware Requirements:**
- NVIDIA GPU with CUDA support

**Software Requirements:**
- [CUDA Toolkit](https://developer.nvidia.com/cuda-downloads) installed and configured

## Installation

GPU support must be enabled at compile time. Follow the standard installation process from the [Installation guide](./installation.md) (Option 2: Building from source), replacing the build command with:

```bash
cargo build --release --features gpu
```

**Recommended:** Build directly on the target GPU server for architecture-specific optimizations and improved runtime performance.

## Usage

Once built with GPU support, use ZisK commands as normal. The GPU will be automatically utilized during proof generation.

## Performance Optimization

### Concurrent Processing

Combine GPU execution with multi-process proof generation as described in [Concurrent Proof Generation](./concurrent-proof-generation.md).

**Memory Considerations:**
- GPU memory is more limited than CPU memory
- Each concurrent process requires dedicated GPU memory
- Monitor GPU memory usage to prevent out-of-memory errors
- Adjust the number of concurrent processes based on available GPU memory
