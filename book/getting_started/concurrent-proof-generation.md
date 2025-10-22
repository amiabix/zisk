# Concurrent Proof Generation

ZisK proofs can be generated using multiple processes concurrently to improve performance and scalability. The standard MPI (Message Passing Interface) approach is used to launch these processes, which can run either on the same server or across multiple servers.

## Basic Usage

To execute a ZisK proof using multiple processes, use the following command:

```bash
mpirun --bind-to none -np <num_processes> -x OMP_NUM_THREADS=<num_threads_per_process> -x RAYON_NUM_THREADS=<num_threads_per_process> target/release/cargo-zisk <zisk arguments>
```

## Command Parameters

In this command:

- `<num_processes>` specifies the number of processes to launch.
- `<num_threads_per_process>` sets the number of threads used by each process via the `OMP_NUM_THREADS` and `RAYON_NUM_THREADS` environment variables.
- `--bind-to none` prevents binding processes to specific cores, allowing the operating system to schedule them dynamically for better load balancing.

## Performance Considerations

Running a ZisK proof with multiple processes enables efficient workload distribution across multiple servers. On a single server with many cores, splitting execution into smaller subsets of cores generally improves performance by increasing concurrency. 

As a general rule, `<num_processes>` * `<num_threads_per_process>` should match the number of available CPU cores or double that if hyperthreading is enabled.

## Memory Requirements

The total memory requirement increases proportionally with the number of processes. If each process requires approximately 25GB of memory, running P processes will require roughly (25 * P)GB of memory. Ensure that the system has sufficient available memory to accommodate all running processes.

## Combining with GPU Acceleration

You can combine concurrent processing with GPU-based execution for maximum performance. See the [GPU Proof Generation](./gpu-proof-generation.md) guide for details.

> **Note:** When combining GPU execution with concurrent processing, ensure that each process has sufficient memory available on the GPU to avoid out-of-memory errors.
